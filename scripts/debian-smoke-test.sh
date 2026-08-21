#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
container_engine="${CONTAINER_ENGINE:-docker}"
image="${OMG_DEBIAN_SMOKE_IMAGE:-omg-debian-smoke:local}"

if ! command -v "${container_engine}" >/dev/null 2>&1; then
  printf 'error: container engine not found: %s\n' "${container_engine}" >&2
  exit 127
fi

"${container_engine}" build --file "${repo_root}/Dockerfile.debian" --tag "${image}" "${repo_root}"
"${container_engine}" run --rm "${image}"
