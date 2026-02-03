#!/usr/bin/env bash
#
# 🚀 OMG Installer
# The fastest unified package manager for all platforms
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
#
# Options (set before piping to bash):
#   OMG_NO_TELEMETRY=1  - Disable anonymous telemetry (no prompt)
#   OMG_SKIP_SHELL=1    - Skip shell integration
#   OMG_VERSION=v0.1.0  - Install specific version
#
# Example with no telemetry:
#   curl -fsSL https://... | OMG_NO_TELEMETRY=1 bash
#

set -u

# 🔒 Telemetry opt-out (set before running to skip prompt)
# Usage: OMG_NO_TELEMETRY=1 curl ... | bash
OMG_NO_TELEMETRY="${OMG_NO_TELEMETRY:-}"

# 🎨 Colors (Chalk-like style)
RESET='\033[0m'
BOLD='\033[1m'
DIM='\033[2m'
RED='\033[31m'
GREEN='\033[32m'
YELLOW='\033[33m'
BLUE='\033[34m'
MAGENTA='\033[35m'
CYAN='\033[36m'
BG_BLUE='\033[44m'
BG_RED='\033[41m'

# ⚙️ Configuration
OMG_VERSION="${OMG_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/omg"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omg"
REPO_URL="https://github.com/PyRo1121/omg.git"
REPO_OWNER="PyRo1121"
REPO_NAME="omg"

# Detect directory
SCRIPT_SOURCE="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR="$(cd "$(dirname "${SCRIPT_SOURCE}")" && pwd)"
IS_SOURCE_INSTALL=false
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    if grep -q 'name = "omg"' "$SCRIPT_DIR/Cargo.toml" 2>/dev/null; then
        IS_SOURCE_INSTALL=true
    fi
fi

# 🔄 UI Functions
spinner_pid=""
tmp_dir=""

tput_safe() {
    if command -v tput >/dev/null 2>&1 && [[ -n "${TERM-}" ]]; then
        tput "$@"
    fi
}

cleanup_tmp_dir() {
    if [[ -n "$tmp_dir" && -d "$tmp_dir" ]]; then
        rm -rf "$tmp_dir"
        tmp_dir=""
    fi
}

cleanup() {
    if [[ -n "$spinner_pid" ]]; then
        kill "$spinner_pid" >/dev/null 2>&1 || true
    fi
    cleanup_tmp_dir
    tput_safe cnorm # Show cursor
}

check_runtime_dependencies() {
    local missing=()
    local deps=("curl" "tar")

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            missing+=("$dep")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Missing runtime dependencies for prebuilt install: ${missing[*]}"
        return 1
    fi

    return 0
}

# 🌍 OS/Distro/Arch Detection Functions

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)
            # Check for WSL
            if grep -qi microsoft /proc/version 2>/dev/null; then
                echo "windows"
            else
                echo "linux"
            fi
            ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) echo "unknown" ;;
    esac
}

detect_distro() {
    local distro="unknown"
    
    if [[ -f /etc/os-release ]]; then
        # Source the file and extract ID
        # shellcheck disable=SC1091
        . /etc/os-release
        distro="${ID:-unknown}"
        
        # Normalize common distro names
        case "$distro" in
            ubuntu) distro="ubuntu" ;;
            debian) distro="debian" ;;
            arch) distro="arch" ;;
            fedora) distro="fedora" ;;
            rhel|centos) distro="fedora" ;; # Use Fedora binary for RHEL/CentOS
            *) distro="unknown" ;;
        esac
    fi
    
    echo "$distro"
}

detect_arch() {
    local machine
    machine="$(uname -m)"
    case "$machine" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64) echo "aarch64" ;;
        arm64) echo "aarch64" ;; # macOS uses arm64, normalize to aarch64
        i686|i386) echo "i686" ;;
        armv7l) echo "armv7l" ;;
        *) echo "$machine" ;;
    esac
}

select_artifact() {
    local version="$1"
    local os="$2"
    local distro="$3"
    local arch="$4"
    local asset_name=""
    
    case "$os" in
        linux)
            case "$distro" in
                arch|debian|ubuntu|fedora)
                    asset_name="omg-${version}-${arch}-linux-${distro}.tar.gz"
                    ;;
                *)
                    # Fallback to Fedora binary for unknown distros
                    warn "Unknown Linux distro '${distro}', using Fedora binary (pure Rust, most portable)"
                    asset_name="omg-${version}-${arch}-linux-fedora.tar.gz"
                    ;;
            esac
            ;;
        darwin)
            asset_name="omg-${version}-${arch}-darwin.tar.gz"
            ;;
        windows)
            asset_name="omg-${version}-${arch}-windows.zip"
            ;;
        *)
            return 1
            ;;
    esac
    
    echo "$asset_name"
}

