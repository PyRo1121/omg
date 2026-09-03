#!/usr/bin/env bash
#
# 🚀 OMG Installer
# The fastest unified package manager for all platforms
# Canonical source: this file. The omg-web production copy must remain byte-identical.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
#
# Options (set before piping to bash):
#   OMG_NO_TELEMETRY=1  - Disable anonymous telemetry (no prompt)
#   OMG_SKIP_SHELL=1    - Skip shell integration
#   OMG_VERSION=v0.1.0  - Install specific version
#
# Uninstall:
#   curl -fsSL https://... | bash -s -- --uninstall
# (removes binaries and shell integration; rc files are backed up first)
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

# Ask a yes/no question. Honors the documented default when stdin is not a
# terminal (e.g. `curl ... | bash`), so automation never hangs or gets prompted.
ask_yes_no() {
  local prompt="$1"
  local default="${2:-y}"
  local reply=""

  if [[ ! -t 0 ]]; then
    [[ "$default" == "y" ]]
    return
  fi

  local hint
  if [[ "$default" == "y" ]]; then
    hint="[Y/n]"
  else
    hint="[y/N]"
  fi

  read -r -p "${prompt} ${hint} " reply || return 1
  reply="${reply:-$default}"
  [[ "$reply" =~ ^[Yy]$ ]]
}

# Install a binary atomically: copy to a temp name, chmod, then rename over
# the destination so an interrupted install never leaves a truncated binary.
install_binary() {
  local src="$1"
  local dst="$2"
  local tmp_dst="${dst}.tmp.$$"

  if [[ ! -f "$src" ]]; then
    warn "Skipping $(basename "$dst"): binary missing from install source"
    return 1
  fi

  if ! cp "$src" "$tmp_dst"; then
    rm -f "$tmp_dst"
    error "Failed to copy $(basename "$src") to $INSTALL_DIR"
  fi
  chmod +x "$tmp_dst"
  if ! mv -f "$tmp_dst" "$dst"; then
    rm -f "$tmp_dst"
    error "Failed to install $dst"
  fi
}

check_runtime_dependencies() {
  local missing=()
  local deps=("curl" "tar")

  for dep in "${deps[@]}"; do
    if ! command -v "$dep" >/dev/null 2>&1; then
      missing+=("$dep")
    fi
  done
  if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
    missing+=("sha256sum or shasum")
  fi

  if [[ ${#missing[@]} -gt 0 ]]; then
    warn "Missing runtime dependencies for prebuilt install: ${missing[*]}"
    return 1
  fi

  return 0
}

calculate_sha256() {
  local file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  else
    shasum -a 256 "$file" | awk '{print $1}'
  fi
}

# 🌍 OS/Distro/Arch Detection Functions

detect_os() {
  local os
  os="$(uname -s)"
  case "$os" in
  Linux*) echo "linux" ;;
  Darwin*) echo "darwin" ;;
  MINGW* | MSYS* | CYGWIN*) echo "unsupported-windows" ;;
  *) echo "unknown" ;;
  esac
}

detect_distro() {
  local os_release_file="${1:-/etc/os-release}"
  local distro="unknown"
  local id_like=""
  local ID=""
  local ID_LIKE=""

  if [[ -f "$os_release_file" ]]; then
    # shellcheck disable=SC1090
    . "$os_release_file"
    distro="${ID:-unknown}"
    id_like=" ${ID_LIKE:-} "

    case "$distro" in
    ubuntu | debian | arch | fedora) ;;
    rhel | centos) distro="fedora" ;;
    *)
      case "$id_like" in
      *" arch "*) distro="arch" ;;
      *" ubuntu "*) distro="ubuntu" ;;
      *" debian "*) distro="debian" ;;
      *" fedora "* | *" rhel "* | *" centos "*) distro="fedora" ;;
      *) distro="unknown" ;;
      esac
      ;;
    esac
  fi

  echo "$distro"
}

