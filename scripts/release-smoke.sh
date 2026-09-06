#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
inventory="$repo_root/tests/cli_behavior_inventory.tsv"

usage() {
  cat <<'EOF'
Usage: scripts/release-smoke.sh [--release latest|vX.Y.Z | --staged-dir PATH --release vX.Y.Z]
                                --distro arch|debian|ubuntu|fedora|all
                                [--case ID] [--family package] [--tier container]
                                [--container-engine docker|podman] [--evidence-dir PATH]

Verifies exact OMG release archives and runs selected release contracts inside
disposable, digest-pinned distro containers.

Options:
  --release TAG              Published tag, or the tag represented by --staged-dir
                             (default: latest without --staged-dir)
  --staged-dir PATH          Read archives and sidecars from a local staging directory
  --distro ID                arch, debian, ubuntu, fedora, or all (default: all)
  --case ID                  Run one release contract
  --family NAME              Run one contract family (default: package)
  --tier NAME                Run one execution tier (default: container)
  --timeout-seconds N        Container execution limit, 1..9999 (default: 300)
  --container-engine ENGINE  docker or podman (default: $OMG_SMOKE_ENGINE or docker)
  --evidence-dir PATH        Evidence base (default: target/release-smoke)

Environment:
  OMG_SMOKE_REPOSITORY    GitHub repository used for published releases
  OMG_SMOKE_ENGINE        Default container engine
  OMG_SMOKE_EVIDENCE_DIR  Default evidence base directory
EOF
}

load_distro() {
  case "$1" in
    arch)
      case_suffix="-x86_64-linux-arch"
      case_image="archlinux:latest@sha256:b0deabeb3d283da2c7f7dbf0eea051b7b2cd0554e0b737cc457fd21683bdcdd1"
      case_index_cmd="pacman-key --init && pacman-key --populate archlinux && pacman -Syu --noconfirm"
      case_probe_pkg="tree"
      case_installed_assert="pacman -Qi tree"
      case_removed_assert="! pacman -Q tree"
      ;;
    debian)
      distro_suffix="-x86_64-linux-debian"
      distro_image="debian:bookworm@sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931"
      distro_index_cmd="apt-get update && apt-get install -y --no-install-recommends ca-certificates"
      distro_installed_assert="dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      distro_removed_assert="! dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      ;;
    ubuntu)
      distro_suffix="-x86_64-linux-ubuntu"
      distro_image="ubuntu:24.04@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517"
      distro_index_cmd="apt-get update && apt-get install -y --no-install-recommends ca-certificates"
      distro_installed_assert="dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      distro_removed_assert="! dpkg-query -W -f='\${db:Status-Abbrev}' tree | grep -q '^ii'"
      ;;
    fedora)
      distro_suffix="-x86_64-linux-fedora"
      distro_image="fedora:latest@sha256:6c75d5bf57cb0fa5aa4b92c6a83c86c791644496d9ac230de7711f5b8ec3b898"
      distro_index_cmd="dnf -y makecache"
      distro_installed_assert="rpm -q tree"
      distro_removed_assert="! rpm -q tree"
      ;;
    *) return 1 ;;
  esac
}

require_engine() {
  if ! command -v "$engine" >/dev/null 2>&1; then
    printf 'error: container engine %q not found in PATH.\n' "$engine" >&2
    return 3
  fi
  if ! "$engine" info >/dev/null 2>&1; then
    printf 'error: %q info failed; the container engine is unavailable.\n' "$engine" >&2
    return 3
  fi
}

case_family() {
  case "$1" in
    release-package-*) printf 'package\n' ;;
    *) printf 'unknown\n' ;;
  esac
}

