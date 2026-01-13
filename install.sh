#!/usr/bin/env bash
#
# 🚀 OMG Installer
# The fastest unified package manager for Arch Linux
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
#

set -u

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

# Detect directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IS_SOURCE_INSTALL=false
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    if grep -q 'name = "omg"' "$SCRIPT_DIR/Cargo.toml" 2>/dev/null; then
        IS_SOURCE_INSTALL=true
    fi
fi

# 🔄 UI Functions
spinner_pid=""

cleanup() {
    if [[ -n "$spinner_pid" ]]; then
        kill "$spinner_pid" >/dev/null 2>&1 || true
    fi
    tput cnorm # Show cursor
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
    tput civis # Hide cursor
    
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
    printf "  ${DIM}The unified DevOps platform for Arch Linux${RESET}\n\n"
}

# 🛡️ System Checks
check_arch() {
    header "Checking System"
    
    if [[ ! -f /etc/arch-release ]]; then
        error "OMG requires Arch Linux."
    fi
    success "Arch Linux detected"
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
    if [[ ! -f /usr/lib/libalpm.so ]]; then missing+=("pacman"); fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Missing dependencies: ${missing[*]}"
        printf "\n"
        read -p "$(printf "${BOLD}Install missing dependencies with sudo?${RESET} [Y/n] ")" -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Nn]$ ]]; then
            start_spinner "Installing dependencies"
            if sudo pacman -S --needed --noconfirm "${missing[@]}" base-devel >/dev/null 2>&1; then
                stop_spinner "Dependencies installed"
            else
                fail_spinner "Failed to install dependencies"
                error "Please install manually: sudo pacman -S ${missing[*]} base-devel"
            fi
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

# 🐚 Shell Setup
setup_shell() {
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

finish() {
    printf "\n"
    printf "${GREEN}${BOLD}Installation Complete! 🚀${RESET}\n"
    printf "\n"
    printf "${BOLD}Next Steps:${RESET}\n"
    printf "  1. Restart your terminal\n"
    printf "  2. Run ${CYAN}omg doctor${RESET} to verify setup\n"
    printf "  3. Try ${CYAN}omg run build${RESET} in a project\n"
    printf "\n"
}

# Run
main() {
    print_banner
    check_arch
    check_dependencies
    build_omg
    setup_config
    setup_shell
    finish
}

main
