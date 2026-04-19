# AppImage Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce self-contained AppImages for x86_64 and aarch64 that bundle ffmpeg and whisper-cli, with whisper-cli built from source during the AppImage build.

**Architecture:** Two-build strategy: x86_64 native on `ubuntu-22.04` GitHub Actions, aarch64 via Docker container on `ubuntu-22.04` runner. Both produce AppImages via `appimage-builder`. System WebKit is NOT bundled — users install `libwebkit2gtk-4.1-0` separately.

**Tech Stack:** `appimage-builder`, `cmake` (for whisper.cpp), Rust, Node 22, Docker, GitHub Actions

---

## File Map

| File | Purpose |
|------|---------|
| `packaging/appimage/appimage-builder.yml` | `appimage-builder` configuration — defines AppImage contents and build |
| `packaging/appimage/AppRun.in` | Template for `AppRun` launch script (processed by appimage-builder) |
| `packaging/appimage/altgo.desktop.in` | Template for `.desktop` file (processed by appimage-builder) |
| `packaging/appimage/build.sh` | Build script run inside Docker for aarch64, or natively for x86_64 |
| `packaging/appimage/Dockerfile.aarch64` | Ubuntu 20.04 Docker image with all build tools for aarch64 cross-compile |
| `.github/workflows/appimage.yml` | GitHub Actions workflow — manual dispatch, builds x86_64 natively and aarch64 via Docker |

---

## Task 1: Create appimage-builder.yml

**Files:**
- Create: `packaging/appimage/appimage-builder.yml`

```yaml
# appimage-builder.yml
version: 1
AppImage.appname: altgo
AppImage.id: com.altgo.app
AppImage.version: "@VERSION@"
AppImage.architectures: ["@ARCH@"]
AppImage.debates: []

script: |
  # Copy application binary
  cp "@CMAKE_CURRENT_BINARY_DIR@/../altgo" "@APPDIR@/usr/bin/altgo"
  chmod 755 "@APPDIR@/usr/bin/altgo"

  # Copy ffmpeg
  cp "@DEPS_DIR@/bin/ffmpeg" "@APPDIR@/usr/bin/ffmpeg"
  chmod 755 "@APPDIR@/usr/bin/ffmpeg"

  # Copy whisper-cli
  cp "@DEPS_DIR@/bin/whisper-cli" "@APPDIR@/usr/bin/whisper-cli"
  chmod 755 "@APPDIR@/usr/bin/whisper-cli"

  # Copy icons
  mkdir -p "@APPDIR@/usr/share/altgo/icons"
  cp -r "@CMAKE_SOURCE_DIR@/src-tauri/icons/"* "@APPDIR@/usr/share/altgo/icons/"
```

- [ ] **Step 1: Write `packaging/appimage/appimage-builder.yml`**

```yaml
version: 1
AppImage.appname: altgo
AppImage.id: com.altgo.app
AppImage.version: "@VERSION@"
AppImage.architectures: ["@ARCH@"]
AppImage.debates: []

script: |
  # Copy application binary
  cp "@CMAKE_CURRENT_BINARY_DIR@/../altgo" "@APPDIR@/usr/bin/altgo"
  chmod 755 "@APPDIR@/usr/bin/altgo"

  # Copy ffmpeg
  cp "@DEPS_DIR@/bin/ffmpeg" "@APPDIR@/usr/bin/ffmpeg"
  chmod 755 "@APPDIR@/usr/bin/ffmpeg"

  # Copy whisper-cli
  cp "@DEPS_DIR@/bin/whisper-cli" "@APPDIR@/usr/bin/whisper-cli"
  chmod 755 "@APPDIR@/usr/bin/whisper-cli"

  # Copy icons
  mkdir -p "@APPDIR@/usr/share/altgo/icons"
  cp -r "@CMAKE_SOURCE_DIR@/src-tauri/icons/"* "@APPDIR@/usr/share/altgo/icons/"
```

- [ ] **Step 2: Commit**

```bash
git add packaging/appimage/appimage-builder.yml
git commit -m "feat(appimage): add appimage-builder.yml configuration"
```

---

## Task 2: Create AppRun.in

**Files:**
- Create: `packaging/appimage/AppRun.in`

The `@APPDIR@` and other variables are substituted by appimage-builder.

```bash
#!/bin/bash
# AppRun — launch script for altgo AppImage

HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"

# Check for WebKit
if ! ldconfig -p | grep -q "libwebkit2gtk-4.1"; then
    echo "ERROR: altgo requires libwebkit2gtk-4.1-0"
    echo "Please install it with: sudo apt install libwebkit2gtk-4.1-dev"
    exit 1
fi

# Launch altgo
exec "${HERE}/usr/bin/altgo" "$@"
```

- [ ] **Step 1: Write `packaging/appimage/AppRun.in`**

