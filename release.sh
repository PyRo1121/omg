#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

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

VERSION="${OMG_VERSION:-}"
TAG_VERSION=""

if [[ "${GITHUB_REF_TYPE:-}" == "tag" && -n "${GITHUB_REF_NAME:-}" ]]; then
  TAG_VERSION="${GITHUB_REF_NAME#v}"
elif [[ -n "${GITHUB_REF_NAME:-}" ]]; then
  TAG_VERSION="${GITHUB_REF_NAME#v}"
fi

if [[ -n "$TAG_VERSION" ]]; then
  VERSION="$TAG_VERSION"
elif [[ -n "$VERSION" && "$VERSION" != "auto" ]]; then
  VERSION="$VERSION"
else
  CURRENT=$(current_version)
  if [[ "${AUTO_BUMP_VERSION:-1}" == "1" ]]; then
    VERSION=$(bump_patch "$CURRENT")
    write_version "$VERSION"
  else
    VERSION="$CURRENT"
  fi
fi

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

echo "Built $ARCHIVE_PATH"