load_release_cases() {
  local header id args safety expected_exit ux requires tiers targets assertions cleanup
  IFS= read -r header < "$inventory"
  local expected_header=$'case\targs_json\tsafety\texpected_exit\texpected_ux\trequires\ttier\ttargets\tassertions\tcleanup'
  if [[ "$header" != "$expected_header" ]]; then
    printf 'error: %s has an unsupported header.\n' "$inventory" >&2
    return 3
  fi

  selected_cases=()
  declare -gA case_args=() case_exit=() case_targets=()
  while IFS=$'\t' read -r id args safety expected_exit ux requires tiers targets assertions cleanup; do
    [[ -n "$id" ]] || continue
    [[ "$id" =~ ^[a-z0-9][a-z0-9-]*$ ]] || {
      printf 'error: invalid release contract identifier %q.\n' "$id" >&2
      return 3
    }
    [[ "$ux" == "pass" ]] || continue
    [[ ",$tiers," == *",$tier,"* ]] || continue
    [[ "$targets" != "hermetic:pass" ]] || continue
    [[ "$(case_family "$id")" == "$family" ]] || continue
    [[ -z "$case_id" || "$id" == "$case_id" ]] || continue
    [[ "$expected_exit" =~ ^[0-9]+$ ]] || {
      printf 'error: release contract %s has invalid expected exit %q.\n' "$id" "$expected_exit" >&2
      return 3
    }
    selected_cases+=("$id")
    case_args["$id"]="$args"
    case_exit["$id"]="$expected_exit"
    case_targets["$id"]="$targets"
  done < <(tail -n +2 "$inventory")

  if [[ ${#selected_cases[@]} -eq 0 ]]; then
    printf 'error: no release contracts match case=%q family=%q tier=%q.\n' "$case_id" "$family" "$tier" >&2
    printf 'valid release contracts:\n' >&2
    awk -F '\t' '$1 ~ /^release-/ && $5 == "pass" && $8 != "hermetic:pass" { print "  " $1 }' "$inventory" >&2
    return 2
  fi
  for id in "${selected_cases[@]}"; do
    if ! probe_kind_for_args "${case_args[$id]}" >/dev/null; then
      printf 'error: release contract %s has no container executor for %s.\n' "$id" "${case_args[$id]}" >&2
      return 3
    fi
  done
}

target_for_distro() {
  local targets=$1 wanted=$2 entry
  local entries=()
  IFS=',' read -ra entries <<< "$targets"
  for entry in "${entries[@]}"; do
    if [[ "${entry%%:*}" == "$wanted" ]]; then
      printf '%s\n' "${entry#*:}"
      return 0
    fi
  done
  return 1
}

validate_checksum() {
  local archive_path=$1 sidecar_path=$2 archive_name=$3
  [[ -f "$archive_path" && ! -L "$archive_path" ]] || return 1
  [[ -f "$sidecar_path" && ! -L "$sidecar_path" ]] || return 1
  local checksum_lines=()
  mapfile -t checksum_lines < "$sidecar_path"
  [[ ${#checksum_lines[@]} -eq 1 ]] || return 1
  local sidecar_digest sidecar_filename sidecar_extra
  read -r sidecar_digest sidecar_filename sidecar_extra <<< "${checksum_lines[0]}"
  [[ -z "${sidecar_extra:-}" ]] || return 1
  [[ "$sidecar_digest" =~ ^[0-9a-f]{64}$ ]] || return 1
  [[ "$sidecar_filename" == "$archive_name" ]] || return 1
  [[ "$(sha256sum "$archive_path" | awk '{print $1}')" == "$sidecar_digest" ]] || return 1
  printf '%s\n' "$sidecar_digest"
}

write_result() {
  local evidence_dir=$1 case_id=$2 distro=$3 result=$4 exit_code=$5 elapsed=$6 expectation=$7
  printf '{"case_id":"%s","distro":"%s","result":"%s","exit_code":%s,"elapsed_seconds":%s,"expectation":"%s","artifact_source":"%s"}\n' \
    "$case_id" "$distro" "$result" "$exit_code" "$elapsed" "$expectation" "$artifact_source" > "$evidence_dir/result.json"
  cat "$evidence_dir/result.json" >> "$results_ndjson"
}

probe_kind_for_args() {
  case "$1" in
    '["search","tree"]') printf 'search-tree\n' ;;
    '["install","--yes","tree"]') printf 'install-tree\n' ;;
    '["remove","--yes","tree"]') printf 'remove-tree\n' ;;
    *) return 1 ;;
  esac
}

write_probe() {
  local path=$1
  cat > "$path" <<'PROBE'
#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
bin="/probe/${OMG_PROBE_BIN}"
bash -c "${OMG_PROBE_INDEX_CMD}" || exit 120
version_line="$(printf '%s\n' "$("$bin" --version)" | head -n 1 | tr -d '[:space:]')"
[[ "$version_line" == "omg${OMG_PROBE_VERSION_NUM}" ]]
bash -c "${OMG_PROBE_REMOVED_ASSERT}" || exit 120
case "$OMG_SMOKE_PROBE_KIND" in
  search-tree)
    search_output="$("$bin" search tree)" || exit 1
    printf '%s\n' "$search_output"
    grep -Eqi '^[[:space:]]+tree[[:space:]]' <<< "$search_output"
    ;;
  install-tree)
    "$bin" install --yes tree || exit 1
    bash -c "${OMG_PROBE_INSTALLED_ASSERT}"
    ;;
  remove-tree)
    "$bin" install --yes tree || exit 120
    bash -c "${OMG_PROBE_INSTALLED_ASSERT}" || exit 120
    "$bin" remove --yes tree || exit 1
    bash -c "${OMG_PROBE_REMOVED_ASSERT}"
    ;;
  *)
    printf 'unknown release probe: %s\n' "$OMG_SMOKE_PROBE_KIND" >&2
    exit 2
    ;;
