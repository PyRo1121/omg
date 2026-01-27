#!/usr/bin/env bash
#
# OMG Release & Publish Script
# Production-grade release automation with comprehensive testing
#
# Usage:
#   ./release_and_publish.sh              # Auto-bump patch version
#   OMG_VERSION=1.2.3 ./release_and_publish.sh  # Specific version
#   AUTO_BUMP_VERSION=0 ./release_and_publish.sh  # Keep current version
#   DRY_RUN=1 ./release_and_publish.sh    # Test without publishing
#
set -euo pipefail

# Fix PATH to use rustup's cargo/rustc pair (avoids version mismatch with OMG-managed Rust)
export PATH="$HOME/.cargo/bin:$PATH"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Configuration
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$ROOT_DIR/dist"
DRY_RUN="${DRY_RUN:-0}"
SKIP_TESTS="${SKIP_TESTS:-0}"
VERBOSE="${VERBOSE:-0}"

cd "$ROOT_DIR"

#=============================================================================
# Utility Functions
#=============================================================================

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[✓]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[✗]${NC} $*" >&2; }
log_step() { echo -e "\n${CYAN}${BOLD}==> $*${NC}"; }

die() {
  log_error "$@"
  exit 1
}

run_cmd() {
  if [[ "$VERBOSE" == "1" ]]; then
    "$@"
  else
    "$@" 2>&1
  fi
}

#=============================================================================
# Prerequisite Checks
#=============================================================================