fetch_release_json() {
    local api_base="https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases"

    if [[ "$OMG_VERSION" == "latest" ]]; then
        curl -fsSL "${api_base}/latest"
    else
        curl -fsSL "${api_base}/tags/${OMG_VERSION}"
    fi
}

install_from_release() {
    if ! check_runtime_dependencies; then
        return 1
    fi

    # Detect system info
    local detected_os
    local detected_distro
    local detected_arch
    detected_os=$(detect_os)
    detected_distro=$(detect_distro)
    detected_arch=$(detect_arch)

    # Use GitHub releases (always up-to-date)
    local release_json
    if ! release_json=$(fetch_release_json 2>/dev/null); then
        warn "Unable to fetch GitHub release metadata"
        return 1
    fi

    # Extract actual version tag from release JSON
    local actual_version
    actual_version=$(printf "%s" "$release_json" | grep -Eo '"tag_name"\s*:\s*"[^"]+"' | head -n1 | cut -d'"' -f4)
    if [[ -z "$actual_version" ]]; then
        warn "Unable to parse version from release metadata"
        return 1
    fi

    # Select correct artifact name
    local artifact_name
    artifact_name=$(select_artifact "$actual_version" "$detected_os" "$detected_distro" "$detected_arch")
    
    if [[ -z "$artifact_name" ]]; then
        warn "Unable to determine artifact name for ${detected_os}/${detected_distro}/${detected_arch}"
        return 1
    fi

    # Find download URL for the artifact
    local asset_url
    asset_url=$(printf "%s" "$release_json" \
        | grep -Eo '"browser_download_url"\s*:\s*"[^"]+"' \
        | cut -d '"' -f4 \
        | grep -F "$artifact_name" \
        | head -n1)

    if [[ -z "$asset_url" ]]; then
        warn "No prebuilt binary found for ${detected_os}/${detected_distro}/${detected_arch} (artifact: ${artifact_name})"
        return 1
    fi

    header "Installing Prebuilt OMG"
    info "Platform: ${detected_os}/${detected_distro}/${detected_arch}"
    tmp_dir=$(mktemp -d)
    trap 'cleanup_tmp_dir' RETURN

    start_spinner "Downloading prebuilt binary"
    local download_file="$tmp_dir/omg-release"
    if [[ "$artifact_name" == *.zip ]]; then
        download_file="$tmp_dir/omg-release.zip"
    else
        download_file="$tmp_dir/omg-release.tar.gz"
    fi
    
    if curl -fsSL "$asset_url" -o "$download_file" >/dev/null 2>&1; then
        stop_spinner "Download complete"
    else
        fail_spinner "Download failed"
        return 1
    fi

    start_spinner "Extracting binaries"
    if [[ "$artifact_name" == *.zip ]]; then
        # Handle Windows .zip files
        if command -v unzip >/dev/null 2>&1; then
            if unzip -q "$download_file" -d "$tmp_dir" >/dev/null 2>&1; then
                stop_spinner "Extraction complete"
            else
                fail_spinner "Extraction failed"
                return 1
            fi
        else
            fail_spinner "unzip not found (required for Windows binaries)"
            return 1
        fi
    else
        # Handle .tar.gz files
        if tar -xzf "$download_file" -C "$tmp_dir" >/dev/null 2>&1; then
            stop_spinner "Extraction complete"
        else
            fail_spinner "Extraction failed"
            return 1
        fi
    fi

    local omg_path
    local omgd_path
    omg_path=$(find "$tmp_dir" -maxdepth 3 -type f -name omg | head -n1)
    omgd_path=$(find "$tmp_dir" -maxdepth 3 -type f -name omgd | head -n1)

    if [[ -z "$omg_path" ]]; then
        warn "Prebuilt archive missing omg binary"
        return 1
    fi

    mkdir -p "$INSTALL_DIR"
    cp "$omg_path" "$INSTALL_DIR/omg"
    if [[ -n "$omgd_path" ]]; then
        cp "$omgd_path" "$INSTALL_DIR/omgd"
    fi
    chmod +x "$INSTALL_DIR/omg" "$INSTALL_DIR/omgd" 2>/dev/null || true

    success "Installed prebuilt binaries to $INSTALL_DIR"
    return 0
}

trap cleanup EXIT

info() {
    printf "${BLUE}${BOLD}info${RESET} %s\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}success${RESET} %s\n" "$1"
}

