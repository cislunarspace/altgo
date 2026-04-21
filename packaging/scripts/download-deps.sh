#!/usr/bin/env bash
# Download platform dependencies for altgo packaging.
# Usage: ./scripts/download-deps.sh [x86_64|aarch64]
#
# Downloads ffmpeg (static) and builds whisper-cli into target/deps/bin/

set -euo pipefail

ARCH="${1:-x86_64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DEPS_DIR="${REPO_ROOT}/target/deps"
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

# ─── Build whisper-cli from source (persistent cache) ─────────────────────────
# 使用 GitHub 源码归档 tar.gz + target/deps/whisper.cpp-src，避免 git clone 在网络差时长时间无输出/卡住。
# 版本变化或缓存损坏时重新下载；日常仅增量 cmake build。
build_whisper_from_source() {
    local target="$1"
    local cache="${DEPS_DIR}/whisper.cpp-src"
    local tag="v${WHISPER_VERSION}"
    local version_file="${cache}/.altgo-whisper-version"
    local archive_url="https://github.com/ggml-org/whisper.cpp/archive/refs/tags/${tag}.tar.gz"
    echo "[INFO] Building whisper-cli ${tag} (cache: target/deps/whisper.cpp-src)..."

    local need_fetch=0
    if [[ ! -f "${cache}/CMakeLists.txt" ]]; then
        need_fetch=1
    elif [[ ! -f "${version_file}" ]] || [[ "$(cat "${version_file}")" != "${WHISPER_VERSION}" ]]; then
        need_fetch=1
    fi

    if [[ "${need_fetch}" -eq 1 ]]; then
        echo "[INFO] Downloading ${archive_url} ..."
        local tmp
        tmp="$(mktemp)"
        if ! curl --fail --location --progress-bar \
            --connect-timeout 30 \
            --max-time 600 \
            --retry 2 \
            --retry-delay 3 \
            -o "${tmp}" "${archive_url}"; then
            rm -f "${tmp}"
            echo "[ERROR] Download failed (timeout or network). Try: export https_proxy=... ; or download the URL manually and extract to ${cache}"
            exit 1
        fi
        rm -rf "${cache}"
        mkdir -p "${cache}"
        if ! tar xzf "${tmp}" --strip-components=1 -C "${cache}"; then
            rm -f "${tmp}"
            echo "[ERROR] Failed to extract whisper.cpp archive"
            exit 1
        fi
        rm -f "${tmp}"
        echo "${WHISPER_VERSION}" > "${version_file}"
    else
        echo "[OK] Reusing whisper.cpp source cache (${tag})"
    fi

    cd "${cache}"
    native="$(uname -m)"
    if [[ "${ARCH}" == "aarch64" && "${native}" != "aarch64" ]] && command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        cmake -B build -DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc -DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++ .
    else
        cmake -B build .
    fi
    cmake --build build --config Release -j"$(nproc)"

    # 必须用 build/bin/whisper-cli：同目录下还有 examples 的 main（十几 KB），find|head 会误选导致运行失败。
    local whisper_bin="${cache}/build/bin/whisper-cli"
    if [[ ! -x "${whisper_bin}" ]]; then
        echo "[ERROR] whisper-cli not found after build: ${whisper_bin}"
        exit 1
    fi

    if ! command -v patchelf >/dev/null 2>&1; then
        echo "[ERROR] patchelf is required to bundle whisper shared libs (e.g. sudo apt install patchelf)"
        exit 1
    fi

    # 动态库默认 RUNPATH 指向构建目录；复制到 target/deps/bin 并改为 \$ORIGIN，便于随 altgo-tauri 分发。
    cp -a "${cache}/build/src"/libwhisper.so* "${BIN_DIR}/"
    cp -a "${cache}/build/ggml/src"/libggml*.so* "${BIN_DIR}/"
    cp -f "${whisper_bin}" "${target}"
    chmod +x "${target}"

    patchelf --set-rpath '$ORIGIN' "${target}"
    shopt -s nullglob
    for so in "${BIN_DIR}"/libwhisper.so.[0-9]* "${BIN_DIR}"/libggml.so.[0-9]* \
        "${BIN_DIR}"/libggml-base.so.[0-9]* "${BIN_DIR}"/libggml-cpu.so.[0-9]*; do
        [[ -f "${so}" && ! -L "${so}" ]] || continue
        patchelf --set-rpath '$ORIGIN' "${so}"
    done
    shopt -u nullglob

    cd "${REPO_ROOT}"
    echo "[OK] whisper-cli built from source"
}

# ─── whisper-cli (Linux) ─────────────────────────────────────────────────────
# Upstream GitHub releases no longer ship Linux whisper-cli binaries; build from source
# (archive download + cmake). Needs: cmake, C++ compiler, curl.
WHISPER_VERSION="1.8.4"
WHISPER_TARGET="${BIN_DIR}/whisper-cli"

if [[ -f "${WHISPER_TARGET}" ]]; then
    echo "[OK] whisper-cli already exists at ${WHISPER_TARGET}"
else
    echo "[INFO] Building whisper-cli v${WHISPER_VERSION} from source (${ARCH})..."
    build_whisper_from_source "${WHISPER_TARGET}"
fi

echo "[OK] All dependencies ready in ${BIN_DIR}/"