detect_arch() {
  local machine
  machine="$(uname -m)"
  case "$machine" in
  x86_64 | amd64) echo "x86_64" ;;
  aarch64) echo "aarch64" ;;
  arm64) echo "aarch64" ;; # macOS uses arm64, normalize to aarch64
  i686 | i386) echo "i686" ;;
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
    arch | debian | ubuntu | fedora)
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
  elif [[ "$OMG_VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    curl -fsSL "${api_base}/tags/${OMG_VERSION}"
  else
    warn "OMG_VERSION must be 'latest' or a version tag such as v1.2.3"
    return 1
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

  if [[ "$detected_os" == "darwin" && "$detected_arch" == "x86_64" ]]; then
    error "Intel macOS is unsupported. OMG supports macOS releases on Apple Silicon (aarch64)."
  fi

  # Use GitHub releases (always up-to-date)
  local release_json
  if ! release_json=$(fetch_release_json 2>/dev/null); then
    warn "Unable to fetch GitHub release metadata"
    return 1
  fi

  # Extract actual version tag from release JSON
  local actual_version
  actual_version=$(printf "%s" "$release_json" | grep -Eo '"tag_name"\s*:\s*"[^"]+"' | head -n1 | cut -d'"' -f4)
  if [[ ! "$actual_version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    warn "Release metadata returned an invalid version tag"
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
  local asset_url=""
  while IFS= read -r candidate; do
    case "$candidate" in
    *"/$artifact_name")
      asset_url="$candidate"
      break
      ;;
    esac
  done < <(
    printf "%s" "$release_json" |
      grep -Eo '"browser_download_url"\s*:\s*"[^"]+"' |
      cut -d '"' -f4
  )

  if [[ -z "$asset_url" ]]; then
    warn "No prebuilt binary found for ${detected_os}/${detected_distro}/${detected_arch} (artifact: ${artifact_name})"
    return 1
  fi
  case "$asset_url" in
  "https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/"*) ;;
  *)
    warn "Release metadata returned an unexpected download origin"
    return 1
    ;;
  esac

  header "Installing Prebuilt OMG"
  info "Platform: ${detected_os}/${detected_distro}/${detected_arch}"
  tmp_dir=$(mktemp -d)
  trap 'cleanup_tmp_dir' RETURN

  # Save under the real artifact name so the published .sha256 sidecar
  # (which references the archive by name) verifies in place.
  start_spinner "Downloading prebuilt binary"
  local download_file="$tmp_dir/$artifact_name"

  if curl -fsSL "$asset_url" -o "$download_file" >/dev/null 2>&1; then
    stop_spinner "Download complete"
  else
    fail_spinner "Download failed"
    return 1
  fi

  # Verify against the release's .sha256 sidecar without trusting the
  # sidecar's filename field as a filesystem path. Note the trust limit:
  # sidecar and artifact share one origin, so this proves integrity
  # against a corrupted download, not against a compromised release.
  # Sigstore attestation is checked by `omg self-update`, not here.
  start_spinner "Verifying checksum"
  if curl -fsSL "${asset_url}.sha256" -o "${download_file}.sha256" >/dev/null 2>&1; then
    local expected_checksum
    local actual_checksum
    expected_checksum=$(awk 'NR == 1 { print $1 }' "${download_file}.sha256")
    if [[ ! "$expected_checksum" =~ ^[0-9a-f]{64}$ ]]; then
      fail_spinner "Checksum verification failed"
      warn "Published checksum for ${artifact_name} is malformed"
      return 1
    fi
    actual_checksum=$(calculate_sha256 "$download_file")
    if [[ "$actual_checksum" == "$expected_checksum" ]]; then
      stop_spinner "Checksum verified"
    else
      fail_spinner "Checksum verification failed"
      warn "Downloaded ${artifact_name} does not match its published sha256"
      return 1
    fi
  else
    fail_spinner "Checksum unavailable"
    warn "No .sha256 sidecar published for ${artifact_name}; refusing to install unverified binaries"
    return 1
  fi

  # Provenance gate: every release archive carries a Sigstore build-provenance
  # attestation generated by GitHub Actions. Verify it when the GitHub CLI is
  # available; without it, warn loudly instead of failing hard (gh is not
  # installed in many minimal environments, and the checksum gate above is
  # already mandatory).
  if command -v gh >/dev/null 2>&1; then
    start_spinner "Verifying build provenance"
    if gh attestation verify "$download_file" -R "${REPO_OWNER}/${REPO_NAME}" >/dev/null 2>&1; then
      stop_spinner "Build provenance verified"
    else
      fail_spinner "Build provenance verification failed"
      warn "Sigstore attestation verification failed for ${artifact_name}"
      warn "Possible supply-chain tampering — run manually: gh attestation verify <archive> -R ${REPO_OWNER}/${REPO_NAME}"
      return 1
    fi
  else
    info "GitHub CLI not found; skipping provenance check (install gh to enable)"
  fi

  start_spinner "Extracting binaries"
  if tar -xzf "$download_file" -C "$tmp_dir" >/dev/null 2>&1; then
    stop_spinner "Extraction complete"
  else
    fail_spinner "Extraction failed"
    return 1
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
  install_binary "$omg_path" "$INSTALL_DIR/omg" || return 1
  if [[ -n "$omgd_path" ]]; then
    install_binary "$omgd_path" "$INSTALL_DIR/omgd" || return 1
  else
    info "Prebuilt archive does not include omgd; skipping daemon install"
  fi

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
  printf "${YELLOW}${BOLD}warn${RESET} %s\n" "$1" >&2
}