warn() {
    printf "${YELLOW}${BOLD}warn${RESET} %s\n" "$1"
}

error() {
    printf "${RED}${BOLD}error${RESET} %s\n" "$1"
    exit 1
}

header() {
    printf "\n${BOLD}${MAGENTA}==>${RESET} ${BOLD}%s${RESET}\n" "$1"
}

start_spinner() {
    local msg="$1"
    tput_safe civis # Hide cursor
    
    (
        local chars="⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
        while :; do
            for (( i=0; i<${#chars}; i++ )); do
                local c="${chars:$i:1}"
                printf "\r${CYAN}${c}${RESET} %s..." "$msg"
                sleep 0.1
            done
        done
    ) &
    spinner_pid=$!
}

stop_spinner() {
    if [[ -n "$spinner_pid" ]]; then
        kill "$spinner_pid" >/dev/null 2>&1 || true
        wait "$spinner_pid" >/dev/null 2>&1 || true
        spinner_pid=""
    fi
    tput cnorm # Show cursor
    printf "\r${GREEN}✓${RESET} %s\n" "$1"
}

fail_spinner() {
    if [[ -n "$spinner_pid" ]]; then
        kill "$spinner_pid" >/dev/null 2>&1 || true
        wait "$spinner_pid" >/dev/null 2>&1 || true
        spinner_pid=""
    fi
    tput cnorm # Show cursor
    printf "\r${RED}✗${RESET} %s\n" "$1"
}

print_banner() {
    clear
    printf "${MAGENTA}${BOLD}"
    cat << 'EOF'
    ____  __  __  ____ 
   / __ \|  \/  |/ ___|
  | |  | | |\/| | |  _ 
  | |__| | |  | | |_| |
   \____/|_|  |_|\____|
EOF
    printf "${RESET}\n"
    printf "  ${DIM}The unified package manager for all platforms${RESET}\n\n"
}

# 🛡️ System Checks
check_platform() {
    header "Checking System"
    
    local detected_os
    local detected_distro
    local detected_arch
    detected_os=$(detect_os)
    detected_distro=$(detect_distro)
    detected_arch=$(detect_arch)
    
    info "Detected OS: ${detected_os}"
    info "Detected Distro: ${detected_distro}"
    info "Detected Architecture: ${detected_arch}"
    
    case "$detected_os" in
        linux)
            case "$detected_distro" in
                arch|debian|ubuntu|fedora)
                    success "Supported platform detected"
                    ;;
                unknown)
                    warn "Unknown Linux distro detected - will use Fedora binary (pure Rust, most portable)"
                    ;;
                *)
                    warn "Untested platform - attempting installation with Fedora binary"
                    ;;
            esac
            ;;
        darwin)
            success "macOS detected"
            ;;
        windows)
            success "Windows/WSL detected"
            ;;
        *)
            error "Unsupported platform: ${detected_os}. Please file an issue at https://github.com/PyRo1121/omg/issues"
            ;;
    esac
}

check_dependencies() {
    local missing=()
    local deps=("git" "cargo" "pkg-config" "gcc")
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            missing+=("$dep")
        fi
    done

    if ! pkg-config --exists libarchive 2>/dev/null; then missing+=("libarchive"); fi
    if ! pkg-config --exists openssl 2>/dev/null; then missing+=("openssl"); fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Missing dependencies: ${missing[*]}"
        printf "\n"
        read -p "$(printf "${BOLD}Install missing dependencies with sudo?${RESET} [Y/n] ")" -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Nn]$ ]]; then
            start_spinner "Installing dependencies"
            
            local detected_os
            detected_os=$(detect_os)
            
            case "$detected_os" in
                linux)
                    local detected_distro
                    detected_distro=$(detect_distro)
                    case "$detected_distro" in
                        arch)
                            if sudo pacman -S --needed --noconfirm "${missing[@]}" base-devel >/dev/null 2>&1; then
                                stop_spinner "Dependencies installed"
                            else
                                fail_spinner "Failed to install dependencies"
                                error "Please install manually: sudo pacman -S ${missing[*]} base-devel"
                            fi
                            ;;
                        debian|ubuntu)
                            if sudo apt-get update >/dev/null 2>&1 && sudo apt-get install -y "${missing[@]}" >/dev/null 2>&1; then
                                stop_spinner "Dependencies installed"
                            else
                                fail_spinner "Failed to install dependencies"
                                error "Please install manually: sudo apt-get install ${missing[*]}"
                            fi
                            ;;
                        fedora)
                            if sudo dnf install -y "${missing[@]}" >/dev/null 2>&1; then
                                stop_spinner "Dependencies installed"
                            else
                                fail_spinner "Failed to install dependencies"
                                error "Please install manually: sudo dnf install ${missing[*]}"
                            fi
                            ;;
                        *)
                            fail_spinner "Unknown package manager"
                            error "Please install dependencies manually: ${missing[*]}"
                            ;;
                    esac
                    ;;
                darwin)
                    if command -v brew >/dev/null 2>&1; then
                        if brew install "${missing[@]}" >/dev/null 2>&1; then
                            stop_spinner "Dependencies installed"
                        else
                            fail_spinner "Failed to install dependencies"
                            error "Please install manually: brew install ${missing[*]}"
                        fi
                    else
                        fail_spinner "Homebrew not found"
                        error "Please install Homebrew first: https://brew.sh"
                    fi
                    ;;
                *)
                    fail_spinner "Unknown OS"
                    error "Please install dependencies manually: ${missing[*]}"
                    ;;
            esac
        else
            error "Dependencies required to proceed."
        fi
    else
        success "All dependencies satisfied"
    fi
}

