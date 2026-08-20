# Development Scripts

Utility scripts for development, testing, and CI of the OMG project.

## Quick Reference

| Script | Purpose | Usage |
|--------|---------|-------|
| `check-perf-regression.py` | Verify no performance regressions | `python3 scripts/check-perf-regression.py` |
| `generate-benchmark-chart.py` | Create benchmark visualizations | `python3 scripts/generate-benchmark-chart.py` |
| `extract-release-notes.sh` | Extract release notes for GitHub releases | `./scripts/extract-release-notes.sh` |

---

## check-perf-regression.py

**Purpose:** Automated performance regression detection for CI/CD

**Usage:**
```bash
python3 scripts/check-perf-regression.py
python3 scripts/check-perf-regression.py --baseline 10.0
python3 scripts/check-perf-regression.py benchmark_results/search.json
```

**How it works:** Reads hyperfine JSON output, compares against a baseline threshold, and exits non-zero on regression.

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

## Script Conventions

- **Shell scripts:** shebang `#!/usr/bin/env bash`, `set -euo pipefail`, marked `+x`
- **Python scripts:** shebang `#!/usr/bin/env python3`, Python 3.8+
- **Exit codes:** `0` success, `1` general failure, `2` invalid usage, `3` missing dependencies, `4` configuration error

---

## Contributing

1. Create the script with a proper shebang.
2. Add usage documentation in docstrings/comments.
3. Make it executable: `chmod +x scripts/your-script.sh`.
4. Add an entry to this README.
5. Test locally before committing.

## Related Documentation

- **[Makefile](../Makefile)** — Common development commands
- **[CONTRIBUTING.md](../CONTRIBUTING.md)** — Contribution guidelines
- **[.github/workflows/](../.github/workflows/)** — CI/CD pipelines