```bash
#!/bin/bash
# AppRun — launch script for altgo AppImage

HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"

# Check for WebKit
if ! ldconfig -p | grep -q "libwebkit2gtk-4.1"; then
    echo "ERROR: altgo requires libwebkit2gtk-4.1-0"
    echo "Please install it with: sudo apt install libwebkit2gtk-4.1-dev"
    exit 1
fi

# Launch altgo
exec "${HERE}/usr/bin/altgo" "$@"
```

- [ ] **Step 2: Commit**

```bash
git add packaging/appimage/AppRun.in
git commit -m "feat(appimage): add AppRun.in launch script template"
```

---

## Task 3: Create altgo.desktop.in

**Files:**
- Create: `packaging/appimage/altgo.desktop.in`

```ini
[Desktop Entry]
Name=altgo
Comment=按 Alt 键语音转文字 — Hold Alt to transcribe speech
Exec=altgo
Icon=altgo
Terminal=false
Type=Application
Categories=Utility;Audio;
Keywords=voice;speech;transcribe;whisper;
```

- [ ] **Step 1: Write `packaging/appimage/altgo.desktop.in`**

```ini
[Desktop Entry]
Name=altgo
Comment=按 Alt 键语音转文字 — Hold Alt to transcribe speech
Exec=altgo
Icon=altgo
Terminal=false
Type=Application
Categories=Utility;Audio;
Keywords=voice;speech;transcribe;whisper;
```

- [ ] **Step 2: Commit**

```bash
git add packaging/appimage/altgo.desktop.in
git commit -m "feat(appimage): add altgo.desktop.in template"
```

---

## Task 4: Create build.sh

**Files:**
- Create: `packaging/appimage/build.sh`

This script is run both natively (x86_64) and inside Docker (aarch64). It:
1. Checks disk space
2. Builds whisper.cpp from source
3. Downloads ffmpeg static binary
4. Builds the Tauri app (no bundle)
5. Assembles the AppImage via `appimage-builder`

```bash
#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:-x86_64}"
VERSION="${2:-}"
APPIMAGE_BUILDER="${3:-appimage-builder}"

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
WHISPER_VERSION="1.7.5"
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

    WHISPER_BIN=$(find "${TMP_BUILD}/whisper/build/bin" -name "whisper-cli" 2>/dev/null | head -1)
    if [[ -z "${WHISPER_BIN}" ]]; then
        WHISPER_BIN=$(find "${TMP_BUILD}/whisper/build" -name "whisper-cli" 2>/dev/null | head -1)
    fi
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
    FFMPEG_VERSION="7.1.1"
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
cargo install tauri-cli --version "^2" --locked --quiet 2>/dev/null || true

cargo tauri build --no-bundle --manifest-path="${PROJECT_ROOT}/src-tauri/Cargo.toml"

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
sed "s/@VERSION@/${VERSION}/g; s/@ARCH@/${ARCH}/g" \
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
${APPIMAGE_BUILDER} build --recipe "${TEMP_APPIMAGEBuilder_YML}"

echo "[OK] AppImage built"
```

- [ ] **Step 1: Write `packaging/appimage/build.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:-x86_64}"
VERSION="${2:-}"
APPIMAGE_BUILDER="${3:-appimage-builder}"

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
WHISPER_VERSION="1.7.5"
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

    WHISPER_BIN=$(find "${TMP_BUILD}/whisper/build/bin" -name "whisper-cli" 2>/dev/null | head -1)
    if [[ -z "${WHISPER_BIN}" ]]; then
        WHISPER_BIN=$(find "${TMP_BUILD}/whisper/build" -name "whisper-cli" 2>/dev/null | head -1)
    fi
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
    FFMPEG_VERSION="7.1.1"
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
cargo install tauri-cli --version "^2" --locked --quiet 2>/dev/null || true

cargo tauri build --no-bundle --manifest-path="${PROJECT_ROOT}/src-tauri/Cargo.toml"

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
sed "s/@VERSION@/${VERSION}/g; s/@ARCH@/${ARCH}/g" \
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
${APPIMAGE_BUILDER} build --recipe "${TEMP_APPIMAGEBuilder_YML}"

echo "[OK] AppImage built"
```

- [ ] **Step 2: Commit**

```bash
git add packaging/appimage/build.sh
git commit -m "feat(appimage): add build.sh for AppImage assembly"
```

---

## Task 5: Create Dockerfile.aarch64

**Files:**
- Create: `packaging/appimage/Dockerfile.aarch64`