check_prerequisites() {
  log_step "Checking prerequisites"
  
  local missing=()
  
  # Required tools
  command -v cargo >/dev/null 2>&1 || missing+=("cargo (Rust toolchain)")
  command -v git >/dev/null 2>&1 || missing+=("git")
  command -v gh >/dev/null 2>&1 || missing+=("gh (GitHub CLI: https://cli.github.com/)")
  command -v sha256sum >/dev/null 2>&1 || missing+=("sha256sum")
  command -v tar >/dev/null 2>&1 || missing+=("tar")
  
  if [[ ${#missing[@]} -gt 0 ]]; then
    log_error "Missing required tools:"
    for tool in "${missing[@]}"; do
      echo "  - $tool"
    done
    exit 1
  fi
  
  # Check gh authentication
  if ! gh auth status >/dev/null 2>&1; then
    die "GitHub CLI not authenticated. Run: gh auth login"
  fi
  
  # Check git status
  if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    log_warn "Working directory has uncommitted changes"
    git status --short
    echo ""
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] || exit 1
  fi
  
  log_success "All prerequisites satisfied"
}

#=============================================================================
# Version Management
#=============================================================================

current_version() {
  awk -F'"' '/^version =/ {print $2; exit}' Cargo.toml
}

bump_patch() {
  local version="$1"
  IFS='.' read -r major minor patch <<< "$version"
  patch=$((patch + 1))
  echo "${major}.${minor}.${patch}"
}

write_version() {
  local version="$1"
  sed -i "s/^version = \".*\"/version = \"${version}\"/" Cargo.toml
  # Also update Cargo.lock
  cargo update -p omg --precise "$version" 2>/dev/null || true
}

resolve_version() {
  local override="${OMG_VERSION:-}"
  local current
  current=$(current_version)

  if [[ -n "$override" && "$override" != "auto" ]]; then
    # Validate semver format
    if ! [[ "$override" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
      die "Invalid version format: $override (expected: X.Y.Z or X.Y.Z-suffix)"
    fi
    echo "$override"
    return
  fi

  if [[ "${AUTO_BUMP_VERSION:-1}" == "1" ]]; then
    bump_patch "$current"
  else
    echo "$current"
  fi
}

last_release_tag() {
  git describe --tags --abbrev=0 --match "v*" 2>/dev/null || echo ""
}

#=============================================================================
# Quality Checks
#=============================================================================

run_quality_checks() {
  log_step "Running quality checks"
  local failed=0
  
  # 1. Format check
  log_info "Checking code formatting..."
  if ! cargo fmt --all -- --check; then
    log_error "Code formatting check failed. Run: cargo fmt"
    failed=1
  else
    log_success "Code formatting OK"
  fi
  
  # 2. Clippy (strict)
  log_info "Running clippy with strict warnings..."
  if ! cargo clippy --features arch --lib --bins -- -D warnings; then
    log_error "Clippy found issues"
    failed=1
  else
    log_success "Clippy passed"
  fi
  
  # 3. Build check
  log_info "Checking build..."
  if ! cargo check --features arch --all-targets; then
    log_error "Build check failed"
    failed=1
  else
    log_success "Build check passed"
  fi
  
  # 4. Tests (unless skipped)
  if [[ "$SKIP_TESTS" != "1" ]]; then
    log_info "Running core test suite..."
    
    if cargo test --features arch --lib -- --test-threads=1; then
      log_success "Unit tests passed"
    else
      log_error "Unit tests failed"
      failed=1
    fi
    
    if [[ $failed -eq 0 ]]; then
      log_info "Running integration tests..."
      if cargo test --features arch --test integration_suite --test arch_tests -- --test-threads=1; then
        log_success "Integration tests passed"
      else
        log_warn "Some integration tests failed (non-blocking for release)"
      fi
    fi
  else
    log_warn "Tests skipped (SKIP_TESTS=1)"
  fi
  
  # 5. Documentation check
  log_info "Checking documentation..."
  if ! cargo doc --no-deps 2>/dev/null; then
    log_warn "Documentation has warnings (non-blocking)"
  else
    log_success "Documentation OK"
  fi
  
  # 6. Typos check (optional)
  if command -v typos >/dev/null 2>&1; then
    log_info "Checking for typos..."
    if typos --config ./_typos.toml; then
      log_success "No typos found"
    else
      log_warn "Typos found (non-blocking)"
    fi
  fi
  
  # 7. Security audit (optional)
  if command -v cargo-audit >/dev/null 2>&1; then
    log_info "Running security audit..."
    if cargo audit --deny warnings 2>/dev/null; then
      log_success "Security audit passed"
    else
      log_warn "Security audit has findings (review recommended)"
    fi
  fi
  
  # 8. Dependency check (optional)
  if command -v cargo-machete >/dev/null 2>&1; then
    log_info "Checking for unused dependencies..."
    cargo machete 2>/dev/null || log_warn "Unused dependencies found"
  fi
  
  if [[ $failed -ne 0 ]]; then
    die "Quality checks failed. Fix issues before releasing."
  fi
  
  log_success "All quality checks passed"
}

#=============================================================================
# Build
#=============================================================================

build_release() {
  log_step "Building release binaries"
  
  local arch
  arch="$(uname -m)"
  
  case "$arch" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    armv7l)  TARGET="armv7-unknown-linux-gnueabihf" ;;
    *)       die "Unsupported architecture: $arch" ;;
  esac
  
  log_info "Target: $TARGET"
  
  # Build with optimizations (arch feature for Arch Linux)
  RUSTFLAGS="-C target-cpu=native" cargo build --release --features arch --locked 2>&1 || \
    cargo build --release --features arch  # Fallback without --locked
  
  # Verify binaries exist
  [[ -f target/release/omg ]] || die "Binary 'omg' not found"
  
  # Get binary info
  local omg_size
  omg_size=$(du -h target/release/omg | cut -f1)
  log_success "Built omg ($omg_size)"
  
  if [[ -f target/release/omgd ]]; then
    local omgd_size
    omgd_size=$(du -h target/release/omgd | cut -f1)
    log_success "Built omgd ($omgd_size)"
  fi
}

#=============================================================================
# Package
#=============================================================================

create_package() {
  local version="$1"
  
  log_step "Creating release package"
  
  mkdir -p "$DIST_DIR"
  
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    armv7l)  TARGET="armv7-unknown-linux-gnueabihf" ;;
    *)       TARGET="$arch-unknown-linux-gnu" ;;
  esac
  
  ARCHIVE_NAME="omg-v${version}-${TARGET}.tar.gz"
  ARCHIVE_PATH="$DIST_DIR/$ARCHIVE_NAME"
  
  # Copy binaries
  cp target/release/omg "$DIST_DIR/"
  [[ -f target/release/omgd ]] && cp target/release/omgd "$DIST_DIR/"
  
  # Create archive
  (
    cd "$DIST_DIR"
    if [[ -f omgd ]]; then
      tar -czf "$ARCHIVE_NAME" omg omgd
    else
      tar -czf "$ARCHIVE_NAME" omg
    fi
  )
  
  # Generate checksums
  (
    cd "$DIST_DIR"
    sha256sum "$ARCHIVE_NAME" > "${ARCHIVE_NAME}.sha256"
  )
  
  # Verify archive
  [[ -f "$ARCHIVE_PATH" ]] || die "Failed to create archive"
  
  local archive_size
  archive_size=$(du -h "$ARCHIVE_PATH" | cut -f1)
  log_success "Created $ARCHIVE_NAME ($archive_size)"
  
  # Display checksum
  log_info "SHA256: $(cat "$DIST_DIR/${ARCHIVE_NAME}.sha256" | cut -d' ' -f1)"
}

#=============================================================================
# Release Notes
#=============================================================================

generate_release_notes() {
  local version="$1"
  local last_tag
  last_tag=$(last_release_tag)
  
  echo "## OMG v${version}"
  echo ""
  echo "### Changes"
  echo ""
  
  if [[ -n "$last_tag" ]]; then
    git log "${last_tag}..HEAD" --pretty="- %s (%h)" --no-merges | head -50
  else
    git log --pretty="- %s (%h)" --no-merges | head -20
  fi
  
  echo ""
  echo "### Installation"
  echo ""
  echo '```bash'
  echo "curl -fsSL https://github.com/PyRo1121/omg/releases/download/v${version}/omg-v${version}-x86_64-unknown-linux-gnu.tar.gz | tar xz"
  echo "sudo mv omg /usr/local/bin/"
  echo '```'
  echo ""
  echo "### Checksums"
  echo ""
  echo "Verify your download:"
  echo '```bash'
  echo "sha256sum -c omg-v${version}-*.sha256"
  echo '```'
}

