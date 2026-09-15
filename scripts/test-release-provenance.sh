#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
source <(sed -n '/^resolve_artifact()/,/^}/p' "$repo_root/scripts/release-smoke.sh")

validate_checksum() { printf 'fixture-digest\n'; }
verify_pinned_digest() { return 0; }
gh() {
  if [[ "$1 $2" == 'release download' ]]; then
    local destination=''
    while (($#)); do
      if [[ "$1" == --dir ]]; then destination=$2; shift; fi
      shift
    done
    printf 'fixture\n' > "$destination/$archive"
    printf 'fixture\n' > "$destination/$archive.sha256"
    return 0
  fi
  if [[ "$1 $2" == 'attestation verify' ]]; then
    printf '%s\n' "$@" > "$scratch/attestation-args"
    return "$attestation_result"
  fi
  return 99
}

tag=v9.9.9
repo=PyRo1121/omg
distro_suffix=-macos-arm64
staged_dir=''
digest_pin_file="$scratch/pins"
executor=native
attestation_result=1
mkdir "$scratch/rejected"
if resolve_artifact "$scratch/rejected"; then
  printf 'FAIL: invalid provenance accepted before native extraction\n' >&2
  exit 1
fi
[[ -f "$scratch/attestation-args" ]] || { printf 'FAIL: verifier was not called\n' >&2; exit 1; }
grep -Fxq -- '--source-ref' "$scratch/attestation-args"
grep -Fxq -- 'refs/tags/v9.9.9' "$scratch/attestation-args"
grep -Fxq -- '--signer-workflow' "$scratch/attestation-args"
grep -Fxq -- 'omg-cli/omg/.github/workflows/release.yml' "$scratch/attestation-args"
grep -Fxq -- '--repo' "$scratch/attestation-args"
grep -Fxq -- 'omg-cli/omg' "$scratch/attestation-args"

attestation_result=0
mkdir "$scratch/accepted"
resolve_artifact "$scratch/accepted"

# Locally staged test artifacts use the independent pin contract, not a
# published-release attestation. The hosted job does not select this mode.
staged_dir="$scratch/accepted"
attestation_result=1
mkdir "$scratch/staged"
resolve_artifact "$scratch/staged"
printf 'PASS: rejected provenance fails closed; verified and staged controls pass\n'
