#!/usr/bin/env bash
#
# OMG - Oh My God! Package Manager
# The fastest unified package manager for Arch Linux
#
# Install script - run with:
#   curl -fsSL https://raw.githubusercontent.com/USER/omg/main/install.sh | bash
#
# Or clone and run:
#   git clone https://github.com/USER/omg && cd omg && ./install.sh
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
OMG_VERSION="${OMG_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/omg"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omg"

# Detect if we're in the source directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IS_SOURCE_INSTALL=false
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]] && grep -q 'name = "omg"' "$SCRIPT_DIR/Cargo.toml" 2>/dev/null; then
    IS_SOURCE_INSTALL=true
fi

print_banner() {
    echo -e "${CYAN}${BOLD}"
    cat << 'EOF'
   ____  __  __  ____
  / __ \|  \/  |/ ___|
 | |  | | |\/| | |  _
 | |__| | |  | | |_| |
  \____/|_|  |_|\____|

  The Fastest Package Manager for Arch Linux
EOF
    echo -e "${NC}"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

check_arch() {
    if [[ ! -f /etc/arch-release ]]; then
        error "OMG requires Arch Linux. Detected: $(uname -s)"
    fi
    success "Arch Linux detected"
}

check_dependencies() {
    local missing=()
    
    # Required for building
    command -v cargo &>/dev/null || missing+=("rust")
    command -v git &>/dev/null || missing+=("git")
    command -v pkg-config &>/dev/null || missing+=("pkgconf")
    
    # Required libraries
    if ! pkg-config --exists libarchive 2>/dev/null; then
        missing+=("libarchive")
    fi
    if ! pkg-config --exists openssl 2>/dev/null; then
        missing+=("openssl")
    fi
    
    # libalpm is part of pacman
    if [[ ! -f /usr/lib/libalpm.so ]]; then
        missing+=("pacman")
    fi
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Missing dependencies: ${missing[*]}"
        info "Installing dependencies..."
        sudo pacman -S --needed --noconfirm "${missing[@]}" base-devel
        success "Dependencies installed"
    else
        success "All dependencies satisfied"
    fi
}

build_from_source() {
    info "Building OMG from source..."
    
    if [[ "$IS_SOURCE_INSTALL" == true ]]; then
        cd "$SCRIPT_DIR"
    else
        # Clone the repository
        local tmp_dir=$(mktemp -d)
        trap "rm -rf $tmp_dir" EXIT
        
        info "Cloning repository..."
        git clone --depth 1 https://github.com/USER/omg.git "$tmp_dir"
        cd "$tmp_dir"
    fi
    
    # Build release binary
    info "Compiling (this may take a minute)..."
    cargo build --release 2>&1 | tail -5
    
    success "Build complete"
}

install_binary() {
    info "Installing to $INSTALL_DIR..."
    
    # Remove old binaries if they exist in other locations
    local old_locs=("/usr/local/bin/omg" "/usr/bin/omg" "$HOME/.cargo/bin/omg")
    for loc in "${old_locs[@]}"; do
        if [[ -f "$loc" ]]; then
            warn "Found old installation at $loc"
            if [[ -w "$(dirname "$loc")" ]]; then
                rm "$loc"
                success "Removed old binary at $loc"
            else
                info "Requesting sudo to remove $loc..."
                sudo rm "$loc" && success "Removed old binary at $loc" || warn "Failed to remove $loc"
            fi
        fi
    done
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Determine source path
    if [[ "$IS_SOURCE_INSTALL" == true ]]; then
        local src="$SCRIPT_DIR/target/release/omg"
    else
        local src="./target/release/omg"
    fi
    
    # Copy binary
    if [[ -f "$src" ]]; then
        cp "$src" "$INSTALL_DIR/omg"
        chmod +x "$INSTALL_DIR/omg"
        success "Installed omg to $INSTALL_DIR/omg"
    else
        error "Binary not found at $src"
    fi
    
    # Also install daemon if it exists
    local daemon_src="${src%omg}omgd"
    if [[ -f "$daemon_src" ]]; then
        cp "$daemon_src" "$INSTALL_DIR/omgd"
        chmod +x "$INSTALL_DIR/omgd"
        success "Installed omgd daemon"
    fi
}

setup_directories() {
    info "Setting up directories..."
    
    mkdir -p "$DATA_DIR"/{versions,cache,db}
    mkdir -p "$CONFIG_DIR"
    
    # Create default config if it doesn't exist
    if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
        cat > "$CONFIG_DIR/config.toml" << 'EOF'
# OMG Configuration
# See: omg config --help

[general]
# Use shell hooks (faster) or shims (more compatible)
use_shims = false

[security]
# Minimum security grade for installs: locked, verified, community, risk
minimum_grade = "community"

[cache]
# Cache TTL in hours
ttl_hours = 24
EOF
        success "Created default config at $CONFIG_DIR/config.toml"
    fi
    
    success "Directories ready"
}

setup_shell() {
    info "Configuring shell integration..."
    
    local shell_name=$(basename "$SHELL")
    local hook_line='eval "$(omg hook '"$shell_name"')"'
    local rc_file=""
    
    case "$shell_name" in
        bash)
            rc_file="$HOME/.bashrc"
            ;;
        zsh)
            rc_file="$HOME/.zshrc"
            ;;
        fish)
            rc_file="$HOME/.config/fish/config.fish"
            hook_line="omg hook fish | source"
            ;;
        *)
            warn "Unknown shell: $shell_name. Add manually: $hook_line"
            return
            ;;
    esac
    
    # Check if PATH includes install dir
    local path_line="export PATH=\"$INSTALL_DIR:\$PATH\""
    if [[ "$shell_name" == "fish" ]]; then
        path_line="fish_add_path $INSTALL_DIR"
    fi
    
    # Add to rc file if not already present
    if [[ -f "$rc_file" ]]; then
        # Ensure PATH includes install dir
        if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
            # Check if line already exists in file to avoid duplicates
            if ! grep -q "export PATH=\"$INSTALL_DIR" "$rc_file"; then
                 echo "" >> "$rc_file"
                 echo "$path_line" >> "$rc_file"
                 success "Added install dir to PATH in $rc_file"
            fi
        fi

        if ! grep -q "omg hook" "$rc_file" 2>/dev/null; then
            echo "" >> "$rc_file"
            echo "# OMG - Fast package manager" >> "$rc_file"
            echo "$hook_line" >> "$rc_file"
            success "Added shell hook to $rc_file"
        else
            success "Shell hook already configured in $rc_file"
        fi
    else
        warn "Shell config not found: $rc_file"
    fi
    
    # Install shell completions
    info "Installing shell completions..."
    "$INSTALL_DIR/omg" completions "$shell_name" 2>/dev/null || warn "Could not install completions"
}

