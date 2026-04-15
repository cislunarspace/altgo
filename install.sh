#!/usr/bin/env bash
# altgo one-click installer for Linux
# Usage: ./install.sh [OPTIONS]
#
# Options:
#   --skip-rust       Skip Rust toolchain check
#   --skip-build      Skip building altgo
#   --skip-whisper    Skip whisper.cpp installation
#   --skip-model      Skip model download
#   --model <name>    Model size: tiny, base (default), small, medium, large
#   -h, --help        Show this help message

set -euo pipefail

# ─── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }
die()   { err "$@"; exit 1; }

# ─── Defaults ────────────────────────────────────────────────────────────────
SKIP_RUST=false
SKIP_BUILD=false
SKIP_WHISPER=false
SKIP_MODEL=false
MODEL_SIZE="base"
PROJECT_DIR=""

# ─── Parse arguments ─────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-rust)    SKIP_RUST=true; shift ;;
        --skip-build)   SKIP_BUILD=true; shift ;;
        --skip-whisper) SKIP_WHISPER=true; shift ;;
        --skip-model)   SKIP_MODEL=true; shift ;;
        --model)        MODEL_SIZE="${2:-}"; shift 2 ;;
        -h|--help)
            head -10 "$0" | tail -8
            exit 0
            ;;
        *) die "Unknown option: $1" ;;
    esac
done

# ─── Validate model size ────────────────────────────────────────────────────
declare -A MODEL_FILES=(
    [tiny]="ggml-tiny.bin"
    [base]="ggml-base.bin"
    [small]="ggml-small.bin"
    [medium]="ggml-medium.bin"
    [large]="ggml-large-v3.bin"
)
[[ -n "${MODEL_FILES[$MODEL_SIZE]:-}" ]] || die "Invalid model size: $MODEL_SIZE (valid: tiny, base, small, medium, large)"
MODEL_FILE="${MODEL_FILES[$MODEL_SIZE]}"
MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${MODEL_FILE}"

# ─── Resolve project directory ──────────────────────────────────────────────
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
[[ -f "${PROJECT_DIR}/Cargo.toml" ]] || die "Cargo.toml not found in ${PROJECT_DIR}. Run this script from the altgo project root."

DEPS_DIR="${PROJECT_DIR}/.deps"
BIN_DIR="${DEPS_DIR}/bin"
MODELS_DIR="${DEPS_DIR}/models"

info "Project directory: ${PROJECT_DIR}"
info "Dependencies directory: ${DEPS_DIR}"

mkdir -p "${BIN_DIR}" "${MODELS_DIR}"

# ─── Step 1: Check Rust toolchain ───────────────────────────────────────────
check_rust() {
    if [[ "$SKIP_RUST" == true ]]; then
        info "Skipping Rust toolchain check (--skip-rust)"
        return 0
    fi

    if command -v cargo &>/dev/null; then
        ok "Rust toolchain found: $(cargo --version)"
        return 0
    fi

    warn "Rust toolchain not found."
    echo ""
    echo "  Install Rust via rustup:"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "  Then restart your shell and re-run this script."
    echo ""
    read -rp "Install Rust now via rustup? [y/N] " answer
    if [[ "${answer,,}" == "y" ]]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "${HOME}/.cargo/env"
        ok "Rust installed: $(cargo --version)"
    else
        die "Rust toolchain is required to build altgo. Aborting."
    fi
}

