#!/usr/bin/env bash
# test-qa-audit.sh — harness for scripts/qa-audit.sh with fixture evidence.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
runner="$repo_root/scripts/qa-audit.sh"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
failures=0
fail() { printf 'FAIL: %s\n' "$1" >&2; failures=$((failures + 1)); }

# Fixture run: one pass, one product fail with a transcript carrying a
# secret and ANSI escapes, one skipped row.
mkdir -p "$scratch/run/arch-search-tree"
# Assemble the PAT-shaped token at runtime so this file carries no
# contiguous credential-shaped literal for secret scan (issue #276).
# \033 is a real ESC byte: the ANSI-scrub assertion below is only
# meaningful if the transcript actually contains escapes.
pat_prefix="ghp"
pat_body="_fixturefakeaudit00000000000000000000"
fake_pat="${pat_prefix}${pat_body}"
printf '\033[31mred\033[0m\ntoken %s\nboom\n' "$fake_pat" > "$scratch/run/arch-search-tree/transcript.txt"
printf '%s' '[{"case_id":"search-tree","distro":"arch","result":"PRODUCT_FAIL","exit_code":1,"elapsed_seconds":2},{"case_id":"other","distro":"arch","result":"PASS","exit_code":0,"elapsed_seconds":1},{"case_id":"skipped-row","distro":"arch","result":"SKIPPED","exit_code":-1,"elapsed_seconds":0}]' > "$scratch/run/results.json"
printf 'case\targs_json\nsearch-tree\t["search","tree"]\n' > "$scratch/cases.tsv"
export GH_TOKEN='fixture-secret-that-must-not-leak'

out=""; rc=0
out=$("$runner" "$scratch/run/results.json" --tsv "$scratch/cases.tsv") || rc=$?
[[ "$rc" -eq 1 ]] || fail "audit with failures must exit 1, got $rc"
grep -q "verdicts:.*PRODUCT_FAIL=1" <<< "$out" || fail "audit omitted verdict counts: $out"
grep -q "### search-tree on arch: PRODUCT_FAIL (exit 1, 2s)" <<< "$out" || fail "audit omitted the failing row: $out"
grep -q "command: omg search tree" <<< "$out" || fail "audit omitted the TSV command: $out"
grep -q "boom" <<< "$out" || fail "audit omitted the excerpt"
grep -q "arch-search-tree/transcript.txt" <<< "$out" || fail "audit omitted the excerpt source"
grep -q "qa-file-issue.sh \"$scratch/run/results.json\"" <<< "$out" || fail "audit omitted the filing command"
grep -q "skipped-row" <<< "$out" && fail "audit printed a skipped row as failure"
if grep -q "fixture-secret-that-must-not-leak" <<< "$out"; then fail "audit leaked GH_TOKEN"; fi
if grep -q "ghp_fixture" <<< "$out"; then fail "audit leaked a token pattern"; fi
if grep -q $'\x1b' <<< "$out"; then fail "audit leaked ANSI escapes"; fi
grep -q "files=1 failing-rows=1" <<< "$out" || fail "audit bad summary: $out"
unset GH_TOKEN

# Clean run exits 0 and says so.
printf '%s' '[{"case_id":"other","distro":"arch","result":"PASS","exit_code":0,"elapsed_seconds":1}]' > "$scratch/clean.json"
out=$("$runner" "$scratch/clean.json"); rc=$?
[[ "$rc" -eq 0 ]] || fail "clean audit must exit 0, got $rc"
grep -q "clean" <<< "$out" || fail "clean audit printed no notice: $out"

# Directory scan finds nested results; junk files are skipped loudly.
mkdir -p "$scratch/suite/a/b"
printf '%s' '[{"case_id":"x","distro":"fedora","result":"HARNESS_ERROR","exit_code":3,"elapsed_seconds":0}]' > "$scratch/suite/a/b/results.json"
printf 'not json\n' > "$scratch/suite/a/notes.txt"
mkdir -p "$scratch/suite/bad"
printf '{"oops":true}\n' > "$scratch/suite/bad/results.json"
out=""; rc=0
out=$("$runner" "$scratch/suite") || rc=$?
[[ "$rc" -eq 1 ]] || fail "suite audit must exit 1, got $rc"
grep -q "not a results.json array; skipped" <<< "$out" || fail "suite audit did not flag the junk file"
grep -q "files=2 failing-rows=1" <<< "$out" || fail "suite audit bad summary: $out"

if [[ "$failures" -ne 0 ]]; then printf '%s failure(s)\n' "$failures" >&2; exit 1; fi
printf 'qa-audit harness: all green\n'