error() {
  printf "${RED}${BOLD}error${RESET} %s\n" "$1" >&2
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
      for ((i = 0; i < ${#chars}; i++)); do
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
  tput_safe cnorm # Show cursor
  printf "\r${GREEN}✓${RESET} %s\n" "$1"
}

fail_spinner() {
  if [[ -n "$spinner_pid" ]]; then
    kill "$spinner_pid" >/dev/null 2>&1 || true
    wait "$spinner_pid" >/dev/null 2>&1 || true
    spinner_pid=""
  fi
  tput_safe cnorm # Show cursor
  printf "\r${RED}✗${RESET} %s\n" "$1"
}

print_banner() {
  clear
  printf "${MAGENTA}${BOLD}"
  cat <<'EOF'
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
    arch | debian | ubuntu | fedora)
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

  # Only the Arch backend (alpm/libalpm toolchain) links these system
  # libraries; Debian/Fedora/macOS source builds must not demand them.
  if [[ "$(detect_distro)" == "arch" ]]; then
    if ! pkg-config --exists libarchive 2>/dev/null; then missing+=("libarchive"); fi
    if ! pkg-config --exists openssl 2>/dev/null; then missing+=("openssl"); fi
  fi

  if [[ ${#missing[@]} -gt 0 ]]; then
    warn "Missing dependencies: ${missing[*]}"
    printf "\n"
    # Non-interactive default is "n": never run sudo without explicit consent.
    if ask_yes_no "Install missing dependencies with sudo?" "n"; then
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
        debian | ubuntu)
          # Translate probe names to real Debian package names.
          local apt_pkgs=()
          local pkg
          for pkg in "${missing[@]}"; do
            case "$pkg" in
            libarchive) apt_pkgs+=("libarchive-dev") ;;
            openssl) apt_pkgs+=("libssl-dev") ;;
            *) apt_pkgs+=("$pkg") ;;
            esac
          done
          if sudo apt-get update >/dev/null 2>&1 && sudo apt-get install -y "${apt_pkgs[@]}" >/dev/null 2>&1; then
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

  if [[ "$IS_SOURCE_INSTALL" != "true" ]]; then
    error "Remote source fallback is disabled; install a verified release or clone a reviewed tag before running install.sh"
  fi

  local work_dir="$SCRIPT_DIR"
  info "Installing from source directory"
  cd "$work_dir"

  # Select features based on platform. WSL reports Linux and uses its distro backend.
  local cargo_features=""
  local detected_os
  detected_os=$(detect_os)
  local detected_distro
  detected_distro=$(detect_distro)

  case "$detected_os" in
  linux)
    case "$detected_distro" in
    arch) cargo_features="--features arch,license,pgp" ;;
    debian | ubuntu) cargo_features="--no-default-features --features debian,license" ;;
    fedora) cargo_features="--no-default-features --features fedora,license,pgp" ;;
    *) cargo_features="--no-default-features --features fedora,license,pgp" ;;
    esac
    ;;
  darwin)
    cargo_features="--no-default-features --features macos,license,pgp"
    ;;
  *)
    # Runtimes only — no system package manager
    cargo_features="--no-default-features --features license"
    ;;
  esac

  info "Build features: ${cargo_features}"
  export RUSTFLAGS="-C target-cpu=native"
  start_spinner "Compiling binary (release)"
  # shellcheck disable=SC2086
  if cargo build --release --quiet ${cargo_features} >/dev/null 2>&1; then
    stop_spinner "Build successful"
  else
    fail_spinner "Build failed"
    printf "\n${RED}Build output:${RESET}\n"
    # shellcheck disable=SC2086
    cargo build --release ${cargo_features}
    exit 1
  fi

  # Install
  mkdir -p "$INSTALL_DIR"
  install_binary "target/release/omg" "$INSTALL_DIR/omg" || return 1
  if [[ -f "target/release/omgd" ]]; then
    install_binary "target/release/omgd" "$INSTALL_DIR/omgd" || true
  fi

  success "Installed to $INSTALL_DIR/omg"
}

# ⚙️ Configuration
setup_config() {
  header "Configuration"

  mkdir -p "$DATA_DIR"/{versions,cache,db}
  mkdir -p "$CONFIG_DIR"

  if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
    cat >"$CONFIG_DIR/config.toml" <<'EOF'
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
  printf "  ${DIM}Data is sent to omg-api.latham.cloud. You can opt out at any time.${RESET}\\n"
  printf "\\n"

  # Ask for consent; non-interactive runs default to opted-out.
  if ask_yes_no "Allow anonymous telemetry to help improve OMG?" "n"; then
    success "Telemetry enabled. Thank you for helping improve OMG!"
    printf "  ${DIM}Opt out anytime with: export OMG_TELEMETRY=0${RESET}\n"
  else
    set_telemetry_opt_out
    success "Telemetry disabled. You can re-enable with: unset OMG_TELEMETRY"
  fi
}

