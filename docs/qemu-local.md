# Running the QA pipeline locally

The pipeline is local-first: every leg runs from a checkout with no CI
involved, and the GitHub workflows mirror these same commands. CI stays
as a backup; the primary loop is your machine filing issues (and,
once enabled, PRs) that agents work while you iterate.

## Prerequisites

- Linux x86_64 with KVM (`/dev/kvm` readable+writable) and Docker
  (or podman via `--container-engine`), `jq`, `gh` (authenticated),
  coreutils. Guests need ~3 GB RAM and several GB of image downloads.
- ARM legs: an aarch64 host with KVM (e.g. `ubuntu-24.04-arm`); KVM
  cannot cross architectures, so x86_64 hosts fail ARM legs closed
  instead of emulating.
- macOS legs: any Mac with Homebrew; no container engine needed
  (`--executor native` runs on the host, which CI keeps disposable —
  locally, know it installs/removes the `tree` probe package).
- One-time: `gh label create qa-failure --description "Automated QA pipeline failures" --color B60205`
  (filing targets this label). Optional: `OMG_SMOKE_SENTRY_DSN` secret
  for Sentry reporting (absence is a visible notice, not silent).

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
./scripts/benchmark-qemu.sh --distro all --release vX.Y.Z --staged-dir <dir> --inventory-tiers container
./scripts/benchmark-qemu.sh --arch aarch64 --distro debian --release vX.Y.Z --staged-dir <arm-dir> --inventory-tiers container  # ARM host only
```

Nightly-equivalent (published release + all safe tiers, x86_64 KVM host):

```bash
./scripts/benchmark-qemu.sh --distro all --release vX.Y.Z --inventory-tiers qemu,container,network,pty --inventory-allow-mutations
```

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
3. QEMU guests per distro with `--inventory-tiers container`.
4. File with `--dry-run`, review, then file for real.
5. Summarize what failed for the audit — paste-ready, secrets scrubbed:

```bash
./scripts/qa-audit.sh ~/.cache/build-targets/omg-qemu-benchmark --tsv tests/cli_behavior_inventory.tsv
./scripts/qa-audit.sh target/release-smoke
```

Bring that output back: it is the input to the code/command audit —
no pending-row flips, no expectation edits, just errors.
