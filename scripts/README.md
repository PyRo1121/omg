# Development Scripts

Utility scripts for development, testing, and CI of the OMG project.

## Quick Reference

| Script | Purpose | Usage |
| -------- | --------- | ------- |
| `check-perf-regression.py` | Verify no performance regressions | `python3 scripts/check-perf-regression.py` |
| `generate-benchmark-chart.py` | Create benchmark visualizations | `python3 scripts/generate-benchmark-chart.py` |
| `extract-release-notes.sh` | Extract release notes for GitHub releases | `./scripts/extract-release-notes.sh` |
| `collect-release-artifacts.sh` | Stage release artifacts for publishing | `./scripts/collect-release-artifacts.sh <version> <artifact-dir> <release-dir>` |
| `debian-smoke-test.sh` | Debian smoke test in a container | `./scripts/debian-smoke-test.sh` |
| `gen-release-notes.sh` | Generate release notes for a version | `./scripts/gen-release-notes.sh <version>` |
| `r2-rollback.sh` | Roll back R2 release artifacts | `./scripts/r2-rollback.sh` |
| `record-benchmark-run.py` | Archive a hyperfine run into `benchmarks/records/` | `python3 scripts/record-benchmark-run.py` |
| `release-smoke.sh` | Smoke-test a published release archive in distro containers | `./scripts/release-smoke.sh --release latest --distro all` |

---

## check-perf-regression.py

**Purpose:** Automated performance regression detection for CI/CD

**Usage:**

```bash
python3 scripts/check-perf-regression.py
```

The script takes no arguments. It reads the baseline from
`benchmarks/summary.json` and the current search time from
`benchmark_results/search.json` (hyperfine export), falling back to
`benchmark_results.json` or `benchmark_report.md`.

**How it works:** Reads Hyperfine JSON output and compares the absolute search mean with two controls. The pacman comparison detects broad search-cost changes. The `omg status` comparison controls for fixed CLI startup, scheduling, and daemon IPC costs on the same runner. A run fails when the available signals regress beyond the default 35% tolerance and their 95% confidence bounds clear the limits. If a control or its distribution data is absent, the remaining signals retain the fail-closed behavior. Missing, unreadable, or corrupt baseline timing also fails closed.

**Used in:** `.github/workflows/benchmark.yml`

---

## generate-benchmark-chart.py

**Purpose:** Create visual benchmark comparison charts

**Usage:**

```bash
python3 scripts/generate-benchmark-chart.py
python3 scripts/generate-benchmark-chart.py --data benchmark_results/
python3 scripts/generate-benchmark-chart.py --output docs/assets/
```

**Requirements:** Python 3.8+, matplotlib, pandas

**Output:** PNG charts saved to `docs/assets/benchmark-comparison.png`

---

## extract-release-notes.sh

**Purpose:** Extract release notes from the changelog for GitHub releases

**Usage:**

```bash
./scripts/extract-release-notes.sh
./scripts/extract-release-notes.sh v0.1.204
./scripts/extract-release-notes.sh v0.1.204 > release-notes.md
```

**Used in:** `.github/workflows/release.yml`

---

## release-smoke.sh

**Purpose:** Verify the exact archives users download. For each selected
distro it resolves a published release with `gh`, validates the archive
against its sha256 sidecar, pulls a digest-pinned distro image, and runs the
shipped `omg` binary inside one disposable container: package-index
preparation, exact `--version` match, `omg search tree`, `omg install -y tree`,
a native installed assertion, `omg remove -y tree`, and a native removed
assertion. No retries, no soft passes.

**Usage:**

```bash
./scripts/release-smoke.sh --release latest --distro arch
./scripts/release-smoke.sh --release v0.1.217 --distro all
./scripts/release-smoke.sh --release latest --distro all --container-engine podman \
  --evidence-dir ~/.cache/omg-smoke
```

- `--release` defaults to `latest` (latest published, non-draft release); an
  explicit `vX.Y.Z` tag is also accepted.
