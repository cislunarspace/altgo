#!/usr/bin/env bash
# Download platform dependencies for altgo packaging.
# Usage: ./scripts/download-deps.sh [x86_64|aarch64]
#
# Downloads ffmpeg (static) and whisper-cli into target/deps/bin/

set -euo pipefail

ARCH="${1:-x86_64}"
DEPS_DIR="target/deps"
BIN_DIR="${DEPS_DIR}/bin"

echo "Downloading dependencies for ${ARCH}..."
mkdir -p "${BIN_DIR}"

# ─── ffmpeg (static build) ────────────────────────────────────────────────────
FFMPEG_VERSION="7.1.1"
FFMPEG_TARGET="${BIN_DIR}/ffmpeg"

if [[ -f "${FFMPEG_TARGET}" ]]; then
    echo "[OK] ffmpeg already exists at ${FFMPEG_TARGET}"
else
    echo "[INFO] Downloading ffmpeg ${FFMPEG_VERSION} (${ARCH})..."

    if [[ "${ARCH}" == "x86_64" ]]; then
        FFMPEG_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
    else
        FFMPEG_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
    fi

    TMP_DIR=$(mktemp -d)
    curl --fail --progress-bar -L -o "${TMP_DIR}/ffmpeg.tar.xz" "${FFMPEG_URL}"
    tar xf "${TMP_DIR}/ffmpeg.tar.xz" -C "${TMP_DIR}"

    # Find the ffmpeg binary inside the extracted directory.
    FFMPEG_BIN=$(find "${TMP_DIR}" -name "ffmpeg" -type f | head -1)
    if [[ -z "${FFMPEG_BIN}" ]]; then
        echo "[ERROR] ffmpeg binary not found in archive"
        rm -rf "${TMP_DIR}"
        exit 1
    fi

    cp "${FFMPEG_BIN}" "${FFMPEG_TARGET}"
    chmod +x "${FFMPEG_TARGET}"
    rm -rf "${TMP_DIR}"
    echo "[OK] ffmpeg downloaded to ${FFMPEG_TARGET}"
fi

# ─── whisper-cli ───────────────────────────────────────────────────────────────
WHISPER_VERSION="1.7.5"
WHISPER_TARGET="${BIN_DIR}/whisper-cli"

if [[ -f "${WHISPER_TARGET}" ]]; then
    echo "[OK] whisper-cli already exists at ${WHISPER_TARGET}"
else
    echo "[INFO] Downloading whisper-cli ${WHISPER_VERSION} (${ARCH})..."

    if [[ "${ARCH}" == "x86_64" ]]; then
        WHISPER_URL="https://github.com/ggml-org/whisper.cpp/releases/download/v${WHISPER_VERSION}/whisper-linux-x64.tar.gz"
    else
        WHISPER_URL="https://github.com/ggml-org/whisper.cpp/releases/download/v${WHISPER_VERSION}/whisper-linux-arm64.tar.gz"
    fi

    TMP_DIR=$(mktemp -d)
    if curl --fail --progress-bar -L -o "${TMP_DIR}/whisper.tar.gz" "${WHISPER_URL}"; then
        tar xzf "${TMP_DIR}/whisper.tar.gz" -C "${TMP_DIR}"
        WHISPER_BIN=$(find "${TMP_DIR}" -name "whisper-cli" -o -name "main" | head -1)
        if [[ -n "${WHISPER_BIN}" ]]; then
            cp "${WHISPER_BIN}" "${WHISPER_TARGET}"
            chmod +x "${WHISPER_TARGET}"
            echo "[OK] whisper-cli downloaded to ${WHISPER_TARGET}"
        else
            echo "[WARN] whisper-cli binary not found in archive, will try building from source"
            build_whisper_from_source "${WHISPER_TARGET}"
        fi
    else
        echo "[WARN] whisper-cli prebuilt binary not available for ${ARCH}, building from source..."
        build_whisper_from_source "${WHISPER_TARGET}"
    fi
    rm -rf "${TMP_DIR}"
fi

echo "[OK] All dependencies ready in ${BIN_DIR}/"

# ─── Build whisper-cli from source (fallback) ──────────────────────────────────
build_whisper_from_source() {
    local target="$1"
    echo "[INFO] Building whisper-cli from source..."

    TMP_BUILD=$(mktemp -d)
    git clone --depth 1 --branch "v${WHISPER_VERSION}" https://github.com/ggml-org/whisper.cpp.git "${TMP_BUILD}/whisper"

    cd "${TMP_BUILD}/whisper"
    if [[ "${ARCH}" == "aarch64" ]]; then
        cmake -B build -DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc -DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++ .
    else
        cmake -B build .
    fi
    cmake --build build --config Release -j"$(nproc)"

    cp build/bin/whisper-cli "${target}" 2>/dev/null || cp build/bin/main "${target}" 2>/dev/null || {
        echo "[ERROR] Failed to build whisper-cli"
        exit 1
    }
    chmod +x "${target}"
    cd -
    rm -rf "${TMP_BUILD}"
    echo "[OK] whisper-cli built from source"
}