esac
PROBE
  chmod 700 "$path"
}

record_nonexecution() {
  local distro=$1 result=$2 message=$3 case_id expectation evidence_dir
  for case_id in "${selected_cases[@]}"; do
    expectation="$(target_for_distro "${case_targets[$case_id]}" "$distro")" || expectation="missing"
    evidence_dir="$run_evidence/${distro}-${case_id}"
    mkdir -p "$evidence_dir"
    printf '%s: %s\n' "$result" "$message" > "$evidence_dir/transcript.txt"
    {
      printf 'case_id=%s\n' "$case_id"
      printf 'distro=%s\n' "$distro"
      printf 'result=%s\n' "$result"
      printf 'expectation=%s\n' "$expectation"
      printf 'release=%s\n' "$tag"
      printf 'engine=%s\n' "$engine"
      printf 'elapsed_seconds=0\n'
    } > "$evidence_dir/metadata.txt"
    write_result "$evidence_dir" "$case_id" "$distro" "$result" 3 0 "$expectation"
  done
}

record_harness_error() {
  record_nonexecution "$1" "HARNESS_ERROR" "$2"
}

resolve_artifact() {
  local workdir=$1
  version="${tag#v}"
  archive="omg-${tag}${distro_suffix}.tar.gz"
  if [[ -n "$staged_dir" ]]; then
    [[ -f "$staged_dir/$archive" && ! -L "$staged_dir/$archive" ]] || return 1
    [[ -f "$staged_dir/${archive}.sha256" && ! -L "$staged_dir/${archive}.sha256" ]] || return 1
    cp "$staged_dir/$archive" "$workdir/$archive" || return 1
    cp "$staged_dir/${archive}.sha256" "$workdir/${archive}.sha256" || return 1
  else
    gh release download "$tag" --repo "$repo" \
      --pattern "$archive" --pattern "${archive}.sha256" --dir "$workdir" || return 1
  fi
  [[ "$(find "$workdir" -maxdepth 1 -type f | wc -l)" -eq 2 ]] || return 1
  digest="$(validate_checksum "$workdir/$archive" "$workdir/${archive}.sha256" "$archive")" || return 1
}