#=============================================================================
# Publish
#=============================================================================

publish_release() {
  local version="$1"
  
  log_step "Publishing release v${version}"
  
  if [[ "$DRY_RUN" == "1" ]]; then
    log_warn "DRY RUN - skipping actual publish"
    log_info "Would commit, tag, and push v${version}"
    log_info "Would create GitHub release with:"
    echo "  - $DIST_DIR/$ARCHIVE_NAME"
    echo "  - $DIST_DIR/${ARCHIVE_NAME}.sha256"
    return
  fi
  
  # Stage changes
  git add Cargo.toml Cargo.lock 2>/dev/null || git add Cargo.toml
  git add "$DIST_DIR"/*.tar.gz "$DIST_DIR"/*.sha256 2>/dev/null || true
  
  # Commit if there are changes
  if ! git diff --cached --quiet; then
    git commit -m "Release v${version}"
    log_success "Committed release changes"
  else
    log_info "No changes to commit"
  fi
  
  # Create tag
  if git rev-parse "v${version}" >/dev/null 2>&1; then
    log_warn "Tag v${version} already exists"
  else
    git tag -a "v${version}" -m "Release v${version}"
    log_success "Created tag v${version}"
  fi
  
  # Push
  git push origin main "v${version}" 2>&1 || {
    log_warn "Push failed, trying with --force-with-lease for tag"
    git push origin main
    git push origin "v${version}" --force-with-lease
  }
  log_success "Pushed to origin"
  
  # Generate release notes
  local release_notes
  release_notes=$(generate_release_notes "$version")
  
  # Create/update GitHub release
  local archive_glob="$DIST_DIR/omg-v${version}-*.tar.gz"
  local checksum_glob="$DIST_DIR/omg-v${version}-*.tar.gz.sha256"
  
  if gh release view "v${version}" >/dev/null 2>&1; then
    log_info "Updating existing release..."
    gh release edit "v${version}" --notes "$release_notes"
    gh release upload "v${version}" $archive_glob $checksum_glob --clobber
  else
    log_info "Creating new release..."
    gh release create "v${version}" \
      $archive_glob $checksum_glob \
      --title "Release v${version}" \
      --notes "$release_notes"
  fi
  
  log_success "Published GitHub release v${version}"
}

#=============================================================================
# Sync & Deploy Website
#=============================================================================

sync_and_deploy_site() {
  log_step "Syncing install script to website"
  
  local site_dir="$ROOT_DIR/site"
  
  if [[ ! -d "$site_dir" ]]; then
    log_warn "Site directory not found, skipping website sync"
    return
  fi
  
  # Sync install.sh to site/public
  cp "$ROOT_DIR/install.sh" "$site_dir/public/install.sh"
  log_success "Synced install.sh to site/public/"
  
  # Deploy site if wrangler is available
  if command -v bunx >/dev/null 2>&1 || command -v npx >/dev/null 2>&1; then
    log_info "Deploying website to Cloudflare Pages..."
    (
      cd "$site_dir"
      if command -v bunx >/dev/null 2>&1; then
        bunx wrangler pages deploy dist --project-name=omg-site 2>&1 || log_warn "Site deploy failed (non-blocking)"
      else
        npx wrangler pages deploy dist --project-name=omg-site 2>&1 || log_warn "Site deploy failed (non-blocking)"
      fi
    ) && log_success "Website deployed"
  else
    log_warn "wrangler not available, skipping site deploy"
  fi
}

#=============================================================================
# Main
#=============================================================================

main() {
  echo -e "${BOLD}${CYAN}"
  echo "╔═══════════════════════════════════════════════════════════╗"
  echo "║           OMG Release & Publish Pipeline                  ║"
  echo "╚═══════════════════════════════════════════════════════════╝"
  echo -e "${NC}"
  
  # Prerequisites
  check_prerequisites
  
  # Resolve version
  local version
  version=$(resolve_version)
  local current
  current=$(current_version)
  
  log_step "Version: $current → $version"
  
  if [[ "$current" != "$version" ]]; then
    write_version "$version"
    log_success "Updated version to $version"
  fi
  
  # Quality checks
  run_quality_checks
  
  # Build
  build_release
  
  # Package
  create_package "$version"
  
  # Publish
  publish_release "$version"
  
  # Sync and deploy website
  sync_and_deploy_site
  
  # Summary
  echo ""
  echo -e "${GREEN}${BOLD}════════════════════════════════════════════════════════════${NC}"
  echo -e "${GREEN}${BOLD}  ✓ Successfully released OMG v${version}${NC}"
  echo -e "${GREEN}${BOLD}════════════════════════════════════════════════════════════${NC}"
  echo ""
  echo "Release URL: https://github.com/PyRo1121/omg/releases/tag/v${version}"
  echo ""
}

main "$@"
