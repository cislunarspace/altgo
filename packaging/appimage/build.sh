#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:-x86_64}"
VERSION="${2:-}"
APPIMAGE_BUILDER="${3:-appimage-builder}"

if [[ ! "${ARCH}" =~ ^(x86_64|aarch64)$ ]]; then
    echo "ERROR: ARCH must be x86_64 or aarch64"
    exit 1
fi

# Initialize cleanup variables
TMP_DIR=""
TMP_BUILD=""

# Cleanup trap for temp directories
trap 'rm -rf "${TMP_DIR}" "${TMP_BUILD}" 2>/dev/null || true' EXIT

# Validate inputs
if [[ -z "$VERSION" ]]; then
    echo "ERROR: VERSION is required (e.g., v1.4.0)"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DEPS_DIR="${PROJECT_ROOT}/target/deps"
BIN_DIR="${DEPS_DIR}/bin"
BUILD_APPIMAGE_DIR="${PROJECT_ROOT}/target/appimage-build"

# Source unified version constants
source "${SCRIPT_DIR}/../scripts/versions.sh"

echo "=== AppImage build for ${ARCH} ==="
echo "VERSION=${VERSION}"
echo "PROJECT_ROOT=${PROJECT_ROOT}"

# Disk space check (~10GB needed)
AVAILABLE_KB=$(df --output=avail . | tail -1 | tr -d ' ')
NEEDED_KB=10485760  # 10GB in KB
if (( AVAILABLE_KB < NEEDED_KB )); then
    echo "ERROR: Insufficient disk space. Need ~10GB, have $((AVAILABLE_KB / 1024 / 1024))GB"
    exit 1
fi

mkdir -p "${BIN_DIR}" "${BUILD_APPIMAGE_DIR}"

# ─── Step 1: Build whisper.cpp from source ────────────────────────────────────
WHISPER_VERSION="${WHISPER_CPP_VERSION}"
WHISPER_TARGET="${BIN_DIR}/whisper-cli"

if [[ -f "${WHISPER_TARGET}" ]]; then
    echo "[OK] whisper-cli already exists"
else
    echo "[INFO] Building whisper-cli from source..."
    TMP_BUILD=$(mktemp -d)
    git clone --depth 1 --branch "v${WHISPER_VERSION}" https://github.com/ggml-org/whisper.cpp.git "${TMP_BUILD}/whisper"

    cd "${TMP_BUILD}/whisper"
    if [[ "${ARCH}" == "aarch64" ]]; then
        cmake -B build \
            -DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc \
            -DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++ \
            -DCMAKE_BUILD_TYPE=Release
    else
        cmake -B build -DCMAKE_BUILD_TYPE=Release
    fi
    cmake --build build -j"$(nproc)"

    WHISPER_BIN=$(find "${TMP_BUILD}/whisper/build" -name "whisper-cli" 2>/dev/null | head -1)
    if [[ -z "${WHISPER_BIN}" ]]; then
        echo "ERROR: whisper-cli binary not found after build"
        exit 1
    fi

    cp "${WHISPER_BIN}" "${WHISPER_TARGET}"
    chmod +x "${WHISPER_TARGET}"
    rm -rf "${TMP_BUILD}"
    echo "[OK] whisper-cli built"
fi

# ─── Step 2: Download ffmpeg static binary ────────────────────────────────────
FFMPEG_TARGET="${BIN_DIR}/ffmpeg"

if [[ -f "${FFMPEG_TARGET}" ]]; then
    echo "[OK] ffmpeg already exists"
else
    echo "[INFO] Downloading ffmpeg..."
    if [[ "${ARCH}" == "x86_64" ]]; then
        FFMPEG_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
    else
        FFMPEG_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
    fi

    TMP_DIR=$(mktemp -d)
    curl --fail --progress-bar -L -o "${TMP_DIR}/ffmpeg.tar.xz" "${FFMPEG_URL}"
    tar xf "${TMP_DIR}/ffmpeg.tar.xz" -C "${TMP_DIR}"
    FFMPEG_BIN=$(find "${TMP_DIR}" -name "ffmpeg" -type f | head -1)
    if [[ -z "${FFMPEG_BIN}" ]]; then
        echo "ERROR: ffmpeg binary not found in archive"
        exit 1
    fi
    cp "${FFMPEG_BIN}" "${FFMPEG_TARGET}"
    chmod +x "${FFMPEG_TARGET}"
    rm -rf "${TMP_DIR}"
    echo "[OK] ffmpeg downloaded"
fi

# ─── Step 3: Install Rust + Node deps ─────────────────────────────────────────
cd "${PROJECT_ROOT}"
if ! command -v rustc &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

if ! command -v npm &>/dev/null; then
    echo "ERROR: npm is required but not installed"
    exit 1
fi

npm ci --prefix "${PROJECT_ROOT}/frontend"

# ─── Step 4: Build Tauri app (no bundle) ─────────────────────────────────────
echo "[INFO] Building Tauri app..."
cargo install tauri-cli --version "^2" --locked --quiet

cd "${PROJECT_ROOT}"
cargo tauri build --no-bundle

# Locate the built binary
TAURI_BINARY=$(find "${PROJECT_ROOT}/src-tauri/target/release" -name "altgo" -type f | head -1)
if [[ -z "${TAURI_BINARY}" ]]; then
    echo "ERROR: altgo binary not found after build"
    exit 1
fi
echo "[OK] Tauri binary: ${TAURI_BINARY}"

# ─── Step 5: Assemble AppImage ─────────────────────────────────────────────────
echo "[INFO] Assembling AppImage..."

# Write effective VERSION and ARCH to a temp file for appimage-builder
TEMP_APPIMAGEBuilder_YML="${BUILD_APPIMAGE_DIR}/appimage-builder.yml"
sed "s|@VERSION@|${VERSION}|g; s|@ARCH@|${ARCH}|g" \
    "${SCRIPT_DIR}/appimage-builder.yml" > "${TEMP_APPIMAGEBuilder_YML}"

TEMP_APPRUN="${BUILD_APPIMAGE_DIR}/AppRun"
sed "s|@APPDIR@|\${APPDIR}|g" "${SCRIPT_DIR}/AppRun.in" > "${TEMP_APPRUN}"
chmod +x "${TEMP_APPRUN}"

TEMP_DESKTOP="${BUILD_APPIMAGE_DIR}/altgo.desktop"
cp "${SCRIPT_DIR}/altgo.desktop.in" "${TEMP_DESKTOP}"

cd "${BUILD_APPIMAGE_DIR}"

DEPS_DIR="${DEPS_DIR}" \
CMAKE_SOURCE_DIR="${PROJECT_ROOT}" \
CMAKE_CURRENT_BINARY_DIR="${PROJECT_ROOT}/src-tauri/target/release" \
${APPIMAGE_BUILDER} --recipe "${TEMP_APPIMAGEBuilder_YML}"

echo "[OK] AppImage built"
