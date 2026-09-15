#!/usr/bin/env bash
set -euo pipefail

workflow="${1:?workflow file is required}"
commit="${2:?commit SHA is required}"
label="${3:-$workflow}"
repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

latest_run="$(
  gh run list \
    --repo "$repository" \
    --workflow "$workflow" \
    --commit "$commit" \
    --event push \
    --limit 1 \
    --json databaseId,status,conclusion \
    --jq '.[0] | select(. != null) | [.databaseId, .status, (.conclusion // "")] | @tsv'
)"
if [[ -z "$latest_run" ]]; then
  echo "::error::No push-triggered $label run found for commit $commit" >&2
  exit 1
fi
IFS=$'\t' read -r run_id run_status conclusion <<< "$latest_run"
[[ "$run_id" =~ ^[0-9]+$ ]] || { echo "::error::Invalid workflow run identifier" >&2; exit 1; }
if [[ "$run_status" == completed && "$conclusion" == success ]]; then
  echo "$label passed for $commit in run $run_id"
  exit 0
fi
if [[ "$run_status" != in_progress && "$run_status" != queued ]]; then
  echo "::error::Latest $label run $run_id did not succeed: $run_status/$conclusion" >&2
  exit 1
fi

echo "Waiting for $label run $run_id on $commit"
if ! gh run watch "$run_id" --repo "$repository" --exit-status; then
  echo "::error::$label run $run_id failed for commit $commit" >&2
  exit 1
fi

echo "$label passed for $commit in run $run_id"
