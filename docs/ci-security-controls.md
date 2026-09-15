# CI security controls

OMG treats a successful PR, a successful main commit, and a verified published
archive as separate evidence. Publication waits for the latest successful push
runs of CI, Benchmark, Security Audit, Secret Scanning, CodeQL, Coverage, Docker
E2E and QEMU for the exact source commit.

## Repository enforcement

The active GitHub rulesets are recorded in `.github/security-rules/`:

- `23399194` requires a PR, resolved review conversations, an up-to-date branch,
  `CI Success` and `Generate Coverage Report` from the GitHub Actions integration.
  Both checks run on every PR, so documentation changes cannot deadlock on a
  path-filtered required workflow. Main cannot be deleted or force-pushed.
- `23399197` prevents updates and deletion of `v*` tags. New version tags remain
  permitted so the gated release workflow can create them.

No actor is listed for bypass. The sole-maintainer policy requires zero external
approvals; it does not claim independent human review. Administrators can still
change repository rules, so this is a technical enforcement boundary rather than
protection against the repository owner. An emergency policy change must record
the reason, affected rule and restoration evidence in a PR or issue. Do not move
an existing release tag; publish a new version.

## QEMU evidence admission

The controller verifies the installed QEMU security floor before parsing a guest
image. QEMU runs as UID/GID 65534 with seccomp, zero effective capabilities,
`NoNewPrivs`, no monitor and no root/TCG fallback. Docker bounds CPU, memory,
process count and its log rotation.

A final health check requires a live guest, complete boot-scoped kernel evidence,
no recognized fatal kernel signatures or OMG coredump records, and a running
controller without an OOM receipt. Serial evidence is independently checked.
Missing, malformed or oversized required evidence fails admission. Only selected
crash identity fields are queried; core dumps and process environments are not
uploaded. These checks detect specified failure classes, not every possible bug.

`tests/qemu-inventory-policy.json` independently records exact supported inventory
digests, selected tiers and permitted per-case skips. Changing the inventory
requires a policy review. CI always supplies the policy to the harness. Its
receipt reports selected, executed, passed, failed, blocked and skipped counts
separately; a declared CLI-shape test is not advertised as an executed VM case.
Ad hoc local inventories may run without this release policy.

## Failure reporting

`qemu-report.yml` uses `workflow_run` to report completed QEMU failures from PRs,
pushes, manual runs and scheduled runs. It checks out the default-branch SHA,
never the triggering PR. Issue-write permission and PR-failure Sentry reporting
remain in this trusted follow-up. ZIP members are parsed as bounded data without
extraction or execution. Repository, workflow, commit and run-attempt identities
are checked against GitHub before processing; old-attempt artifacts cannot close
a current failure.

Issues include the observed case, exit status, duration, commit, failed job/step,
attempt and evidence link. An exit status is not asserted to be a root cause.
Missing or invalid results create a workflow-level harness issue. More than 25
case failures also produce an aggregate issue linking the full evidence.
Superseded/cancelled and skipped runs remain visible in Actions without opening
product-failure issues. Recurring failures use the existing case fingerprint.
Only a successful push run whose commit still equals current `main` can close
matching issues. PR success and older green runs cannot authorize closure.

The reporter becomes active after its workflow lands on the default branch.
Confirm an actual completed-run report after merging; local parser tests alone
do not prove GitHub delivery.

References: [GitHub secure use](https://docs.github.com/en/actions/reference/security/secure-use),
[repository rules API](https://docs.github.com/en/rest/repos/rules),
[QEMU security](https://www.qemu.org/docs/master/system/security.html),
[journalctl](https://www.freedesktop.org/software/systemd/man/journalctl), and
[SLSA artifact verification](https://slsa.dev/spec/v1.2/verifying-artifacts).