# ─── Step 2: Check system dependencies (info only) ─────────────────────────
check_system_deps() {
    info "Checking system dependencies..."

    local missing=()
    local tools=("xinput" "xmodmap" "parecord" "notify-send")

    for tool in "${tools[@]}"; do
        if ! command -v "$tool" &>/dev/null; then
            missing+=("$tool")
        fi
    done

    # Check clipboard tool (at least one needed)
    if ! command -v xclip &>/dev/null && ! command -v xsel &>/dev/null && ! command -v wl-copy &>/dev/null; then
        missing+=("xclip (or xsel / wl-copy)")
    fi

    if [[ ${#missing[@]} -eq 0 ]]; then
        ok "All system dependencies are installed."
        return 0
    fi

    warn "Missing system dependencies: ${missing[*]}"
    echo ""
    echo "  Install them with your package manager:"
    echo ""
    echo "  Debian/Ubuntu:"
    echo "    sudo apt install xinput xdotool pulseaudio-utils xclip libnotify-bin"
    echo ""
    echo "  Arch Linux:"
    echo "    sudo pacman -S xorg-xinput xorg-xmodmap pulseaudio xclip libnotify"
    echo ""
    echo "  Fedora:"
    echo "    sudo dnf install xinput xmodmap pulseaudio-utils xclip libnotify"
    echo ""
    read -rp "Continue anyway? [y/N] " answer
    [[ "${answer,,}" == "y" ]] || die "Install missing dependencies first."
}

# ─── Step 3: Build altgo ───────────────────────────────────────────────────
build_altgo() {
    if [[ "$SKIP_BUILD" == true ]]; then
        info "Skipping altgo build (--skip-build)"
        return 0
    fi

    info "Building altgo (cargo build --release)..."
    (cd "${PROJECT_DIR}" && cargo build --release)
    cp "${PROJECT_DIR}/target/release/altgo" "${PROJECT_DIR}/altgo"
    ok "altgo built and copied to ${PROJECT_DIR}/altgo"
}

# ─── Step 4: Install whisper.cpp ────────────────────────────────────────────
install_whisper() {
    if [[ "$SKIP_WHISPER" == true ]]; then
        info "Skipping whisper.cpp installation (--skip-whisper)"
        return 0
    fi

    local whisper_bin="${BIN_DIR}/whisper-cli"

    # Already installed?
    if [[ -x "${whisper_bin}" ]]; then
        ok "whisper-cli already installed at ${whisper_bin}"
        return 0
    fi

    # Check if already on PATH
    if command -v whisper-cli &>/dev/null; then
        local system_bin
        system_bin="$(command -v whisper-cli)"
        info "Found whisper-cli on PATH at ${system_bin}, copying to ${whisper_bin}"
        cp "${system_bin}" "${whisper_bin}"
        chmod +x "${whisper_bin}"
        ok "whisper-cli installed from PATH."
        return 0
    fi

    # Need to compile from source (no prebuilt Linux binaries available)
    info "No prebuilt whisper-cli for Linux — compiling from source."

    # Check build tools
    local build_missing=()
    command -v gcc &>/dev/null || build_missing+=("gcc")
    command -v make &>/dev/null || build_missing+=("make")
    command -v git &>/dev/null || build_missing+=("git")

    if [[ ${#build_missing[@]} -gt 0 ]]; then
        die "Missing build tools: ${build_missing[*]}. Install with: sudo apt install build-essential git"
    fi

    local build_dir
    build_dir="$(mktemp -d /tmp/whisper-cpp-build.XXXXXX)"

    info "Cloning whisper.cpp into ${build_dir}..."
    git clone --depth 1 https://github.com/ggml-org/whisper.cpp.git "${build_dir}"

    info "Building whisper.cpp (this may take a few minutes)..."
    (cd "${build_dir}" && make -j"$(nproc)" whisper-cli)

    if [[ -x "${build_dir}/bin/whisper-cli" ]]; then
        cp "${build_dir}/bin/whisper-cli" "${whisper_bin}"
    elif [[ -x "${build_dir}/build/bin/whisper-cli" ]]; then
        cp "${build_dir}/build/bin/whisper-cli" "${whisper_bin}"
    elif [[ -x "${build_dir}/main" ]]; then
        # Older versions named the binary "main"
        cp "${build_dir}/main" "${whisper_bin}"
    else
        # Search for it
        local found
        found="$(find "${build_dir}" -name 'whisper-cli' -o -name 'main' | head -1)"
        if [[ -n "${found}" ]]; then
            cp "${found}" "${whisper_bin}"
        else
            die "Could not find compiled whisper-cli binary in ${build_dir}"
        fi
    fi

    chmod +x "${whisper_bin}"
    rm -rf "${build_dir}"
    ok "whisper-cli compiled and installed to ${whisper_bin}"
}

# ─── Step 5: Download model ─────────────────────────────────────────────────
download_model() {
    if [[ "$SKIP_MODEL" == true ]]; then
        info "Skipping model download (--skip-model)"
        return 0
    fi

    local model_path="${MODELS_DIR}/${MODEL_FILE}"

    if [[ -f "${model_path}" ]]; then
        ok "Model already exists: ${model_path}"
        return 0
    fi

    info "Downloading ${MODEL_SIZE} model (${MODEL_FILE}, ~$(model_size_human "${MODEL_SIZE}"))..."
    info "URL: ${MODEL_URL}"

    curl --fail --progress-bar -L -o "${model_path}.tmp" "${MODEL_URL}"
    mv "${model_path}.tmp" "${model_path}"

    # Verify file size (at least 10MB)
    local size
    size="$(stat -c%s "${model_path}" 2>/dev/null || stat -f%z "${model_path}" 2>/dev/null)"
    if [[ "${size}" -lt 10485760 ]]; then
        rm -f "${model_path}"
        die "Downloaded model is too small (${size} bytes). Download may have failed."
    fi

    ok "Model downloaded: ${model_path}"
}

model_size_human() {
    case "$1" in
        tiny)   echo "75MB" ;;
        base)   echo "142MB" ;;
        small)  echo "466MB" ;;
        medium) echo "1.5GB" ;;
        large)  echo "2.9GB" ;;
    esac
}

# ─── Step 6: Generate config ────────────────────────────────────────────────
generate_config() {
    local config_dir="${HOME}/.config/altgo"
    local config_path="${config_dir}/altgo.toml"

    # Compute absolute paths with forward slashes for TOML compatibility
    local whisper_path model_path
    whisper_path="$(cd "${PROJECT_DIR}" && realpath "${BIN_DIR}/whisper-cli")"
    model_path="$(cd "${PROJECT_DIR}" && realpath "${MODELS_DIR}/${MODEL_FILE}")"

    # If config already exists, prompt before overwriting
    if [[ -f "${config_path}" ]]; then
        warn "Config file already exists: ${config_path}"
        read -rp "Overwrite with new configuration? [y/N] " answer
        if [[ "${answer,,}" != "y" ]]; then
            info "Keeping existing config. Update whisper_path and model manually if needed:"
            echo "  whisper_path = \"${whisper_path}\""
            echo "  model = \"${model_path}\""
            return 0
        fi
        # Backup existing config
        cp "${config_path}" "${config_path}.bak"
        info "Backed up existing config to ${config_path}.bak"
    fi

    mkdir -p "${config_dir}"

    cat > "${config_path}" << EOF
# altgo configuration — generated by install.sh
# Edit as needed. See configs/altgo.toml for all available options.

[transcriber]
engine = "local"
model = "${model_path}"
whisper_path = "${whisper_path}"
language = "zh"
timeout_seconds = 30

[polisher]
level = "none"

[output]
enable_notify = true
EOF

    ok "Config written to ${config_path}"
}

# ─── Step 7: Verification ───────────────────────────────────────────────────
verify_install() {
    echo ""
    info "=== Installation Summary ==="
    echo ""

    if [[ -f "${PROJECT_DIR}/altgo" ]]; then
        ok "altgo binary: ${PROJECT_DIR}/altgo"
    else
        warn "altgo binary not found at ${PROJECT_DIR}/altgo"
    fi

    if [[ -x "${BIN_DIR}/whisper-cli" ]]; then
        ok "whisper-cli: ${BIN_DIR}/whisper-cli"
    else
        warn "whisper-cli not found at ${BIN_DIR}/whisper-cli"
    fi

    if [[ -f "${MODELS_DIR}/${MODEL_FILE}" ]]; then
        ok "Whisper model: ${MODELS_DIR}/${MODEL_FILE}"
    else
        warn "Whisper model not found at ${MODELS_DIR}/${MODEL_FILE}"
    fi

    local config_path="${HOME}/.config/altgo/altgo.toml"
    if [[ -f "${config_path}" ]]; then
        ok "Config file: ${config_path}"
    else
        warn "Config file not found at ${config_path}"
    fi

    echo ""
    info "To start altgo, run:"
    echo "  ${PROJECT_DIR}/altgo"
    echo ""
}

# ─── Main ────────────────────────────────────────────────────────────────────
main() {
    echo ""
    info "=== altgo Installer for Linux ==="
    echo ""

    check_rust
    check_system_deps
    build_altgo
    install_whisper
    download_model
    generate_config
    verify_install
}

main
