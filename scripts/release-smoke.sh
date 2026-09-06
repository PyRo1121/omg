#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: scripts/release-smoke.sh --release latest|vX.Y.Z --distro arch|debian|ubuntu|fedora|all
                                [--container-engine docker|podman] [--evidence-dir PATH]

Smoke-tests a published OMG release archive: verifies the sha256 sidecar, then
runs the shipped `omg` binary inside a disposable, digest-pinned distro
container to verify the exact release version output, `omg search tree`,
`omg install -y tree`, a native installed assertion, `omg remove -y tree`,
and a native removed assertion. Package indexes are prepared first; each
selected distro writes its own evidence directory (prior evidence is kept).

Options:
  --release latest|vX.Y.Z    Release tag to test (default: latest)
  --distro ID                arch, debian, ubuntu, fedora, or all (default: all)
  --container-engine ENGINE  docker or podman (default: $OMG_SMOKE_ENGINE or docker)
  --evidence-dir PATH        Base directory for per-distro evidence
                             (default: target/release-smoke in the repository)

Environment:
  OMG_SMOKE_REPOSITORY    GitHub repository to resolve releases from
                          (fallbacks: $GITHUB_REPOSITORY, then PyRo1121/omg)
  OMG_SMOKE_ENGINE        Container engine when --container-engine is absent
  OMG_SMOKE_EVIDENCE_DIR  Default evidence base directory
EOF
}

# SmokeCase registry: one entry per distro with the release artifact suffix,
# a digest-pinned container image, the package-index preparation command, the
# probe package, and the native installed/removed assertions (evaluated in the
# container). Image provenance: arch/debian/fedora mirror the build container
# images in .github/workflows/release.yml; ubuntu mirrors Dockerfile.ubuntu.
load_case() {
  case "$1" in
    arch)
      case_suffix="-x86_64-linux-arch"
      case_image="archlinux:latest@sha256:b0deabeb3d283da2c7f7dbf0eea051b7b2cd0554e0b737cc457fd21683bdcdd1"
      case_index_cmd="pacman -Sy --noconfirm"
      case_probe_pkg="tree"
      case_installed_assert="pacman -Qi tree"
      case_removed_assert="! pacman -Q tree"
      ;;
    debian)
      case_suffix="-x86_64-linux-debian"
      case_image="debian:bookworm@sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931"
      case_index_cmd="apt-get update && apt-get install -y --no-install-recommends ca-certificates"
      case_probe_pkg="tree"
      case_installed_assert="dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      case_removed_assert="! dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      ;;
    ubuntu)
      case_suffix="-x86_64-linux-ubuntu"
      case_image="ubuntu:24.04@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517"
      case_index_cmd="apt-get update && apt-get install -y --no-install-recommends ca-certificates"
      case_probe_pkg="tree"
      case_installed_assert="dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      case_removed_assert="! dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      ;;
    fedora)
      case_suffix="-x86_64-linux-fedora"
      case_image="fedora:latest@sha256:6c75d5bf57cb0fa5aa4b92c6a83c86c791644496d9ac230de7711f5b8ec3b898"
      case_index_cmd="dnf -y makecache"
      case_probe_pkg="tree"
      case_installed_assert="rpm -q tree"
      case_removed_assert="! rpm -q tree"
      ;;
    *)
      return 1
      ;;
  esac
}

require_engine() {
  local engine=$1
  if ! command -v "$engine" >/dev/null 2>&1; then
    printf 'error: container engine %q not found in PATH.\n' "$engine" >&2
    printf '       Install Docker or Podman, or select one with --container-engine / OMG_SMOKE_ENGINE.\n' >&2
    exit 3
  fi
  if ! "$engine" info >/dev/null 2>&1; then
    printf 'error: %q info failed - the %s daemon is not reachable.\n' "$engine" "$engine" >&2
    printf '       Start the daemon (e.g. sudo systemctl start docker) and rerun.\n' >&2
    exit 3
  fi
}

