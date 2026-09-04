#!/usr/bin/env bash
set -euo pipefail

workflow="${1:?workflow file is required}"
commit="${2:?commit SHA is required}"
label="${3:-$workflow}"
repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

successful_run="$(
  gh run list \
    --repo "$repository" \
    --workflow "$workflow" \
    --commit "$commit" \
    --status success \
    --limit 1 \
    --json databaseId \
    --jq '.[0].databaseId // empty'
)"
if [[ -n "$successful_run" ]]; then
  echo "$label passed for $commit in run $successful_run"
  exit 0
fi

pending_run="$(
  gh run list \
    --repo "$repository" \
    --workflow "$workflow" \
    --commit "$commit" \
    --limit 20 \
    --json databaseId,status \
    --jq '[.[] | select(.status == "in_progress" or .status == "queued")][0].databaseId // empty'
)"
if [[ -z "$pending_run" ]]; then
  echo "::error::No successful or in-progress $label run found for commit $commit" >&2
  exit 1
fi

echo "Waiting for $label run $pending_run on $commit"
if ! gh run watch "$pending_run" --repo "$repository" --exit-status; then
  echo "::error::$label run $pending_run failed for commit $commit" >&2
  exit 1
fi

echo "$label passed for $commit in run $pending_run"