- `--distro` is `arch`, `debian`, `ubuntu`, `fedora`, or `all` (default). With
  `all`, a semantic failure on one distro does not stop the others; the run
  still exits 1.
- `--container-engine` defaults to `$OMG_SMOKE_ENGINE`, then `docker`. The
  engine executable and `<engine> info` are checked before any network access
  (exit 3 if unavailable). Pull failures are also exit 3; semantic failures
  exit 1; invalid usage exits 2.
- Evidence: each invocation writes a unique per-distro directory (default base
  `target/release-smoke`, overridable with `--evidence-dir` or
  `$OMG_SMOKE_EVIDENCE_DIR`) containing `transcript.txt` (full shell tracing),
  `probe.sh`, and `metadata.txt` (status, release tag, image, archive digest,
  elapsed seconds). Prior evidence is never erased.

**Image provenance:** the digest pins for Arch (`archlinux`), Debian
(`debian:bookworm`), and Fedora (`fedora`) mirror the build container images in
`.github/workflows/release.yml` (`build-arch`, `build-debian`, `build-fedora`);
the Ubuntu pin (`ubuntu:24.04`) mirrors the `FROM` line of `Dockerfile.ubuntu`.
Update those sources first, then the `load_case` registry in the script.

**Used in:** `.github/workflows/release-smoke.yml` (shadow mode: runs on pull
requests touching the workflow or the script, and on `workflow_dispatch` with
an optional release tag; it uploads per-case evidence even on failure and does
not gate releases).

---

## Release Artifact Naming (canonical scheme)

All release pipelines and the installer MUST use this single naming convention.
`.github/workflows/release.yml` is the source of truth that produces these
assets; `install.sh` consumes them via the GitHub releases API.

| Platform | Archive name |
| -------- | ------------ |
| Arch Linux | `omg-v<version>-<arch>-linux-arch.tar.gz` |
| Debian | `omg-v<version>-<arch>-linux-debian.tar.gz` |
| Ubuntu | `omg-v<version>-<arch>-linux-ubuntu.tar.gz` |
| Fedora / unknown Linux distro fallback | `omg-v<version>-<arch>-linux-fedora.tar.gz` |
| macOS | `omg-v<version>-<arch>-darwin.tar.gz` |

- `<version>` is the release tag without the leading `v` (e.g. `0.1.204`).
- `<arch>` is one of `x86_64`, `aarch64`, `i686`, `armv7l` (see `detect_arch`
  in `install.sh`).
- Every archive MUST have a sidecar `<archive-name>.sha256` containing exactly
  one standard `sha256sum` entry. `install.sh` refuses missing, malformed, or
  mismatched sidecars.
- `collect-release-artifacts.sh` rejects duplicate, missing, unexpected, or
  checksum-invalid release files before GitHub publication or R2 upload.
- Any new release pipeline (local or CI) must emit exactly these names plus
  checksum sidecars; do not invent alternate schemes such as Rust target-triple
  names (`x86_64-unknown-linux-gnu`) - the installer will not select them.

---

## Script Conventions

- **Shell scripts:** shebang `#!/usr/bin/env bash`, `set -euo pipefail`, marked `+x`
- **Python scripts:** shebang `#!/usr/bin/env python3`, Python 3.8+
- **Exit codes:** `0` success, `1` general failure, `2` invalid usage, `3` missing dependencies, `4` configuration error. Two scripts predate the convention and keep their codes: `r2-rollback.sh` exits `65` (invalid semver) and `66` (missing R2 object); `debian-smoke-test.sh` exits `127` (no container engine).

---

## Contributing

1. Create the script with a proper shebang.
2. Add usage documentation in docstrings/comments.
3. Make it executable: `chmod +x scripts/your-script.sh`.
4. Add an entry to this README.
5. Test locally before committing.

## Related Documentation

- **[Makefile](../Makefile)** - Common development commands
- **[CONTRIBUTING.md](../CONTRIBUTING.md)** - Contribution guidelines
- **[.github/workflows/](../.github/workflows/)** - CI/CD pipelines
