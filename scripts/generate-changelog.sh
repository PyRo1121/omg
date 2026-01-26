#!/usr/bin/env bash
# OMG Changelog Generator
# Generates user-focused changelogs from git commits using git-cliff
#
# Usage:
#   ./scripts/generate-changelog.sh              # Generate full changelog
#   ./scripts/generate-changelog.sh --latest     # Generate only latest release
#   ./scripts/generate-changelog.sh --unreleased # Generate unreleased changes
#   ./scripts/generate-changelog.sh --tag v0.1.140  # Generate for specific tag

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHANGELOG_FILE="$PROJECT_ROOT/docs/changelog.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1"
}

# Check if git-cliff is installed
check_dependencies() {
    if ! command -v git-cliff &> /dev/null; then
        log_error "git-cliff is not installed"
        echo ""
        echo "Install it with:"
        echo "  cargo install git-cliff"
        echo "  # or"
        echo "  pacman -S git-cliff"
        echo "  # or"
        echo "  brew install git-cliff"
        exit 1
    fi
    log_success "git-cliff is installed ($(git-cliff --version))"
}

# Backup existing changelog
backup_changelog() {
    if [ -f "$CHANGELOG_FILE" ]; then
        local backup_file="${CHANGELOG_FILE}.backup-$(date +%Y%m%d-%H%M%S)"
        cp "$CHANGELOG_FILE" "$backup_file"
        log_info "Backed up existing changelog to: $backup_file"
    fi
}

# Generate full changelog
generate_full() {
    log_info "Generating full changelog..."
    cd "$PROJECT_ROOT"

    git-cliff --config cliff.toml --output "$CHANGELOG_FILE"

    log_success "Full changelog generated: $CHANGELOG_FILE"
}

# Generate latest release only
generate_latest() {
    log_info "Generating latest release changelog..."
    cd "$PROJECT_ROOT"

    git-cliff --config cliff.toml --latest --output "$CHANGELOG_FILE"

    log_success "Latest release changelog generated: $CHANGELOG_FILE"
}

# Generate unreleased changes
generate_unreleased() {
    log_info "Generating unreleased changes..."
    cd "$PROJECT_ROOT"

    git-cliff --config cliff.toml --unreleased --output "$CHANGELOG_FILE"

    log_success "Unreleased changes generated: $CHANGELOG_FILE"
}

# Generate for specific tag
generate_tag() {
    local tag="$1"
    log_info "Generating changelog for tag: $tag"
    cd "$PROJECT_ROOT"

    git-cliff --config cliff.toml --tag "$tag" --output "$CHANGELOG_FILE"

    log_success "Changelog for $tag generated: $CHANGELOG_FILE"
}

# Preview without writing (useful for CI/PR checks)
preview_unreleased() {
    log_info "Previewing unreleased changes..."
    cd "$PROJECT_ROOT"

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    git-cliff --config cliff.toml --unreleased
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
}

# Show help
show_help() {
    cat << EOF
OMG Changelog Generator

Generates user-focused changelogs from git commits using git-cliff.

USAGE:
    $0 [OPTIONS]

OPTIONS:
    (no args)           Generate full changelog from all tags
    --latest            Generate only the latest release
    --unreleased        Generate unreleased changes since last tag
    --preview           Preview unreleased changes without writing
    --tag TAG           Generate changelog for a specific tag
    --help, -h          Show this help message

EXAMPLES:
    # Before a release, preview what will be in the changelog
    $0 --preview

    # Generate full changelog (recommended after each release)
    $0

    # Update changelog with latest release only
    $0 --latest

    # Generate unreleased changes for current development
    $0 --unreleased

    # Generate changelog up to a specific tag
    $0 --tag v0.1.140

WORKFLOW:
    1. During development: Use --preview to see upcoming changes
    2. Before release: Use --unreleased to update docs
    3. After release: Use --latest or full generation
    4. For hotfixes: Use --tag to regenerate specific release

FILES:
    Config:     $PROJECT_ROOT/cliff.toml
    Output:     $CHANGELOG_FILE

NOTES:
    - Automatically creates backups before overwriting
    - Follows conventional commit format (feat/fix/perf/docs/etc)
    - Groups changes by user impact (Features, Fixes, Performance, etc)
    - Skips noise (WIP commits, trivial chores, release commits)

EOF
}

# Main logic
main() {
    check_dependencies

    case "${1:-}" in
        --help|-h)
            show_help
            ;;
        --latest)
            backup_changelog
            generate_latest
            ;;
        --unreleased)
            backup_changelog
            generate_unreleased
            ;;
        --preview)
            preview_unreleased
            ;;
        --tag)
            if [ -z "${2:-}" ]; then
                log_error "Tag name required"
                echo "Usage: $0 --tag TAG"
                exit 1
            fi
            backup_changelog
            generate_tag "$2"
            ;;
        "")
            backup_changelog
            generate_full
            ;;
        *)
            log_error "Unknown option: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

main "$@"