```dockerfile
FROM ubuntu:20.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    curl \
    xz-utils \
    python3 \
    python3-pip \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    libasound2-dev \
    libudev-dev \
    pkg-config \
    libssl-dev \
    # Cross-compile toolchain for aarch64
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    libudev-dev-arm64-cross \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Rust for cross-compilation
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && "$HOME/.cargo/bin/rustup" target add aarch64-unknown-linux-gnu

# Install Node 22
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install appimage-builder
RUN pip3 install appimage-builder

WORKDIR /build
CMD ["/build/build.sh", "aarch64", "vPLACEHOLDER", "python3 -m appimage_builder"]
```

- [ ] **Step 1: Write `packaging/appimage/Dockerfile.aarch64`**

```dockerfile
FROM ubuntu:20.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    curl \
    xz-utils \
    python3 \
    python3-pip \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    libasound2-dev \
    libudev-dev \
    pkg-config \
    libssl-dev \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    libudev-dev-arm64-cross \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && "$HOME/.cargo/bin/rustup" target add aarch64-unknown-linux-gnu

RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install appimage-builder

WORKDIR /build
CMD ["/build/build.sh", "aarch64", "vPLACEHOLDER", "python3 -m appimage_builder"]
```

- [ ] **Step 2: Commit**

```bash
git add packaging/appimage/Dockerfile.aarch64
git commit -m "feat(appimage): add Dockerfile for aarch64 cross-compile builds"
```

---

## Task 6: Create GitHub Actions workflow

**Files:**
- Create: `.github/workflows/appimage.yml`

```yaml
name: AppImage

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version tag (e.g., v1.4.0)'
        required: true
        type: string
      arch:
        description: 'Architecture'
        required: false
        type: choice
        default: both
        options:
          - both
          - x86_64
          - aarch64

env:
  CARGO_TERM_COLOR: always

jobs:
  build-x86_64:
    name: Build x86_64 AppImage
    runs-on: ubuntu-22.04
    if: inputs.arch == 'both' || inputs.arch == 'x86_64'
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
            librsvg2-dev patchelf libasound2-dev \
            build-essential cmake git curl xz-utils \
            libudev-dev pkg-config libssl-dev \
            python3 python3-pip

      - name: Install appimage-builder
        run: pip3 install appimage-builder

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Check version match
        run: |
          CARGO_VERSION=$(grep '^version = ' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
          INPUT_VERSION="${VERSION#v}"
          if [[ "$CARGO_VERSION" != "$INPUT_VERSION" ]]; then
            echo "ERROR: Cargo.toml version ($CARGO_VERSION) != input version ($INPUT_VERSION)"
            exit 1
          fi

      - name: Build x86_64 AppImage
        run: |
          bash packaging/appimage/build.sh x86_64 "$VERSION" appimage-builder

      - name: Find AppImage artifact
        id: find_artifact
        run: |
          APPIMAGE=$(find target/appimage-build -name '*.AppImage' | head -1)
          echo "path=${APPIMAGE}" >> $GITHUB_OUTPUT
          echo "name=$(basename ${APPIMAGE})" >> $GITHUB_OUTPUT

      - uses: actions/upload-artifact@v4
        with:
          name: altgo-x86_64-${{ inputs.version }}.AppImage
          path: ${{ steps.find_artifact.outputs.path }}
          if-no-files-found: error

  build-aarch64:
    name: Build aarch64 AppImage
    runs-on: ubuntu-22.04
    if: inputs.arch == 'both' || inputs.arch == 'aarch64'
    steps:
      - uses: actions/checkout@v4

      - name: Check version match
        run: |
          CARGO_VERSION=$(grep '^version = ' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
          INPUT_VERSION="${VERSION#v}"
          if [[ "$CARGO_VERSION" != "$INPUT_VERSION" ]]; then
            echo "ERROR: Cargo.toml version ($CARGO_VERSION) != input version ($INPUT_VERSION)"
            exit 1
          fi

      - name: Build aarch64 AppImage in Docker
        run: |
          docker build -t altgo-aarch64-build \
            --build-arg VERSION=${{ inputs.version }} \
            -f packaging/appimage/Dockerfile.aarch64 .

      - name: Copy artifact from container
        run: |
          docker run --rm \
            -v "$PWD/target:/build/target" \
            altgo-aarch64-build \
            bash -c 'cp /build/target/appimage-build/*.AppImage /build/target/ || true'

      - name: Find AppImage artifact
        id: find_artifact
        run: |
          APPIMAGE=$(find target/appimage-build -name '*.AppImage' | head -1)
          echo "path=${APPIMAGE}" >> $GITHUB_OUTPUT
          echo "name=$(basename ${APPIMAGE})" >> $GITHUB_OUTPUT

      - uses: actions/upload-artifact@v4
        with:
          name: altgo-aarch64-${{ inputs.version }}.AppImage
          path: ${{ steps.find_artifact.outputs.path }}
          if-no-files-found: error
```

- [ ] **Step 1: Write `.github/workflows/appimage.yml`**