# 🏗️ Build & Install
build_omg() {
    header "Building OMG"
    
    local work_dir
    
    if [[ "$IS_SOURCE_INSTALL" == "true" ]]; then
        work_dir="$SCRIPT_DIR"
        info "Installing from source directory"
    else
        work_dir=$(mktemp -d)
        trap 'rm -rf "$work_dir"' EXIT
        
        start_spinner "Cloning repository"
        if git clone --depth 1 "$REPO_URL" "$work_dir" >/dev/null 2>&1; then
            stop_spinner "Repository cloned"
        else
            fail_spinner "Failed to clone repository"
            exit 1
        fi
    fi

    cd "$work_dir"
    
    export RUSTFLAGS="-C target-cpu=native"
    start_spinner "Compiling binary (release)"
    if cargo build --release --quiet >/dev/null 2>&1; then
        stop_spinner "Build successful"
    else
        fail_spinner "Build failed"
        printf "\n${RED}Build output:${RESET}\n"
        cargo build --release
        exit 1
    fi

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "target/release/omg" "$INSTALL_DIR/"
    if [[ -f "target/release/omgd" ]]; then
        cp "target/release/omgd" "$INSTALL_DIR/"
    fi
    chmod +x "$INSTALL_DIR/omg"
    
    success "Installed to $INSTALL_DIR/omg"
}

# ⚙️ Configuration
setup_config() {
    header "Configuration"
    
    mkdir -p "$DATA_DIR"/{versions,cache,db}
    mkdir -p "$CONFIG_DIR"

    if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
        cat > "$CONFIG_DIR/config.toml" << 'EOF'
[general]
use_shims = false

[security]
minimum_grade = "community"

[cache]
ttl_hours = 24
EOF
        success "Default config created"
    else
        info "Config already exists"
    fi
}

# 🔒 Telemetry Setup
setup_telemetry() {
    header "Privacy & Telemetry"

    # Check if already opted out via environment variable
    if [[ "$OMG_NO_TELEMETRY" == "1" ]]; then
        info "Telemetry disabled via OMG_NO_TELEMETRY=1"
        set_telemetry_opt_out
        return
    fi

    # Show privacy disclosure
    printf "\\n${BOLD}Data Collection Disclosure:${RESET}\\n"
    printf "  OMG collects ${BOLD}anonymous${RESET} usage data to improve the product:\\n"
    printf "  • One-time install ping (version, platform, random UUID)\\n"
    printf "  • Command usage statistics (what commands you run)\\n"
    printf "  • Error reports (helps us fix bugs)\\n"
    printf "\\n"
    printf "  ${DIM}No personal information, file contents, or package names collected.${RESET}\\n"
    printf "  ${DIM}Data is sent to api.pyro1121.com. You can opt out at any time.${RESET}\\n"
    printf "\\n"

    # Ask for consent
    read -p "$(printf "${BOLD}Allow anonymous telemetry to help improve OMG?${RESET} [Y/n] ")" -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Nn]$ ]]; then
        set_telemetry_opt_out
        success "Telemetry disabled. You can re-enable with: unset OMG_TELEMETRY"
    else
        success "Telemetry enabled. Thank you for helping improve OMG!"
        printf "  ${DIM}Opt out anytime with: export OMG_TELEMETRY=0${RESET}\\n"
    fi
}