verify_installation() {
    info "Verifying installation..."
    
    # Check if omg is accessible
    if [[ -x "$INSTALL_DIR/omg" ]]; then
        local version=$("$INSTALL_DIR/omg" --version 2>/dev/null || echo "unknown")
        success "OMG installed successfully: $version"
    else
        error "Installation verification failed"
    fi
}

print_next_steps() {
    echo ""
    echo -e "${GREEN}${BOLD}Installation complete!${NC}"
    echo ""
    echo -e "${BOLD}Next steps:${NC}"
    echo ""
    echo -e "  1. Restart your shell or run:"
    echo -e "     ${CYAN}source ~/.$(basename $SHELL)rc${NC}"
    echo ""
    echo -e "  2. Verify installation:"
    echo -e "     ${CYAN}omg --version${NC}"
    echo ""
    echo -e "  3. Try it out:"
    echo -e "     ${CYAN}omg search firefox${NC}      # Search packages (blazing fast!)"
    echo -e "     ${CYAN}omg status${NC}              # Show system status"
    echo -e "     ${CYAN}omg list node --available${NC}  # List Node.js versions"
    echo -e "     ${CYAN}omg use node 22${NC}         # Install & use Node.js 22"
    echo ""
    echo -e "  ${BOLD}Documentation:${NC} https://github.com/USER/omg"
    echo ""
}

uninstall() {
    info "Uninstalling OMG..."
    
    # Remove binaries
    rm -f "$INSTALL_DIR/omg" "$INSTALL_DIR/omgd"
    
    # Ask about data
    read -p "Remove data directory ($DATA_DIR)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$DATA_DIR"
        success "Removed data directory"
    fi
    
    # Ask about config
    read -p "Remove config directory ($CONFIG_DIR)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$CONFIG_DIR"
        success "Removed config directory"
    fi
    
    warn "Remember to remove the hook from your shell rc file"
    success "OMG uninstalled"
}

main() {
    # Handle uninstall
    if [[ "${1:-}" == "uninstall" ]] || [[ "${1:-}" == "--uninstall" ]]; then
        uninstall
        exit 0
    fi
    
    print_banner
    
    info "Starting OMG installation..."
    echo ""
    
    check_arch
    check_dependencies
    build_from_source
    install_binary
    setup_directories
    setup_shell
    verify_installation
    print_next_steps
}

main "$@"
