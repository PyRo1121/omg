# Running the QA pipeline locally

The pipeline is local-first: every leg runs from a checkout with no CI
involved, and the GitHub workflows mirror these same commands. CI stays
as a backup; the primary loop is your machine filing issues (and,
once enabled, PRs) that agents work while you iterate.

## Prerequisites

- Linux x86_64 with KVM (`/dev/kvm` readable+writable), Docker, `jq`,
  authenticated `gh`, and coreutils. Guests need ~3 GB RAM and several GB
  of image downloads. Container smoke also supports Podman; QEMU uses Docker.
- ARM legs: an aarch64 host with working KVM; a runner label alone does not
  prove `/dev/kvm` is available. KVM
  cannot cross architectures, so x86_64 hosts fail ARM legs closed
  instead of emulating.
- macOS legs: any Mac with Homebrew; no container engine needed
  (`--executor native` runs on the host, which CI keeps disposable —
  locally, know it installs/removes the `tree` probe package).
- One-time: `gh label create qa-failure --description "Automated QA pipeline failures" --color B60205`
  (filing targets this label). Optional Sentry reporting reads
  `~/.config/omg-smoke/sentry.json` or `OMG_SMOKE_SENTRY_CONFIG`.
  See [reporter configuration](../scripts/README.md#optional-sentry-reporting).

## What to run

Audit pins without booting anything (fast, always works):

```bash
./scripts/benchmark-qemu.sh --print-pins
```

Fast hermetic gate before any guest (runs the fixture suites):

```bash
./scripts/test-release-smoke.sh
./scripts/test-qa-file-issue.sh
```

Container smoke against your local build (no release needed) — package
exactly like `.github/workflows/release.yml` does, then:

```bash
./scripts/release-smoke.sh --release vX.Y.Z --distro all --staged-dir <dir>
```

Native macOS smoke (on a Mac):

```bash
./scripts/release-smoke.sh --release vX.Y.Z --distro macos --executor native
```

Full QEMU guests (needs KVM + Docker):

```bash
./scripts/benchmark-qemu.sh --distro all --release vX.Y.Z --staged-dir <dir> --inventory-tiers hermetic,container --inventory-allow-mutations
./scripts/benchmark-qemu.sh --arch aarch64 --distro debian --release vX.Y.Z --staged-dir <arm-dir> --inventory-tiers hermetic,container --inventory-allow-mutations  # ARM host only
```

Nightly-equivalent (published release + all safe tiers, x86_64 KVM host):

```bash
./scripts/benchmark-qemu.sh --distro all --release vX.Y.Z --inventory-tiers hermetic,qemu,container,network,pty --inventory-allow-mutations
```

The package lifecycle also requires a nonempty privileged audit log and runs
`omg audit verify` against it. Directory modes and verification output are saved
under guest evidence. See [Linux audit storage and migration](security.md#audit-logging).

## What inventory results prove

The inventory currently has 184 rows: 167 hermetic-tier contracts, three
container package contracts, and 14 declaration-only rows. The `hermetic`
tier names the fixture-based Rust test contracts. Running these rows in a
real guest is not hermetic: runtime downloads and other network operations
still need network access. Guest images are pinned, but package index refreshes
use live repositories. Saved repository hashes and package versions identify the
observed run, not a promise of identical future repository contents.
Do not report all commands tested when declared,
credentialed, or otherwise gated rows were skipped.

The runner validates the inventory before executing commands. It replays
per-row prerequisite chains in fresh working directories, expands `${ROOT}`
as a literal fixture path, and checks declared JSON and artifact assertions
when the command succeeds. A known defect remains a failure, not a pass.
The guest fixture provides Podman on Fedora and requires no container engine on
the other three images. Container command exit expectations reflect that fixture.
An unexpected engine configuration fails setup rather than changing expectations.
Gated or failed prerequisites block their dependents. Missing receipts,
transport failures, and empty execution selections fail the harness.

A guest-side supervisor records completed CLI exits separately from executor
exits. A CLI returning 125 is not a timeout-tool failure. Guest-side deadlines
terminate commands independently of SSH. Each row
retains stdout, stderr, prerequisite output, and a completion receipt under
`inventory/rows/`. `inventory/input-sha256.txt` identifies the runner and TSV.
`inventory/metadata.json` records the binary path, tiers, deadlines, and opt-ins. Existing inventory evidence cannot be overwritten.
Working directories and installed runtime state disappear with the guest;
only cwd-local fixtures are isolated per row, not the guest's home directory
or package database. Native ARM and macOS results require their own runners.

## Evidence contracts and sources

- [GNU timeout](https://www.gnu.org/software/coreutils/manual/html_node/timeout-invocation.html)
  defines exit 124 for a deadline, 125 for a timeout-tool failure, and 126 or 127
  for invocation failures. Exit 137 alone does not identify what received SIGKILL
  or prove an out-of-memory event. Guest receipts and cleanup evidence are needed
  in addition to the transport exit.
- [OpenSSH exit status](https://man.openbsd.org/ssh#EXIT_STATUS) returns the remote
  command status, or 255 for an SSH error. The runner therefore requires a guest
  completion receipt before accepting a product exit.
- [Podman exec exit status](https://docs.podman.io/en/latest/markdown/podman-exec.1.html#exit-status)
  uses 125 for Podman errors. The missing-container fixture on Fedora returns
  that status through OMG. The guest supervisor distinguishes it from a
  timeout-tool failure with the same number.
- [GitHub release assets](https://docs.github.com/en/rest/releases/assets#get-a-release-asset)
  expose a `digest` field. The Python installation row uses 3.12.14 from
  [PBS release 20260901](https://github.com/astral-sh/python-build-standalone/releases/tag/20260901),
  whose standard x86_64 GNU/Linux archive has digest
  `sha256:936c246dfdbbfa7cb22dd01814a21f582a892689fae96b06071a5e433baffa22`.
  This identifies the observed asset, not a guarantee that future assets have digests.
- [Sentry fingerprints](https://docs.sentry.io/platforms/javascript/guides/node/enriching-events/fingerprinting/)
  control issue grouping. Local `reporting.log` proves an attempted event ID and
  HTTP intake acceptance or failure. Indexed visibility requires looking up that
  event ID in Sentry. See [reporter limits](../scripts/README.md#optional-sentry-reporting).

These references define tool contracts. A passing OMG claim additionally needs
its frozen runner and inventory hashes, artifact checksum, guest identity,
selected row results, and completion receipt. A documentation citation is not
execution evidence.

## Filing issues (and the coming PRs) from a local run

Dry-run first, file second. Use a unique run URL per local run so
repeat runs comment rather than staying silent:

```bash
RUN_URL="local-$(hostname)-$(date -u +%Y%m%dT%H%M%SZ)"
./scripts/qa-file-issue.sh <run-dir>/results.json --run-url "$RUN_URL" --source qemu-matrix --dry-run
./scripts/qa-file-issue.sh <run-dir>/results.json --run-url "$RUN_URL" --source qemu-matrix
```

Notes:

- `--repo` defaults to your checkout's repo via `gh repo view`
  (forks file to the fork); override with `--repo owner/name`.
- Evidence dir defaults to the results file's directory; excerpts come
  from transcripts / `guest-check.log` / inventory row logs.
- The filed issue's runbook has the rerun command; green reruns
  auto-close it. See `docs/qa-loop.md` for the full contract.
- Fingerprints are shared between local and CI runs (same `--source`
  names), so a CI nightly and your local run update the same issues
  instead of duplicating them.

## Opening fix PRs from [qa] issues (opt-in, drafts only)

Nothing opens PRs unless you run this. From a clean tree with your fix
committed on a branch:

```bash
./scripts/qa-open-pr.sh --issue 123 --branch fix/search-tree-arch --dry-run
./scripts/qa-open-pr.sh --issue 123 --branch fix/search-tree-arch
```

The script refuses non-`[qa]` issues, closed issues, dirty trees, and
unknown branches; then pushes the branch and opens a **draft** PR with
`Fixes #123` plus a verification checklist. Verify per the checklist,
promote from draft, merge — merging closes the issue (the nightly
resolve-on-green would close it too).

## Evidence map

- Release smoke: `<evidence-base>/run-*/<distro>-<case>/{transcript.txt,metadata.txt,probe.sh,result.json}`, aggregate `results.json`.
- QEMU: `<root>/run-*/{guest-check.log,guest/evidence/,inventory/{results.json,rows/*.log},metadata.txt,kvm-probe.log,results.json}`.
- Suites: `<root>/suite-*/results.json` aggregates per-distro runs.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `KVM device /dev/kvm is missing` | No KVM here; run on a KVM host. Never bypass with `OMG_QEMU_ALLOW_NO_KVM=1` outside tests. |
| `guest arch aarch64 needs a aarch64 host` | Cross-arch emulation refused by design; use an ARM host. |
| `no published aarch64-linux archives` | ARM legs need `--staged-dir` with arm64-native builds. |
| `no aarch64 cloud image pinned for arch` | Upstream publishes no ARM Arch image; arch is x86_64-only. |
| `container engine … not found` / `info failed` | Start Docker (or pass `--container-engine podman`). |
| `gh: Requires authentication` | `gh auth login`; filing and published downloads need it. |
| `Sentry reporting disabled` | Set `OMG_SMOKE_SENTRY_DSN` or ignore (results are unaffected). |
| `invalid identifier` inventory row | Bad TSV case id; fix the row — it fails loud, never executes. |

## First run-through checklist (feeds the error audit)

1. `--print-pins` + all fixture suites green (`test-release-smoke`,
   `test-qa-file-issue`, `test-qa-open-pr`, `test-qa-audit`).
2. Container smoke on your build, all four distros.
3. QEMU guests per distro with `--inventory-tiers hermetic,container --inventory-allow-mutations`.
4. File with `--dry-run`, review, then file for real.
5. Summarize what failed for the audit — paste-ready, secrets scrubbed:

```bash
./scripts/qa-audit.sh ~/.cache/build-targets/omg-qemu-benchmark --tsv tests/cli_behavior_inventory.tsv
./scripts/qa-audit.sh target/release-smoke
```

Bring that output back: it is the input to the code/command audit —
no pending-row flips, no expectation edits, just errors.
