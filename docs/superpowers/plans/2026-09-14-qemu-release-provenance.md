# QEMU release contracts and verdicts

Evidence: scheduled run 34824771316 used v0.1.220 with the inventory at
dfcbd5a1. That release predates update --aur-only. Four passing guest lifecycle
receipts were also reported as PRODUCT_FAIL when inventory rows failed.

1. Reproduce duplicate lifecycle reporting with the existing simulated guest
   harness; assert the process fails and Sentry receives only the failed row.
2. Load published inventories from the immutable commit resolved by the release
   tag, retaining the current harness. Record the inventory revision and digest.
   Keep staged inventory coverage unchanged. Fail closed if retrieval fails.
3. Run focused shell/Python regressions, open a conventionally titled PR, and run
   both staged PR coverage and a published v0.1.220 dispatch.
4. Merge after checks pass and resolve issues 405-412 with the PR and guest evidence.

Accepted Daybreak review: exclude Sentry secrets from pull-request jobs, scope
GitHub credentials to release preparation and nightly issue filing, and replace
world-writable KVM permissions with a mandatory user ACL. Preserve reporting on
trusted schedule/dispatch runs and evidence artifacts on pull requests.
