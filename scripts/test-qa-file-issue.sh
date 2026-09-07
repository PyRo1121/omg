#!/usr/bin/env bash
# test-qa-file-issue.sh — harness for scripts/qa-file-issue.sh with a fake gh.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
runner="$repo_root/scripts/qa-file-issue.sh"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin"
failures=0
fail() { printf 'FAIL: %s\n' "$1" >&2; failures=$((failures + 1)); }
assert_rc() { if [[ "$1" != "$2" ]]; then fail "expected exit $1, got $2 ($3)"; fi; }

# Fake gh: `list` prints $FAKE_ISSUES_JSON, `view` prints comment bodies,
# `create`/`comment` append their argv to the call log.
cat > "$scratch/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$CALL_LOG"
case "$1 $2" in
  "issue list") printf '%s' "${FAKE_ISSUES_JSON:-[]}";;
  "issue view") printf '%s' "${FAKE_COMMENTS_JSON:-[]}";;
  "issue create") printf 'https://github.com/x/y/issues/1\n';;
  "issue comment") printf '';;
  "issue close") printf '';;
  *) exit 9;;
esac
EOF
chmod 700 "$scratch/bin/gh"
export PATH="$scratch/bin:$PATH" CALL_LOG="$scratch/calls.log"
results="$scratch/results.json"

# 1. Fresh failure opens an issue carrying the fingerprint marker.
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"PRODUCT_FAIL","exit_code":1,"elapsed_seconds":2}]' > "$results"
: > "$CALL_LOG"; export FAKE_ISSUES_JSON='[]' FAKE_COMMENTS_JSON='[]'
out=$(bash "$runner" "$results" --run-url https://run/1 --source qemu); assert_rc 0 "$?" "create-new"
grep -q "issue create" "$CALL_LOG" || fail "create-new issued no create call"
grep -q "filed=1 updated=0 closed=0 errors=0" <<< "$out" || fail "create-new bad summary: $out"

# 2. Same fingerprint open already -> comment, never create.
export FAKE_ISSUES_JSON='[{"number":7,"body":"<!-- omg-qa-fingerprint: qemu:arch:search-tree -->"}]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/2 --source qemu); assert_rc 0 "$?" "dedup-comment"
grep -q "issue comment 7" "$CALL_LOG" || fail "dedup-comment issued no comment call"
grep -q "issue create" "$CALL_LOG" && fail "dedup-comment must not create"
grep -q "filed=0 updated=1 closed=0 errors=0" <<< "$out" || fail "dedup-comment bad summary: $out"

# 3. Run URL already recorded on the issue -> complete silence.
export FAKE_COMMENTS_JSON='["earlier https://run/2 note"]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/2 --source qemu); assert_rc 0 "$?" "same-run-skip"
grep -q "issue create\|issue comment" "$CALL_LOG" && fail "same-run-skip must stay silent"
grep -q "filed=0 updated=0 closed=0 errors=0" <<< "$out" || fail "same-run-skip bad summary: $out"

# 4. Dry run performs no mutations beyond the issue list.
export FAKE_COMMENTS_JSON='[]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/3 --source qemu --dry-run); assert_rc 0 "$?" "dry-run"
grep -q "issue create\|issue comment" "$CALL_LOG" && fail "dry-run mutated"
grep -q "would comment on #7" <<< "$out" || fail "dry-run printed no plan: $out"

# 5. Malformed results fail closed before any gh mutation.
printf '%s' '[{"case_id":"x","distro":"mars","result":"PRODUCT_FAIL","exit_code":1,"elapsed_seconds":1}]' > "$results"
: > "$CALL_LOG"
if bash "$runner" "$results" --run-url https://run/4 --source qemu 2>/dev/null; then
  fail "schema violation must fail"
fi
[[ -s "$CALL_LOG" ]] && fail "schema violation must not call gh"

# 6. Clean run files nothing (and closes nothing when no issue is open).
printf '%s' '[{"case_id":"x","distro":"arch","result":"PASS","exit_code":0,"elapsed_seconds":1}]' > "$results"
: > "$CALL_LOG"
export FAKE_ISSUES_JSON='[]'
out=$(bash "$runner" "$results" --run-url https://run/5 --source qemu); assert_rc 0 "$?" "clean-run"
grep -q "No failures to file" <<< "$out" || fail "clean-run printed no notice"
grep -q "filed=0 updated=0 closed=0 errors=0" <<< "$out" || fail "clean-run bad summary: $out"

# 7. New issue carries a scrubbed failure excerpt plus an agent runbook.
mkdir -p "$scratch/ev/arch-search-tree"
printf 'line one\n[32mgreen output[0m\nGH_TOKEN is fixture-secret-that-must-not-leak\nghp_fixturefakepattern00000000000000000000\nboom: exit 1\n' > "$scratch/ev/arch-search-tree/transcript.txt"
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"PRODUCT_FAIL","exit_code":1,"elapsed_seconds":2}]' > "$results"
: > "$CALL_LOG"; export FAKE_ISSUES_JSON='[]' GH_TOKEN='fixture-secret-that-must-not-leak'
out=$(bash "$runner" "$results" --run-url https://run/7 --source qemu --evidence-dir "$scratch/ev"); assert_rc 0 "$?" "excerpt-create"
grep -q "issue create" "$CALL_LOG" || fail "excerpt-create issued no create call"
grep -q "boom: exit 1" "$CALL_LOG" || fail "excerpt-create omitted the failure tail"
grep -q "green output" "$CALL_LOG" || fail "excerpt-create dropped plain log text"
grep -q "Agent runbook" "$CALL_LOG" || fail "excerpt-create omitted the runbook"
grep -q "arch-search-tree/transcript.txt" "$CALL_LOG" || fail "excerpt-create omitted the evidence path"
if grep -q "fixture-secret-that-must-not-leak" "$CALL_LOG"; then fail "excerpt leaked GH_TOKEN"; fi
if grep -q "ghp_fixture" "$CALL_LOG"; then fail "excerpt leaked a token pattern"; fi
if grep -q $'\x1b' "$CALL_LOG"; then fail "excerpt leaked ANSI escapes"; fi
unset GH_TOKEN

# 8. A passing case resolves its open issue (comment + close).
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"PASS","exit_code":0,"elapsed_seconds":2}]' > "$results"
export FAKE_ISSUES_JSON='[{"number":7,"state":"open","body":"<!-- omg-qa-fingerprint: qemu:arch:search-tree -->"}]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/8 --source qemu); assert_rc 0 "$?" "resolve-close"
grep -q "issue close 7" "$CALL_LOG" || fail "resolve-close issued no close call"
grep -q "filed=0 updated=0 closed=1 errors=0" <<< "$out" || fail "resolve-close bad summary: $out"