run_distro() (
  # Subshell body: fatal() below must exit this one case with code 3 without
  # ending the whole run when --distro all is still making progress.
  set -euo pipefail
  local distro=$1
  local status=error
  local elapsed=0
  local digest=none
  local workdir stage evidence_dir tag version archive probe_bin started

  load_case "$distro" || { printf 'error: unknown distro %q\n' "$distro" >&2; exit 2; }

  workdir="$(mktemp -d "${TMPDIR:-/tmp}/omg-release-smoke-${distro}.XXXXXX")"
  stage="$workdir/stage"
  trap '[[ -n "${workdir:-}" ]] && rm -rf "$workdir"' EXIT

  evidence_dir="$evidence_base/${distro}-$(date -u +%Y%m%dT%H%M%SZ)"
  if [[ -e "$evidence_dir" ]]; then
    evidence_dir="${evidence_dir}-$$"
  fi
  mkdir -p "$evidence_dir"

  write_metadata() {
    {
      printf 'distro=%s\n' "$distro"
      printf 'status=%s\n' "$status"
      printf 'release=%s\n' "${tag:-none}"
      printf 'image=%s\n' "${case_image:-none}"
      printf 'archive=%s\n' "${archive:-none}"
      printf 'archive_sha256=%s\n' "$digest"
      printf 'engine=%s\n' "$engine"
      printf 'probe_package=%s\n' "${case_probe_pkg:-none}"
      printf 'elapsed_seconds=%s\n' "$elapsed"
      printf 'evidence_dir=%s\n' "$evidence_dir"
      printf 'date_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$evidence_dir/metadata.txt"
  }

  fatal() {
    printf 'error: %s\n' "$1" >&2
    status=error
    write_metadata
    exit 3
  }

  exec > >(tee "$evidence_dir/transcript.txt") 2>&1
  set -x

  if [[ "$release" == "latest" ]]; then
    if ! tag="$(gh release view --repo "$repo" --json tagName --jq .tagName)"; then
      fatal "could not resolve latest release in $repo via gh"
    fi
  else
    tag="$release"
  fi
  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fatal "release tag '$tag' is not a published vX.Y.Z tag"
  fi
  local is_draft
  if ! is_draft="$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft)"; then
    fatal "release $tag not found or inaccessible in $repo"
  fi
  if [[ "$is_draft" != "false" ]]; then
    fatal "release $tag is not a published (non-draft) release in $repo"
  fi

  version="${tag#v}"
  archive="omg-${tag}${case_suffix}.tar.gz"

  if ! gh release download "$tag" --repo "$repo" \
      --pattern "$archive" --pattern "${archive}.sha256" --dir "$workdir"; then
    fatal "could not download $archive (+ sidecar) from release $tag in $repo"
  fi
  if [[ ! -f "$workdir/$archive" || ! -f "$workdir/${archive}.sha256" ]]; then
    fatal "release $tag is missing $archive or its .sha256 sidecar"
  fi
  if [[ "$(find "$workdir" -maxdepth 1 -type f | wc -l)" -ne 2 ]]; then
    fatal "download returned unexpected files for $archive"
  fi

  # Boundary validation, run once: one sidecar line, lowercase 64-hex digest,
  # exactly the expected recorded filename, no extra field, recomputed checksum.
  local checksum_lines=()
  mapfile -t checksum_lines < "$workdir/${archive}.sha256"
  if [[ "${#checksum_lines[@]}" -ne 1 ]]; then
    fatal "${archive}.sha256 must contain exactly one line"
  fi
  local sidecar_digest sidecar_filename sidecar_extra
  read -r sidecar_digest sidecar_filename sidecar_extra <<<"${checksum_lines[0]}"
  if [[ -n "${sidecar_extra:-}" ]]; then
    fatal "${archive}.sha256 has extra fields after the filename"
  fi
  if [[ ! "$sidecar_digest" =~ ^[0-9a-f]{64}$ ]]; then
    fatal "${archive}.sha256 digest is not a lowercase 64-hex sha256"
  fi
  if [[ "$sidecar_filename" != "$archive" ]]; then
    fatal "${archive}.sha256 records filename '$sidecar_filename', expected '$archive'"
  fi
  if [[ "$(sha256sum "$workdir/$archive" | awk '{print $1}')" != "$sidecar_digest" ]]; then
    fatal "checksum mismatch: $archive does not match ${archive}.sha256"
  fi
  digest="$sidecar_digest"

  if ! "$engine" pull "$case_image"; then
    fatal "could not pull image $case_image"
  fi

  mkdir -p "$stage"
  if ! tar -xzf "$workdir/$archive" -C "$stage"; then
    fatal "could not extract $archive"
  fi
  probe_bin="omg-${tag}${case_suffix}/omg"
  if [[ ! -f "$stage/$probe_bin" ]]; then
    fatal "archive does not contain $probe_bin"
  fi

  cat > "$stage/probe.sh" <<'PROBE'
#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
bin="/probe/${OMG_PROBE_BIN}"

bash -c "${OMG_PROBE_INDEX_CMD}"

version_line="$(printf '%s\n' "$("$bin" --version)" | head -n 1 | tr -d '[:space:]')"
if [[ "$version_line" != "omg${OMG_PROBE_VERSION_NUM}" ]]; then
  echo "FAIL: expected first --version line 'omg${OMG_PROBE_VERSION_NUM}', got '${version_line}'" >&2
  exit 1
fi

bash -c "${OMG_PROBE_REMOVED_ASSERT}"
search_output="$("$bin" search "${OMG_PROBE_PKG}")"
printf '%s\n' "$search_output"
grep -Eqi "^[[:space:]]+${OMG_PROBE_PKG}[[:space:]]" <<<"$search_output"
"$bin" install -y "${OMG_PROBE_PKG}"
bash -c "${OMG_PROBE_INSTALLED_ASSERT}"
"$bin" remove -y "${OMG_PROBE_PKG}"
bash -c "${OMG_PROBE_REMOVED_ASSERT}"
echo "PASS: ${OMG_PROBE_VERSION_NUM} ${OMG_PROBE_PKG} install/remove verified"
PROBE
  cp "$stage/probe.sh" "$evidence_dir/probe.sh"

  started=$SECONDS
  status=pass
  if ! "$engine" run --rm \
      -e OMG_PROBE_VERSION_NUM="$version" \
      -e OMG_PROBE_BIN="$probe_bin" \
      -e OMG_PROBE_INDEX_CMD="$case_index_cmd" \
      -e OMG_PROBE_INSTALLED_ASSERT="$case_installed_assert" \
      -e OMG_PROBE_REMOVED_ASSERT="$case_removed_assert" \
      -e OMG_PROBE_PKG="$case_probe_pkg" \
      -v "$stage:/probe:ro" \
      "$case_image" \
      bash -x /probe/probe.sh; then
    status=fail
  fi
  elapsed=$((SECONDS - started))

  set +x
  write_metadata
  printf '== distro %s: status=%s elapsed=%ss evidence=%s ==\n' \
    "$distro" "$status" "$elapsed" "$evidence_dir"

  [[ "$status" == "pass" ]]
)

run_all() {
  local distros=()
  if [[ "$distro" == "all" ]]; then
    distros=(arch debian ubuntu fedora)
  else
    distros=("$distro")
  fi
  local overall=0
  local d rc
  for d in "${distros[@]}"; do
    rc=0
    run_distro "$d" || rc=$?
    if [[ "$rc" == "3" ]]; then
      printf 'error: infrastructure failure while smoking %s; aborting remaining distros\n' "$d" >&2
      return 3
    fi
    if [[ "$rc" != "0" ]]; then
      overall=1
    fi
  done
  return "$overall"
}

release="latest"
distro="all"
engine="${OMG_SMOKE_ENGINE:-docker}"
evidence_base="${OMG_SMOKE_EVIDENCE_DIR:-$repo_root/target/release-smoke}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --release|--distro|--container-engine|--evidence-dir)
      if [[ $# -lt 2 ]]; then
        printf 'error: %s requires a value\n' "$1" >&2
        usage >&2
        exit 2
      fi
      case "$1" in
        --release) release=$2 ;;
        --distro) distro=$2 ;;
        --container-engine) engine=$2 ;;
        --evidence-dir) evidence_base=$2 ;;
      esac
      shift 2
      ;;
    *)
      printf 'error: unknown argument %q\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$release" in
  latest) ;;
  *)
    if [[ ! "$release" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      printf 'error: --release must be "latest" or vX.Y.Z, got %q\n' "$release" >&2
      exit 2
    fi
    ;;
esac
case "$distro" in
  arch|debian|ubuntu|fedora|all) ;;
  *)
    printf 'error: --distro must be arch, debian, ubuntu, fedora, or all, got %q\n' "$distro" >&2
    exit 2
    ;;
esac

repo="${OMG_SMOKE_REPOSITORY:-${GITHUB_REPOSITORY:-PyRo1121/omg}}"

# Engine and gh availability are checked before any network access.
require_engine "$engine"
if ! command -v gh >/dev/null 2>&1; then
  printf 'error: gh CLI not found in PATH. Install it (https://cli.github.com) and authenticate (GH_TOKEN or gh auth login).\n' >&2
  exit 3
fi

rc=0
run_all || rc=$?
exit "$rc"