#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="$repo_root/scripts/release-smoke.sh"
scratch_root="$HOME/.cache/build-targets"
mkdir -p "$scratch_root"
scratch="$(mktemp -d "$scratch_root/omg-release-smoke-tests.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin" "$scratch/tmp"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

results_file() {
  find "$1" -mindepth 2 -maxdepth 2 -name results.json -type f -print -quit
}

assert_rc() {
  local expected=$1
  shift
  local actual=0
  "$@" > "$scratch/command.out" 2>&1 || actual=$?
  [[ "$actual" -eq "$expected" ]] || {
    cat "$scratch/command.out" >&2
    fail "expected exit $expected, got $actual"
  }
}

cat > "$scratch/bin/fake-engine" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  info) exit "${FAKE_INFO_EXIT:-0}" ;;
  pull) exit 0 ;;
  run)
    if [[ "${FAKE_HANG:-0}" == 1 ]]; then
      touch "$FAKE_CONTAINER_STATE"
      sleep 30
    fi
    exit "${FAKE_RUN_EXIT:-0}"
    ;;
  rm)
    [[ "${FAKE_CLEANUP_FAIL:-0}" != 1 ]] || exit 1
    rm -f "$FAKE_CONTAINER_STATE"
    ;;
  ps)
    [[ "${FAKE_CLEANUP_FAIL:-0}" != 1 ]] || exit 1
    if [[ -f "$FAKE_CONTAINER_STATE" ]]; then printf 'remaining-container\n'; fi
    ;;
  *) exit 2 ;;