# 9. Resolve is scoped to cases present in this run: other open issues stay open.
export FAKE_ISSUES_JSON='[{"number":9,"state":"open","body":"<!-- omg-qa-fingerprint: qemu:debian:other-case -->"}]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/9 --source qemu); assert_rc 0 "$?" "resolve-scope"
grep -q "issue close\|issue comment" "$CALL_LOG" && fail "resolve-scope touched an unrelated issue"
grep -q "filed=0 updated=0 closed=0 errors=0" <<< "$out" || fail "resolve-scope bad summary: $out"

# 10. A recurrence after a close links the closed issue as a follow-up.
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"HARNESS_ERROR","exit_code":3,"elapsed_seconds":2}]' > "$results"
export FAKE_ISSUES_JSON='[{"number":9,"state":"closed","body":"<!-- omg-qa-fingerprint: qemu:arch:search-tree -->"}]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/10 --source qemu); assert_rc 0 "$?" "followup-link"
grep -q "issue create" "$CALL_LOG" || fail "followup-link issued no create call"
grep -q "Follow-up to #9" "$CALL_LOG" || fail "followup-link omitted the prior issue"
grep -q "filed=1 updated=0 closed=0 errors=0" <<< "$out" || fail "followup-link bad summary: $out"

# 11. Dry run plans closes without mutating.
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"PASS","exit_code":0,"elapsed_seconds":2}]' > "$results"
export FAKE_ISSUES_JSON='[{"number":7,"state":"open","body":"<!-- omg-qa-fingerprint: qemu:arch:search-tree -->"}]'
: > "$CALL_LOG"
out=$(bash "$runner" "$results" --run-url https://run/11 --source qemu --dry-run); assert_rc 0 "$?" "dry-run-close"
grep -q "issue close" "$CALL_LOG" && fail "dry-run-close mutated"
grep -q "would close #7" <<< "$out" || fail "dry-run-close printed no plan: $out"

if [[ "$failures" -ne 0 ]]; then printf '%s failure(s)\n' "$failures" >&2; exit 1; fi
printf 'qa-file-issue harness: all green\n'