run_case() (
  local distro=$1 case_id=$2 stage=$3
  local expectation evidence_dir started elapsed probe_bin probe_kind observed_exit result
  local container_name="omg-smoke-${BASHPID}-${distro}-${case_id}" cleanup_ok=true
  expectation="$(target_for_distro "${case_targets[$case_id]}" "$distro")" || expectation="missing"
  evidence_dir="$run_evidence/${distro}-${case_id}"
  mkdir -p "$evidence_dir"
  probe_kind="$(probe_kind_for_args "${case_args[$case_id]}")" || return 3
  write_probe "$stage/probe-${case_id}.sh"
  cp "$stage/probe-${case_id}.sh" "$evidence_dir/probe.sh"
  probe_bin="omg-${tag}${distro_suffix}/omg"

  cleanup_container() {
    local remaining
    timeout --kill-after=5s 10s "$engine" rm --force "$container_name" >> "$evidence_dir/cleanup.txt" 2>&1 || true
    remaining="$(timeout --kill-after=5s 10s "$engine" ps --all --quiet --filter "name=^/${container_name}$" 2>> "$evidence_dir/cleanup.txt")" || return 1
    [[ -z "$remaining" ]] || return 1
    printf 'verified absent: %s\n' "$container_name" >> "$evidence_dir/cleanup.txt"
  }
  trap 'cleanup_container || exit 3' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM

  exec 3>&1 4>&2
  exec > >(tee "$evidence_dir/transcript.txt") 2>&1
  set -x
  started=$SECONDS
  observed_exit=0
  timeout --kill-after=5s "${timeout_seconds}s" "$engine" run --rm --name "$container_name" \
    -e OMG_SMOKE_PROBE_KIND="$probe_kind" \
    -e OMG_PROBE_VERSION_NUM="$version" \
    -e OMG_PROBE_BIN="$probe_bin" \
    -e OMG_PROBE_INDEX_CMD="$distro_index_cmd" \
    -e OMG_PROBE_INSTALLED_ASSERT="$distro_installed_assert" \
    -e OMG_PROBE_REMOVED_ASSERT="$distro_removed_assert" \
    -v "$stage:/probe:ro" \
    "$distro_image" bash -x "/probe/probe-${case_id}.sh" || observed_exit=$?
  cleanup_container || cleanup_ok=false
  trap - EXIT INT TERM
  elapsed=$((SECONDS - started))
  set +x
  exec 1>&3 2>&4
  exec 3>&- 4>&-

  case "$expectation" in
    pass|known-defect)
      if [[ $observed_exit -eq "${case_exit[$case_id]}" ]]; then result="PASS"; else result="PRODUCT_FAIL"; fi
      ;;
    expected-rejection)
      if [[ $observed_exit -eq "${case_exit[$case_id]}" ]]; then result="EXPECTED_REJECTION"; else result="PRODUCT_FAIL"; fi
      ;;
    blocked|not-applicable) result="BLOCKED" ;;
    *) result="HARNESS_ERROR" ;;
  esac
  case "$observed_exit" in
    120|124|125|126|127|137) result="HARNESS_ERROR" ;;
  esac
  if [[ "$cleanup_ok" != true ]]; then
    result="HARNESS_ERROR"
  fi

  {
    printf 'case_id=%s\n' "$case_id"
    printf 'distro=%s\n' "$distro"
    printf 'result=%s\n' "$result"
    printf 'expectation=%s\n' "$expectation"
    printf 'release=%s\n' "$tag"
    printf 'image=%s\n' "$distro_image"
    printf 'archive=%s\n' "$archive"
    printf 'archive_sha256=%s\n' "$digest"
    printf 'engine=%s\n' "$engine"
    printf 'elapsed_seconds=%s\n' "$elapsed"
  } > "$evidence_dir/metadata.txt"
  write_result "$evidence_dir" "$case_id" "$distro" "$result" "$observed_exit" "$elapsed" "$expectation"
  printf '== %s %s: %s elapsed=%ss evidence=%s ==\n' "$distro" "$case_id" "$result" "$elapsed" "$evidence_dir"
  case "$result" in
    PASS|EXPECTED_REJECTION) return 0 ;;
    PRODUCT_FAIL) return 1 ;;
    HARNESS_ERROR|BLOCKED) return 3 ;;
  esac
)

run_distro() (
  set -euo pipefail
  local distro=$1 workdir stage case_id rc case_rc=0
  load_distro "$distro" || return 2
  local work_root="$HOME/.cache/build-targets/omg-release-smoke"
  mkdir -p "$work_root" || return 3
  workdir="$(mktemp -d "$work_root/${distro}.XXXXXX")" || return 3
  trap 'rm -rf "$workdir"' EXIT
  stage="$workdir/stage"
  mkdir -p "$stage"

  if ! resolve_artifact "$workdir"; then
    record_harness_error "$distro" "failed to acquire or validate the release artifact"
    return 3
  fi
  if ! tar -xzf "$workdir/$archive" -C "$stage"; then
    record_harness_error "$distro" "failed to extract the release artifact"
    return 3
  fi
  if [[ ! -f "$stage/omg-${tag}${distro_suffix}/omg" ]]; then
    record_harness_error "$distro" "release artifact does not contain the expected binary"
    return 3
  fi
  if ! "$engine" pull "$distro_image"; then
    record_harness_error "$distro" "failed to pull the pinned container image"
    return 3
  fi

  for case_id in "${selected_cases[@]}"; do
    rc=0
    run_case "$distro" "$case_id" "$stage" || rc=$?
    if [[ $rc -eq 3 ]]; then
      return 3
    fi
    if [[ $rc -ne 0 ]]; then
      case_rc=1
    fi
  done
  return "$case_rc"
)

finalize_results() {
  {
    printf '[\n'
    awk 'NR > 1 { printf ",\n" } { printf "  %s", $0 } END { if (NR > 0) printf "\n" }' "$results_ndjson"
    printf ']\n'
  } > "$run_evidence/results.json"
  if ! timeout --kill-after=2s 12s env OMG_SMOKE_RELEASE="$tag" \
      "$repo_root/scripts/report-smoke-sentry.sh" "$run_evidence/results.json" \
      > "$run_evidence/reporting.log" 2>&1; then
    printf 'warning: Sentry reporting failed; results remain in %s\n' "$run_evidence" >&2
  fi
}

