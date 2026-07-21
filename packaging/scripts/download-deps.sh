#!/usr/bin/env bash
# Download platform dependencies for altgo packaging.
# Usage: ./scripts/download-deps.sh [x86_64|aarch64]
#
# Builds whisper-cli / whisper-server (with optional CUDA backend) into target/deps/bin/.

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

        # 查找 cublas 库和头文件（CUDA 12/13 可能安装在非标准路径）
        local cublas_lib="${CUDA_cublas_LIBRARY:-}"
        if [ -z "$cublas_lib" ]; then
            # 查找 libcublas.so（支持 CUDA 12 和 13 的路径）
            cublas_lib="$(find /usr/lib/x86_64-linux-gnu/libcublas /usr/local/cuda* -name "libcublas.so" 2>/dev/null | head -1)"
            # 如果没找到，尝试版本化的符号链接
            if [ -z "$cublas_lib" ]; then
                for ver in 12 13; do
                    local lib="/usr/lib/x86_64-linux-gnu/libcublas.so.${ver}"
                    if [ -f "$lib" ]; then
                        cublas_lib="$lib"
                        break
                    fi
                done
            fi
        fi
        if [ -n "$cublas_lib" ]; then
            echo "[INFO] Found cublas at: $cublas_lib" >&2
            flags="${flags} -DCUDA_cublas_LIBRARY=${cublas_lib}"
        fi
        # 查找 cublas 头文件
        local cublas_inc=""
        for dir in /usr/include/libcublas/13 /usr/include/libcublas/12 /usr/local/cuda/include /usr/include/cuda; do
            if [ -f "$dir/cublas_v2.h" ]; then
                cublas_inc="$dir"
                break
            fi
        done
        if [ -n "$cublas_inc" ]; then
            echo "[INFO] Found cublas headers at: $cublas_inc" >&2
            flags="${flags} -DCMAKE_CUDA_FLAGS=-I${cublas_inc}"
        fi
    else
        echo "[INFO] CUDA not detected — CPU-only build" >&2
    fi

    # OpenBLAS：modest encoder-only speedup；暂不启用，因需额外构建依赖且增益有限。
    # 如需，可在此加：-DGGML_BLAS=ON -DGGML_BLAS_VENDOR=OpenBLAS

    printf '%s' "${flags# }"
}

# ─── 把目录里的 .so 精简成"单文件、文件名 = SONAME"形态 ─────────────────────
# whisper.cpp / ggml 构建产物是 libfoo.so → libfoo.so.X → libfoo.so.X.Y.Z 三层软链，
# 同一份数据被引用三次。Tauri 打包时会解引用软链，把每个 .so 复制成三份独立副本，
# 把包体积撑大三倍（libggml-cuda 单份 131M → 三份 393M）。
#
# 链接器加载库时只认内嵌 SONAME（如 libwhisper.so.1），按该名字在 rpath 目录查找；
# 所以只保留"名为 SONAME 的真文件"即可，软链和完整版本号文件都是多余的。
trim_so_to_soname() {
    local dir="$1"
    local changed=0
    # 先收集所有真文件（非软链）的 SONAME，把真文件重命名为 SONAME 名。
    while IFS= read -r -d '' so; do
        local soname
        soname="$(patchelf --print-soname "${so}" 2>/dev/null || true)"
        [[ -z "${soname}" ]] && continue
        local target="${dir}/${soname}"
        if [[ "${so}" != "${target}" ]]; then
            mv -f "${so}" "${target}"
            changed=1
        fi
    done < <(find "${dir}" -maxdepth 1 -name '*.so*' ! -type l -print0)
    # 真文件就位后，剩下的 .so* 都是软链（指向已不存在的原名），全部删掉。
    find "${dir}" -maxdepth 1 -name '*.so*' -type l -delete
    if [[ "${changed}" -eq 1 ]]; then
        echo "[OK] trimmed .so symlinks (Tauri would otherwise triplicate them)"
    fi
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

    # 若构建选项（如 CUDA 是否可用）相比上次变化，或 CMakeCache 不存在，强制重新配置。
    local need_reconfigure=0
    if [[ ! -f "${cache}/build/CMakeCache.txt" ]]; then
        need_reconfigure=1
        echo "[INFO] CMakeCache not found — forcing reconfigure"
    elif [[ ! -f "${buildopts_file}" ]] || [[ "$(cat "${buildopts_file}" 2>/dev/null)" != "${cmake_flags}" ]]; then
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

    # 把每个 .so 精简成"单文件、文件名 = SONAME"形态：
    # whisper.cpp 默认产出 libfoo.so → libfoo.so.X → libfoo.so.X.Y.Z 三层软链。
    # Tauri 打包时会解引用软链，把同一份 .so 复制成三份独立副本——libggml-cuda
    # 单份 131M，三份就是 393M，把 deb 撑到 340M+。
    # 链接器加载时只看内嵌 SONAME（如 libwhisper.so.1），按该名字在 rpath 查找；
    # 所以只保留"名为 SONAME 的真文件"即可，软链和完整版本号文件都是多余的。
    trim_so_to_soname "${BIN_DIR}"

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

    # CUDA runtime（libcublas / libcublasLt / libcudart）不随包分发：
    # - 这些库很大（libcublasLt 单份就近 500M），打成 deb 会把包撑到 1.5G。
    # - libggml-cuda.so 通过 dlopen 按需加载，未安装 CUDA 的机器加载失败会自动回退 CPU。
    # - 有 NVIDIA 显卡的用户自行安装 CUDA runtime 即可启用 GPU 加速。

    # 对所有真实文件（跳过符号链接）统一设 $ORIGIN。
    find "${BIN_DIR}" -maxdepth 1 -name '*.so*' ! -type l -exec patchelf --set-rpath '$ORIGIN' {} \;

    cd "${REPO_ROOT}"
    echo "[OK] whisper-cli built from source"
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