# Helper to set telemetry opt-out in shell config
set_telemetry_opt_out() {
  local shell_type
  shell_type=$(basename "${SHELL:-/bin/sh}")
  local rc_file=""

  case "$shell_type" in
  bash) rc_file="$HOME/.bashrc" ;;
  zsh) rc_file="$HOME/.zshrc" ;;
  fish) rc_file="$HOME/.config/fish/config.fish" ;;
  *)
    warn "Unsupported shell: $shell_type"
    return
    ;;
  esac

  if [[ -f "$rc_file" ]]; then
    if ! grep -q "OMG_TELEMETRY" "$rc_file"; then
      echo >>"$rc_file"
      echo "# OMG Telemetry opt-out" >>"$rc_file"
      if [[ "$shell_type" == "fish" ]]; then
        echo "set -gx OMG_TELEMETRY 0" >>"$rc_file"
      else
        echo "export OMG_TELEMETRY=0" >>"$rc_file"
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

  local shell_type
  shell_type=$(basename "${SHELL:-/bin/sh}")
  local rc_file=""

  case "$shell_type" in
  bash) rc_file="$HOME/.bashrc" ;;
  zsh) rc_file="$HOME/.zshrc" ;;
  fish) rc_file="$HOME/.config/fish/config.fish" ;;
  *)
    warn "Unsupported shell: $shell_type"
    return
    ;;
  esac

  # Ensure PATH
  if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    if [[ -f "$rc_file" ]]; then
      if ! grep -qE "export PATH=\"$INSTALL_DIR|fish_add_path $INSTALL_DIR" "$rc_file"; then
        if [[ "$shell_type" == "fish" ]]; then
          echo "fish_add_path $INSTALL_DIR" >>"$rc_file"
        else
          echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >>"$rc_file"
        fi
        success "Added $INSTALL_DIR to PATH in $rc_file"
      fi
    fi
  fi

  # Ensure Hook
  if [[ -f "$rc_file" ]]; then
    if ! grep -q "omg hook" "$rc_file"; then
      echo >>"$rc_file"
      echo "# OMG Package Manager" >>"$rc_file"
      if [[ "$shell_type" == "fish" ]]; then
        echo "omg hook fish | source" >>"$rc_file"
      else
        echo 'eval "$(omg hook '"$shell_type"')"' >>"$rc_file"
      fi
      success "Added hook to $rc_file"
    else
      info "Hook already present"
    fi
  fi

  # Generate completions
  "$INSTALL_DIR/omg" completions "$shell_type" >/dev/null 2>&1 || true
}

uninstall_omg() {
  header "Uninstall OMG"

  for bin in omg omgd; do
    if [[ -f "$INSTALL_DIR/$bin" ]]; then
      rm -f "$INSTALL_DIR/$bin"
      success "Removed $INSTALL_DIR/$bin"
    else
      info "$bin not present in $INSTALL_DIR"
    fi
  done

  # Remove the exact lines setup_shell appends. Each rc file is backed up
  # first; only OMG's own marker, hook, and PATH lines are deleted.
  for rc_file in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.config/fish/config.fish"; do
    [[ -f "$rc_file" ]] || continue
    if grep -qE "# OMG Package Manager|omg hook" "$rc_file"; then
      cp "$rc_file" "$rc_file.omg-backup"
      sed -i \
        -e "/# OMG Package Manager/d" \
        -e '/^eval "$(omg hook \(bash\|zsh\))"$/d' \
        -e '/^omg hook fish | source$/d' \
        -e "\|^export PATH=\"$INSTALL_DIR:|d" \
        -e "\|^fish_add_path $INSTALL_DIR$|d" \
        "$rc_file"
      success "Removed OMG integration from $rc_file (backup at $rc_file.omg-backup)"
    fi
  done

  printf "\n"
  printf "${GREEN}${BOLD}Uninstall Complete!${RESET}\n"
  printf "\n"
  printf "  Config and caches were left in place.\n"
  printf "  Remove them manually if desired.\n"
  printf "\n"
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
if [[ "${1:-}" == "--uninstall" || "${OMG_UNINSTALL:-0}" == "1" ]]; then
  uninstall_omg
  exit 0
fi

main() {
  print_banner
  if ! install_from_release; then
    if [[ "$IS_SOURCE_INSTALL" != "true" ]]; then
      error "No verified prebuilt release is available for this platform; refusing to build unpinned repository HEAD"
    fi
    check_platform
    check_dependencies
    build_omg
  fi
  setup_config
  setup_telemetry
  setup_shell
  finish
}

main