release="latest"
release_set=false
staged_dir=""
distro="all"
case_id=""
family="package"
tier="container"
timeout_seconds=300
engine="${OMG_SMOKE_ENGINE:-docker}"
evidence_base="${OMG_SMOKE_EVIDENCE_DIR:-$repo_root/target/release-smoke}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --release|--staged-dir|--distro|--case|--family|--tier|--timeout-seconds|--container-engine|--evidence-dir)
      [[ $# -ge 2 ]] || { printf 'error: %s requires a value\n' "$1" >&2; exit 2; }
      case "$1" in
        --release) release=$2; release_set=true ;;
        --staged-dir) staged_dir=$2 ;;
        --distro) distro=$2 ;;
        --case) case_id=$2 ;;
        --family) family=$2 ;;
        --tier) tier=$2 ;;
        --timeout-seconds) timeout_seconds=$2 ;;
        --container-engine) engine=$2 ;;
        --evidence-dir) evidence_base=$2 ;;
      esac
      shift 2
      ;;
    *) printf 'error: unknown argument %q\n' "$1" >&2; exit 2 ;;
  esac
done

if [[ "$release" != "latest" && ! "$release" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'error: --release must be latest or vX.Y.Z\n' >&2
  exit 2
fi
case "$distro" in
  arch|debian|ubuntu|fedora|all) ;;
  *) printf 'error: invalid distro %q\n' "$distro" >&2; exit 2 ;;
esac
if [[ "$family" != "package" ]]; then
  printf 'error: invalid family %q; valid values: package\n' "$family" >&2
  exit 2
fi
if [[ "$tier" != "container" ]]; then
  printf 'error: invalid tier %q; valid values: container\n' "$tier" >&2
  exit 2
fi
if [[ -n "$staged_dir" ]]; then
  [[ "$release_set" == true && "$release" != "latest" ]] || {
    printf 'error: --staged-dir requires an explicit --release vX.Y.Z\n' >&2; exit 2;
  }
  if [[ ! -d "$staged_dir" ]]; then
    printf 'error: staged directory not found: %s\n' "$staged_dir" >&2
    exit 2
  fi
fi

if [[ ! "$timeout_seconds" =~ ^[1-9][0-9]{0,3}$ ]]; then
  printf 'error: --timeout-seconds must be an integer between 1 and 9999\n' >&2
  exit 2
fi
command -v timeout >/dev/null 2>&1 || { printf 'error: GNU timeout is required\n' >&2; exit 3; }
artifact_source=published
if [[ -n "$staged_dir" ]]; then
  artifact_source=staged
fi
load_release_cases || exit $?
repo="${OMG_SMOKE_REPOSITORY:-${GITHUB_REPOSITORY:-PyRo1121/omg}}"
tag="$release"
if [[ "$distro" == "all" ]]; then
  distros=(arch debian ubuntu fedora)
else
  distros=("$distro")
fi
mkdir -p "$evidence_base"
run_evidence="$(mktemp -d "$evidence_base/run-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")"
results_ndjson="$run_evidence/.results.ndjson"
: > "$results_ndjson"
trap 'rm -f "$results_ndjson"' EXIT

if ! require_engine; then
  for selected_distro in "${distros[@]}"; do
    record_nonexecution "$selected_distro" "BLOCKED" "container engine is unavailable"
  done
  finalize_results
  exit 3
fi
if [[ -z "$staged_dir" ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    for selected_distro in "${distros[@]}"; do
      record_harness_error "$selected_distro" "gh CLI not found"
    done
    finalize_results
    exit 3
  fi
  if [[ "$release" == "latest" ]]; then
    if ! tag="$(gh release view --repo "$repo" --json tagName --jq .tagName)"; then
      for selected_distro in "${distros[@]}"; do
        record_harness_error "$selected_distro" "latest release could not be resolved"
      done
      finalize_results
      exit 3
    fi
  fi
  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    for selected_distro in "${distros[@]}"; do
      record_harness_error "$selected_distro" "release tag is not vX.Y.Z"
    done
    finalize_results
    exit 3
  fi
  if ! is_draft="$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft)"; then
    for selected_distro in "${distros[@]}"; do
      record_harness_error "$selected_distro" "release metadata could not be resolved"
    done
    finalize_results
    exit 3
  fi
  if [[ "$is_draft" != "false" ]]; then
    for selected_distro in "${distros[@]}"; do
      record_harness_error "$selected_distro" "release is not published"
    done
    finalize_results
    exit 3
  fi
fi

overall=0
for selected_distro in "${distros[@]}"; do
  rc=0
  run_distro "$selected_distro" || rc=$?
  if [[ $rc -eq 3 ]]; then overall=3; break; fi
  [[ $rc -eq 0 ]] || overall=1
done
finalize_results
exit "$overall"