# Helper to set telemetry opt-out in shell config
set_telemetry_opt_out() {
    local shell_type=$(basename "$SHELL")
    local rc_file=""
    
    case "$shell_type" in
        bash) rc_file="$HOME/.bashrc" ;;
        zsh)  rc_file="$HOME/.zshrc" ;;
        fish) rc_file="$HOME/.config/fish/config.fish" ;;
        *)    warn "Unsupported shell: $shell_type"; return ;;
    esac

    if [[ -f "$rc_file" ]]; then
        if ! grep -q "OMG_TELEMETRY" "$rc_file"; then
            echo >> "$rc_file"
            echo "# OMG Telemetry opt-out" >> "$rc_file"
            if [[ "$shell_type" == "fish" ]]; then
                echo "set -gx OMG_TELEMETRY 0" >> "$rc_file"
            else
                echo "export OMG_TELEMETRY=0" >> "$rc_file"
            fi
        fi
    fi
}

# 🐚 Shell Setup
setup_shell() {
    if [[ "${OMG_SKIP_SHELL:-0}" == "1" ]]; then
        info "Skipping shell integration (OMG_SKIP_SHELL=1)"
        return
    fi

    header "Shell Integration"
    
    local shell_type=$(basename "$SHELL")
    local rc_file=""
    
    case "$shell_type" in
        bash) rc_file="$HOME/.bashrc" ;;
        zsh)  rc_file="$HOME/.zshrc" ;;
        fish) rc_file="$HOME/.config/fish/config.fish" ;;
        *)    warn "Unsupported shell: $shell_type"; return ;;
    esac

    # Ensure PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        if [[ -f "$rc_file" ]]; then
            if ! grep -q "export PATH=\"$INSTALL_DIR" "$rc_file"; then
                if [[ "$shell_type" == "fish" ]]; then
                    echo "fish_add_path $INSTALL_DIR" >> "$rc_file"
                else
                    echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$rc_file"
                fi
                success "Added $INSTALL_DIR to PATH in $rc_file"
            fi
        fi
    fi

    # Ensure Hook
    if [[ -f "$rc_file" ]]; then
        if ! grep -q "omg hook" "$rc_file"; then
            echo >> "$rc_file"
            echo "# OMG Package Manager" >> "$rc_file"
            if [[ "$shell_type" == "fish" ]]; then
                echo "omg hook fish | source" >> "$rc_file"
            else
                echo 'eval "$(omg hook '"$shell_type"')"' >> "$rc_file"
            fi
            success "Added hook to $rc_file"
        else
            info "Hook already present"
        fi
    fi
    
    # Generate completions
    "$INSTALL_DIR/omg" completions "$shell_type" >/dev/null 2>&1 || true
}

setup_turbo() {
    local detected_os
    detected_os=$(detect_os)
    
    if [[ "$detected_os" != "linux" ]]; then
        return
    fi

    header "Turbo Mode (Recommended)"
    
    printf "\\n${BOLD}What is Turbo Mode?${RESET}\\n"
    printf "  Turbo mode enables instant package operations without sudo prompts.\\n"
    printf "  It uses Linux capabilities to grant omg permission to manage packages.\\n"
    printf "\\n"
    printf "  ${DIM}Benefits:${RESET}\\n"
    printf "  • No sudo password prompts for install/update/remove\\n"
    printf "  • 40x faster privilege elevation (5ms vs 200ms)\\n"
    printf "  • Works in scripts and automation without NOPASSWD\\n"
    printf "\\n"

    read -p "$(printf "${BOLD}Enable turbo mode now?${RESET} [Y/n] ")" -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        start_spinner "Enabling turbo mode"
        if sudo setcap 'cap_dac_override,cap_fowner,cap_chown+ep' "$INSTALL_DIR/omg" >/dev/null 2>&1; then
            stop_spinner "Turbo mode enabled"
            success "Package operations now work without sudo!"
        else
            fail_spinner "Failed to enable turbo mode"
            warn "You can enable it later with: omg doctor --turbo"
        fi
    else
        info "Skipped. Enable later with: omg doctor --turbo"
    fi
}

finish() {
    printf "\n"
    printf "${GREEN}${BOLD}Installation Complete! 🚀${RESET}\n"
    printf "\n"
    printf "${BOLD}Next Steps:${RESET}\n"
    printf "  1. Restart your terminal\n"
    printf "  2. Run ${CYAN}omg doctor${RESET} to verify setup\n"
    printf "  3. Try ${CYAN}omg search firefox${RESET} to test\n"
    printf "\n"
}

# Run
main() {
    print_banner
    if ! install_from_release; then
        check_platform
        check_dependencies
        build_omg
    fi
    setup_config
    setup_telemetry
    setup_shell
    setup_turbo
    finish
}

main
