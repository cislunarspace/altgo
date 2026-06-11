#!/usr/bin/env bash
# Download platform dependencies for altgo packaging.
# Usage: ./scripts/download-deps.sh [x86_64|aarch64]
#
# Downloads ffmpeg (static) and builds whisper-cli into target/deps/bin/

set -euo pipefail

ARCH="${1:-x86_64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Source unified version constants
source "${SCRIPT_DIR}/versions.sh"

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

# ─── Detect and compose CMake acceleration flags ──────────────────────────────
detect_accel_flags() {
    local flags=""
    # 必须关闭 -march=native，否则二进制只在构建机器上运行，其它机器 SIGILL。
    flags="${flags} -DGGML_NATIVE=OFF"

    # CUDA：构建端探测 nvcc；运行时若目标机无 NVIDIA 驱动则优雅回退 CPU（通过 -DGGML_CUDA_NO_VMM=ON
    # 避免加载时硬依赖 libcuda.so.1）。
    if command -v nvcc >/dev/null 2>&1; then
        echo "[INFO] CUDA detected (nvcc: $(nvcc --version 2>/dev/null | tail -1)) — enabling GPU backend" >&2
        local nvcc_path
        nvcc_path="$(command -v nvcc 2>/dev/null || true)"
        flags="${flags} -DGGML_CUDA=ON -DGGML_CUDA_NO_VMM=ON -DCMAKE_CUDA_COMPILER=${nvcc_path}"
    else
        echo "[INFO] CUDA not detected — CPU-only build" >&2
    fi

    # OpenBLAS：modest encoder-only speedup；暂不启用，因需额外构建依赖且增益有限。
    # 如需，可在此加：-DGGML_BLAS=ON -DGGML_BLAS_VENDOR=OpenBLAS

    printf '%s' "${flags# }"
}

# ─── Build whisper-cli from source (persistent cache) ─────────────────────────
# 使用 GitHub 源码归档 tar.gz + target/deps/whisper.cpp-src，避免 git clone 在网络差时长时间无输出/卡住。
# 版本变化、缓存损坏或构建选项变化时重新下载/重新配置；日常仅增量 cmake build。
build_whisper_from_source() {
    local target="$1"
    local cache="${DEPS_DIR}/whisper.cpp-src"
    local tag="v${WHISPER_VERSION}"
    local version_file="${cache}/.altgo-whisper-version"
    local buildopts_file="${cache}/.altgo-whisper-buildopts"
    local archive_url="https://github.com/ggml-org/whisper.cpp/archive/refs/tags/${tag}.tar.gz"
    local cmake_flags
    cmake_flags="$(detect_accel_flags)"

    echo "[INFO] Building whisper-cli ${tag} (cache: target/deps/whisper.cpp-src)..."
    echo "[INFO] CMake flags: ${cmake_flags}"

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

    # 若构建选项（如 CUDA 是否可用）相比上次变化，强制重新配置。
    local need_reconfigure=0
    if [[ ! -f "${buildopts_file}" ]] || [[ "$(cat "${buildopts_file}" 2>/dev/null)" != "${cmake_flags}" ]]; then
        need_reconfigure=1
        echo "[INFO] Build options changed or new — forcing reconfigure"
    fi

    cd "${cache}"
    local native
    native="$(uname -m)"

    local cmake_cmd=(cmake -B build)
    # 把空格分隔的 flags 拆成独立参数（bash 默认 IFS 下安全）。
    # shellcheck disable=SC2206
    local flag_array=($cmake_flags)
    cmake_cmd+=("${flag_array[@]}")
    if [[ "${ARCH}" == "aarch64" && "${native}" != "aarch64" ]] && command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        cmake_cmd+=("-DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc")
        cmake_cmd+=("-DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++")
    fi
    cmake_cmd+=(.)

    if [[ "${need_reconfigure}" -eq 1 ]]; then
        rm -rf "${cache}/build"
        "${cmake_cmd[@]}"
        echo "${cmake_flags}" > "${buildopts_file}"
    fi

    cmake --build build --config Release -j"$(nproc)"

    # 必须用 build/bin/whisper-cli：同目录下还有 examples 的 main（十几 KB），find|head 会误选导致运行失败。
    local whisper_bin="${cache}/build/bin/whisper-cli"
    if [[ ! -x "${whisper_bin}" ]]; then
        echo "[ERROR] whisper-cli not found after build: ${whisper_bin}"
        exit 1
    fi
    # whisper-server：常驻后端用，模型只载入一次（v1.8.4 默认随 examples 一起编出）。
    local server_bin="${cache}/build/bin/whisper-server"

    if ! command -v patchelf >/dev/null 2>&1; then
        echo "[ERROR] patchelf is required to bundle whisper shared libs (e.g. sudo apt install patchelf)"
        exit 1
    fi

    # 复制所有构建产物 .so（whisper.cpp 产生的，包括新增的 libggml-cuda.so 等）。
    # find 比硬编码文件名列表更鲁棒——新增后端自动被复制。
    find "${cache}/build/src" -maxdepth 1 -name 'libwhisper.so*' -exec cp -a {} "${BIN_DIR}/" \;
    find "${cache}/build/ggml/src" -maxdepth 2 -name 'libggml*.so*' -exec cp -a {} "${BIN_DIR}/" \;

    cp -f "${whisper_bin}" "${target}"
    chmod +x "${target}"
    patchelf --set-rpath '$ORIGIN' "${target}"

    # 一并捆绑 whisper-server（与 whisper-cli 同级，resource.rs / whisper_server.rs 会查找）。
    if [[ -x "${server_bin}" ]]; then
        cp -f "${server_bin}" "${BIN_DIR}/whisper-server"
        chmod +x "${BIN_DIR}/whisper-server"
        patchelf --set-rpath '$ORIGIN' "${BIN_DIR}/whisper-server"
        echo "[OK] bundled whisper-server -> ${BIN_DIR}/whisper-server"
    else
        echo "[WARN] whisper-server not found at ${server_bin}; resident backend will fall back to whisper-cli"
    fi

    # 若启用了 CUDA，还需捆绑 CUDA runtime 动态库（NVIDIA EULA 允许随应用分发）。
    if [[ "${cmake_flags}" == *"GGML_CUDA=ON"* ]]; then
        echo "[INFO] Bundling CUDA runtime libraries..."
        bundle_cuda_libs
    fi

    # 对所有真实文件（跳过符号链接）统一设 $ORIGIN。
    find "${BIN_DIR}" -maxdepth 1 -name '*.so*' ! -type l -exec patchelf --set-rpath '$ORIGIN' {} \;

    cd "${REPO_ROOT}"
    echo "[OK] whisper-cli built from source"
}

