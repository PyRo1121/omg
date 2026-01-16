#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required (https://cli.github.com/)"
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "gh is not authenticated. Run: gh auth login"
  exit 1
fi

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
}

resolve_version() {
  local override="${OMG_VERSION:-}"
  local current
  current=$(current_version)

  if [[ -n "$override" && "$override" != "auto" ]]; then
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

release_notes() {
  local last_tag
  last_tag=$(last_release_tag)

  if [[ -n "$last_tag" ]]; then
    git log "${last_tag}..HEAD" --pretty="- %s (%h)"
  else
    git log --pretty="- %s (%h)"
  fi
}

run_checks() {
  echo "Running local checks..."
  cargo fmt --all -- --check
  cargo check --verbose
  cargo test --verbose -- --test-threads=1
  cargo clippy -- -D warnings

  if command -v typos >/dev/null 2>&1; then
    typos --config ./_typos.toml
  else
    echo "typos not installed; skipping spell check"
  fi

  if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit || true
  else
    echo "cargo-audit not installed; skipping security audit"
  fi
}

run_checks

VERSION=$(resolve_version)
if [[ "$(current_version)" != "$VERSION" ]]; then
  write_version "$VERSION"
fi

RELEASE_NOTES=$(release_notes)

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
  aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

DIST_DIR="$ROOT_DIR/dist"
mkdir -p "$DIST_DIR"

cargo build --release

ARCHIVE_NAME="omg-v${VERSION}-${TARGET}.tar.gz"
ARCHIVE_PATH="$DIST_DIR/$ARCHIVE_NAME"

cp target/release/omg "$DIST_DIR/"
if [[ -f target/release/omgd ]]; then
  cp target/release/omgd "$DIST_DIR/"
fi

(
  cd "$DIST_DIR"
  tar -czf "$ARCHIVE_NAME" omg omgd 2>/dev/null || tar -czf "$ARCHIVE_NAME" omg
  sha256sum "$ARCHIVE_NAME" > "${ARCHIVE_NAME}.sha256"
)

ARCHIVE="dist/omg-v${VERSION}-*.tar.gz"
CHECKSUM="dist/omg-v${VERSION}-*.tar.gz.sha256"

if ! ls $ARCHIVE >/dev/null 2>&1; then
  echo "Release archive not found: $ARCHIVE"
  exit 1
fi

if ! ls $CHECKSUM >/dev/null 2>&1; then
  echo "Release checksum not found: $CHECKSUM"
  exit 1
fi

git add Cargo.toml dist/*.tar.gz dist/*.sha256
if git diff --cached --quiet; then
  echo "No changes to commit."
else
  git commit -m "Release v${VERSION}"
fi

git tag -a "v${VERSION}" -m "Release v${VERSION}" || true

git push origin main "v${VERSION}"

# Create or update GitHub Release with assets
if gh release view "v${VERSION}" >/dev/null 2>&1; then
  gh release edit "v${VERSION}" -n "$RELEASE_NOTES"
  gh release upload "v${VERSION}" $ARCHIVE $CHECKSUM --clobber
else
  gh release create "v${VERSION}" $ARCHIVE $CHECKSUM -t "Release v${VERSION}" -n "$RELEASE_NOTES"
fi

echo "Published release v${VERSION}"