esac
EOF
chmod 700 "$scratch/bin/fake-engine"
cat > "$scratch/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "release" ]] || exit 2
case "${2:-}" in
  view)
    if [[ " $* " == *" isDraft "* ]]; then printf 'false\n'; else printf 'v9.9.9\n'; fi
    ;;
  download)
    destination=""
    while [[ $# -gt 0 ]]; do
      if [[ "$1" == "--dir" ]]; then destination=$2; break; fi
      shift
    done
    [[ -n "$destination" ]]
    cp "$FAKE_GH_SOURCE/omg-v9.9.9-x86_64-linux-arch.tar.gz" "$destination/"
    cp "$FAKE_GH_SOURCE/omg-v9.9.9-x86_64-linux-arch.tar.gz.sha256" "$destination/"
    ;;
  *) exit 2 ;;
esac
EOF
chmod 700 "$scratch/bin/gh"

make_stage() {
  local destination=$1
  local directory="omg-v9.9.9-x86_64-linux-${2:-arch}"
  local archive="$directory.tar.gz"
  rm -rf "$destination"
  mkdir -p "$destination/root/$directory"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$destination/root/$directory/omg"
  chmod 700 "$destination/root/$directory/omg"
  tar -czf "$destination/$archive" -C "$destination/root" "$directory"
  printf '%s  %s\n' "$(sha256sum "$destination/$archive" | awk '{print $1}')" "$archive" > "$destination/${archive}.sha256"
}

base_args=(
  --release v9.9.9
  --distro arch
  --case release-package-search-tree
  --container-engine fake-engine
)
export PATH="$scratch/bin:$PATH"
export TMPDIR="$scratch/tmp"
export HOME="$scratch/home"
mkdir -p "$HOME"

assert_rc 2 "$runner" --timeout-seconds 0
assert_rc 2 "$runner" --timeout-seconds -1
assert_rc 2 "$runner" --timeout-seconds 1.5
assert_rc 2 "$runner" --family invalid
assert_rc 2 "$runner" --tier invalid
assert_rc 2 "$runner" --case not-a-contract

grep -q 'valid release contracts' "$scratch/command.out" || fail "unknown case did not list valid contracts"

make_stage "$scratch/mismatch"
printf '0%.0s' {1..64} > "$scratch/mismatch/omg-v9.9.9-x86_64-linux-arch.tar.gz.sha256"
printf '  omg-v9.9.9-x86_64-linux-arch.tar.gz\n' >> "$scratch/mismatch/omg-v9.9.9-x86_64-linux-arch.tar.gz.sha256"
assert_rc 3 "$runner" "${base_args[@]}" --staged-dir "$scratch/mismatch" --evidence-dir "$scratch/mismatch-evidence"
grep -q '"result":"HARNESS_ERROR"' "$(results_file "$scratch/mismatch-evidence")" || fail "checksum mismatch was not a harness error"

make_stage "$scratch/missing"
rm "$scratch/missing/omg-v9.9.9-x86_64-linux-arch.tar.gz.sha256"
assert_rc 3 "$runner" "${base_args[@]}" --staged-dir "$scratch/missing" --evidence-dir "$scratch/missing-evidence"
grep -q '"result":"HARNESS_ERROR"' "$(results_file "$scratch/missing-evidence")" || fail "missing sidecar was not a harness error"

make_stage "$scratch/valid"
export FAKE_CONTAINER_STATE="$scratch/container-state"
for failure_code in 120 125 126 127; do
  export FAKE_RUN_EXIT="$failure_code"
  assert_rc 3 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/launch-error-$failure_code"
  grep -q '"result":"HARNESS_ERROR"' "$(results_file "$scratch/launch-error-$failure_code")" || fail "setup or engine failure was blamed on the product"
done
unset FAKE_RUN_EXIT

export FAKE_HANG=1
assert_rc 3 "$runner" "${base_args[@]}" --timeout-seconds 1 --staged-dir "$scratch/valid" --evidence-dir "$scratch/timeout"
unset FAKE_HANG
[[ ! -e "$FAKE_CONTAINER_STATE" ]] || fail "container survived timeout"
[[ -z "$(find "$HOME/.cache/build-targets/omg-release-smoke" -mindepth 1 -print -quit)" ]] || fail "artifact scratch survived timeout"
grep -R -q 'verified absent:' "$scratch/timeout" || fail "timeout has no cleanup proof"
grep -q '"exit_code":124' "$(results_file "$scratch/timeout")" || fail "timeout code was lost"
grep -q '"result":"HARNESS_ERROR"' "$(results_file "$scratch/timeout")" || fail "timeout was reported as a product verdict"

export FAKE_CLEANUP_FAIL=1
assert_rc 3 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/cleanup-error"
unset FAKE_CLEANUP_FAIL
grep -q '"result":"HARNESS_ERROR"' "$(results_file "$scratch/cleanup-error")" || fail "unverified cleanup passed"

export FAKE_INFO_EXIT=1
assert_rc 3 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/blocked-evidence"
unset FAKE_INFO_EXIT
grep -q '"result":"BLOCKED"' "$(results_file "$scratch/blocked-evidence")" || fail "unavailable engine was not blocked"

export FAKE_RUN_EXIT=7
assert_rc 1 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/failing-evidence"
unset FAKE_RUN_EXIT
find "$scratch/tmp" -mindepth 1 -maxdepth 1 -name 'omg-release-smoke-*' -print -quit | grep -q . && fail "temporary workdir survived failing case"
work_root="$HOME/.cache/build-targets/omg-release-smoke"
[[ -d "$work_root" ]] || fail "runner did not use disk-backed artifact scratch"
[[ -z "$(find "$work_root" -mindepth 1 -print -quit)" ]] || fail "artifact scratch survived failing case"
grep -q '"result":"PRODUCT_FAIL"' "$(results_file "$scratch/failing-evidence")" || fail "failing case was not a product failure"

make_stage "$scratch/fedora" fedora
fedora_args=(--release v9.9.9 --distro fedora --case release-package-search-tree --container-engine fake-engine --staged-dir "$scratch/fedora")
assert_rc 0 "$runner" "${fedora_args[@]}" --evidence-dir "$scratch/fixed-defect"
grep -q '"result":"PASS"' "$(results_file "$scratch/fixed-defect")" || fail "fixed defect was forced to fail"
grep -q '"expectation":"known-defect"' "$(results_file "$scratch/fixed-defect")" || fail "historical expectation was lost"
export FAKE_RUN_EXIT=7
assert_rc 1 "$runner" "${fedora_args[@]}" --evidence-dir "$scratch/remaining-defect"
unset FAKE_RUN_EXIT
grep -q '"result":"PRODUCT_FAIL"' "$(results_file "$scratch/remaining-defect")" || fail "remaining defect was hidden"

export FAKE_GH_SOURCE="$scratch/valid"
assert_rc 0 "$runner" "${base_args[@]}" --evidence-dir "$scratch/published-evidence"
grep -q '"result":"PASS"' "$(results_file "$scratch/published-evidence")" || fail "published artifact path did not pass"
grep -q '"artifact_source":"published"' "$(results_file "$scratch/published-evidence")" || fail "published source is not recorded"

family_args=(--release v9.9.9 --distro arch --family package --container-engine fake-engine)
assert_rc 0 "$runner" "${family_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/family-evidence"
[[ "$(grep -c '"case_id"' "$(results_file "$scratch/family-evidence")")" -eq 3 ]] || fail "package family did not select three contracts"
grep -R -q 'install --yes tree' "$scratch/family-evidence" || fail "install probe did not preserve canonical --yes"
grep -R -q 'remove --yes tree' "$scratch/family-evidence" || fail "remove probe did not preserve canonical --yes"
if grep -R -E '(install|remove) -y tree' "$scratch/family-evidence" >/dev/null; then
  fail "probe substituted the short -y alias"
fi

export GH_TOKEN='fixture-secret-that-must-not-leak'
assert_rc 0 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/pass-evidence"
if grep -R -F "$GH_TOKEN" "$scratch/pass-evidence" >/dev/null; then
  fail "secret appeared in evidence"
fi
result="$(results_file "$scratch/pass-evidence")"
grep -q '"case_id":"release-package-search-tree"' "$result" || fail "result omits case id"
grep -q '"distro":"arch"' "$result" || fail "result omits distro"
grep -q '"result":"PASS"' "$result" || fail "result omits pass classification"
grep -q '"artifact_source":"staged"' "$result" || fail "staged result is indistinguishable from a published release"
grep -q '"exit_code":0' "$result" || fail "result omits exit code"
grep -Eq '"elapsed_seconds":[0-9]+' "$result" || fail "result omits elapsed seconds"
assert_rc 0 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/pass-evidence"
[[ "$(find "$scratch/pass-evidence" -mindepth 2 -maxdepth 2 -name results.json -type f | wc -l)" -eq 2 ]] || fail "a later invocation replaced prior aggregate evidence"

reporter="$repo_root/scripts/report-smoke-sentry.sh"
mkdir -p "$scratch/sentry-run"
printf '%s\n' '{"dsn":"https://fixturekey@o123.ingest.us.sentry.io/123"}' > "$scratch/sentry-config.json"
printf '%s\n' '[{"case_id":"release-package-search-tree","distro":"arch","result":"PRODUCT_FAIL","exit_code":1,"elapsed_seconds":2,"stderr":"fixture-private-token","environment":{"GH_TOKEN":"fixture-private-token"}}]' > "$scratch/sentry-run/results.json"
cat > "$scratch/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat > "$FAKE_SENTRY_ENVELOPE"
printf '%s' "${FAKE_SENTRY_HTTP:-200}"
EOF
chmod 700 "$scratch/bin/curl"
export OMG_SMOKE_SENTRY_CONFIG="$scratch/sentry-config.json"
export FAKE_SENTRY_ENVELOPE="$scratch/envelope.txt"
assert_rc 0 "$reporter" "$scratch/sentry-run/results.json"
if grep -q 'fixture-private-token' "$FAKE_SENTRY_ENVELOPE"; then
  fail "Sentry reporter included unapproved fields"
fi
jq -se 'length == 3 and .[1].type == "event" and .[2].extra.failures[0].result == "PRODUCT_FAIL"' "$FAKE_SENTRY_ENVELOPE" >/dev/null || fail "invalid Sentry envelope"
rm "$FAKE_SENTRY_ENVELOPE"
assert_rc 0 "$reporter" "$result"
[[ ! -f "$FAKE_SENTRY_ENVELOPE" ]] || fail "passing run sent an error report"
export FAKE_SENTRY_HTTP=429
assert_rc 1 "$reporter" "$scratch/sentry-run/results.json"
export FAKE_RUN_EXIT=7
assert_rc 1 "$runner" "${base_args[@]}" --staged-dir "$scratch/valid" --evidence-dir "$scratch/reporting-failure"
unset FAKE_RUN_EXIT FAKE_SENTRY_HTTP OMG_SMOKE_SENTRY_CONFIG FAKE_SENTRY_ENVELOPE
grep -q '"result":"PRODUCT_FAIL"' "$(results_file "$scratch/reporting-failure")" || fail "reporting failure changed the test verdict"

printf 'PASS: release smoke fixture suite\n'