# ─── Bundle NVIDIA CUDA runtime libraries ─────────────────────────────────────
# libcuda.so.1 是驱动库，不允许分发，且通过 -DGGML_CUDA_NO_VMM=ON 已去掉硬依赖；
# 以下三个 runtime 库允许随应用分发（NVIDIA CUDA Toolkit EULA）。
bundle_cuda_libs() {
    local cuda_lib_dirs=()
    local nvcc_dir
    nvcc_dir="$(command -v nvcc 2>/dev/null | xargs dirname | xargs dirname)"
    if [[ -n "${nvcc_dir}" ]]; then
        cuda_lib_dirs+=("${nvcc_dir}/lib64" "${nvcc_dir}/targets/x86_64-linux/lib")
    fi
    cuda_lib_dirs+=(/usr/local/cuda/lib64 /usr/local/cuda/targets/x86_64-linux/lib /usr/lib/x86_64-linux-gnu)
    local needed=(libcudart.so libcublas.so libcublasLt.so)
    for lib in "${needed[@]}"; do
        local found=""
        for dir in "${cuda_lib_dirs[@]}"; do
            if [[ -z "${dir}" ]]; then continue; fi
            local candidate=""
            # 优先复制带版本号的真实 so 文件（如 libcudart.so.12），保留符号链接关系。
            candidate="$(find "${dir}" -maxdepth 1 -name "${lib}"'*' -not -type l | sort -V | tail -1)"
            if [[ -n "${candidate}" && -f "${candidate}" ]]; then
                found="${candidate}"
                break
            fi
        done
        if [[ -n "${found}" ]]; then
            # 若已有同名文件，先移除再复制（支持升级）
            rm -f "${BIN_DIR}/$(basename "${found}")"
            cp -L "${found}" "${BIN_DIR}/"
            echo "[OK] bundled CUDA runtime: $(basename "${found}")"
        else
            echo "[WARN] CUDA runtime library ${lib}* not found — GPU backend may fail on target machine without it"
        fi
    done
}

# ─── whisper-cli (Linux) ─────────────────────────────────────────────────────
# Upstream GitHub releases no longer ship Linux whisper-cli binaries; build from source
# (archive download + cmake). Needs: cmake, C++ compiler, curl.
WHISPER_VERSION="${WHISPER_CPP_VERSION}"
WHISPER_TARGET="${BIN_DIR}/whisper-cli"

if [[ -f "${WHISPER_TARGET}" ]]; then
    echo "[OK] whisper-cli already exists at ${WHISPER_TARGET}"
else
    echo "[INFO] Building whisper-cli v${WHISPER_VERSION} from source (${ARCH})..."
    build_whisper_from_source "${WHISPER_TARGET}"
fi

echo "[OK] All dependencies ready in ${BIN_DIR}/"
