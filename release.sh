#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

VERSION="${OMG_VERSION:-}"
if [[ -z "$VERSION" ]]; then
  VERSION=$(awk -F'"' '/^version =/ {print $2; exit}' Cargo.toml)
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