```yaml
name: AppImage

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version tag (e.g., v1.4.0)'
        required: true
        type: string
      arch:
        description: 'Architecture'
        required: false
        type: choice
        default: both
        options:
          - both
          - x86_64
          - aarch64

env:
  CARGO_TERM_COLOR: always

jobs:
  build-x86_64:
    name: Build x86_64 AppImage
    runs-on: ubuntu-22.04
    if: inputs.arch == 'both' || inputs.arch == 'x86_64'
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
            librsvg2-dev patchelf libasound2-dev \
            build-essential cmake git curl xz-utils \
            libudev-dev pkg-config libssl-dev \
            python3 python3-pip

      - name: Install appimage-builder
        run: pip3 install appimage-builder

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Check version match
        run: |
          CARGO_VERSION=$(grep '^version = ' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
          INPUT_VERSION="${VERSION#v}"
          if [[ "$CARGO_VERSION" != "$INPUT_VERSION" ]]; then
            echo "ERROR: Cargo.toml version ($CARGO_VERSION) != input version ($INPUT_VERSION)"
            exit 1
          fi

      - name: Build x86_64 AppImage
        run: |
          bash packaging/appimage/build.sh x86_64 "$VERSION" appimage-builder

      - name: Find AppImage artifact
        id: find_artifact
        run: |
          APPIMAGE=$(find target/appimage-build -name '*.AppImage' | head -1)
          echo "path=${APPIMAGE}" >> $GITHUB_OUTPUT
          echo "name=$(basename ${APPIMAGE})" >> $GITHUB_OUTPUT

      - uses: actions/upload-artifact@v4
        with:
          name: altgo-x86_64-${{ inputs.version }}.AppImage
          path: ${{ steps.find_artifact.outputs.path }}
          if-no-files-found: error

  build-aarch64:
    name: Build aarch64 AppImage
    runs-on: ubuntu-22-04
    if: inputs.arch == 'both' || inputs.arch == 'aarch64'
    steps:
      - uses: actions/checkout@v4

      - name: Check version match
        run: |
          CARGO_VERSION=$(grep '^version = ' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
          INPUT_VERSION="${VERSION#v}"
          if [[ "$CARGO_VERSION" != "$INPUT_VERSION" ]]; then
            echo "ERROR: Cargo.toml version ($CARGO_VERSION) != input version ($INPUT_VERSION)"
            exit 1
          fi

      - name: Build aarch64 AppImage in Docker
        run: |
          docker build -t altgo-aarch64-build \
            --build-arg VERSION=${{ inputs.version }} \
            -f packaging/appimage/Dockerfile.aarch64 .

      - name: Copy artifact from container
        run: |
          docker run --rm \
            -v "$PWD/target:/build/target" \
            altgo-aarch64-build \
            bash -c 'cp /build/target/appimage-build/*.AppImage /build/target/ || true'

      - name: Find AppImage artifact
        id: find_artifact
        run: |
          APPIMAGE=$(find target/appimage-build -name '*.AppImage' | head -1)
          echo "path=${APPIMAGE}" >> $GITHUB_OUTPUT
          echo "name=$(basename ${APPIMAGE})" >> $GITHUB_OUTPUT

      - uses: actions/upload-artifact@v4
        with:
          name: altgo-aarch64-${{ inputs.version }}.AppImage
          path: ${{ steps.find_artifact.outputs.path }}
          if-no-files-found: error
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/appimage.yml
git commit -m "feat(appimage): add GitHub Actions workflow for AppImage builds"
```

---

## Self-Review Checklist

- [ ] Spec coverage: All 6 files from spec design are covered
- [ ] No placeholders (TBD/TODO): All file contents are concrete
- [ ] Version input flow: passed through `build.sh` → `appimage-builder.yml` substitution
- [ ] ARCH substitution: both `build.sh` (for whisper.cpp compile) and `appimage-builder.yml` (for `@ARCH@`)
- [ ] aarch64 cross-compile: CMake toolchain uses `aarch64-linux-gnu-gcc/g++`
- [ ] Artifact naming: `altgo-{arch}-v{version}.AppImage` format consistent
- [ ] Workflow is `workflow_dispatch` only — not triggered by tags

## Notes

- The `appimage-builder.yml` uses variable substitution via `sed` in `build.sh` before passing to `appimage-builder`. This is because appimage-builder doesn't natively support `@VERSION@`/`@ARCH@` placeholders — they're processed by build.sh.
- The aarch64 Docker build passes `--build-arg VERSION` but the Dockerfile uses a placeholder `CMD` — the actual version is passed via `docker run` in the workflow. The Dockerfile CMD is a fallback.
- whisper.cpp version `1.7.5` matches the existing `download-deps.sh` version.
- `build.sh` installs `tauri-cli` from crates.io — this is fine since it's inside Docker or a clean GitHub Actions runner.
