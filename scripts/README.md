# Development Scripts

This directory contains utility scripts for development, testing, and maintenance of the OMG project.

---

## Quick Reference

| Script | Purpose | Usage |
|--------|---------|-------|
| `check-perf-regression.py` | Verify no performance regressions | `python3 scripts/check-perf-regression.py` |
| `generate-benchmark-chart.py` | Create benchmark visualizations | `python3 scripts/generate-benchmark-chart.py` |
| `benchmark-debian.sh` | Benchmark on Debian/Ubuntu | `./scripts/benchmark-debian.sh` |
| `test-all.sh` | Run comprehensive test suite | `./scripts/test-all.sh` |
| `test-debian.sh` | Test Debian-specific features | `./scripts/test-debian.sh` |
| `docker-test.sh` | Test in Docker containers | `./scripts/docker-test.sh` |
| `debian-smoke-test.sh` | Quick Debian validation | `./scripts/debian-smoke-test.sh` |
| `generate-changelog.sh` | Generate CHANGELOG.md | `./scripts/generate-changelog.sh` |
| `update-changelog.sh` | Update changelog entry | `./scripts/update-changelog.sh` |
| `extract-release-notes.sh` | Extract notes for release | `./scripts/extract-release-notes.sh` |
| `enhance-commit-messages.py` | Improve commit quality | `./scripts/enhance-commit-messages.py` |
| `install-hooks.sh` | Install git hooks | `./scripts/install-hooks.sh` |
| `sync-wiki.sh` | Sync docs to wiki | `./scripts/sync-wiki.sh` |

---

## Benchmarking

### check-perf-regression.py

**Purpose:** Automated performance regression detection for CI/CD

**Usage:**
```bash
# Check latest benchmark results
python3 scripts/check-perf-regression.py

# Specify custom baseline
python3 scripts/check-perf-regression.py --baseline 10.0

# Check specific JSON file
python3 scripts/check-perf-regression.py benchmark_results/search.json
```

**How it works:**
1. Reads hyperfine JSON benchmark output
2. Compares against baseline threshold
3. Exits with error code if regression detected
4. Outputs detailed comparison

**Used in:** `.github/workflows/benchmark.yml`

### generate-benchmark-chart.py

**Purpose:** Create visual benchmark comparison charts

**Usage:**
```bash
# Generate charts from latest benchmarks
python3 scripts/generate-benchmark-chart.py

# Specify custom data directory
python3 scripts/generate-benchmark-chart.py --data benchmark_results/

# Output to specific directory
python3 scripts/generate-benchmark-chart.py --output docs/assets/
```

**Requirements:**
- Python 3.8+
- matplotlib
- pandas

**Output:**
- PNG charts comparing OMG vs competitors
- Saved to `docs/assets/benchmark-comparison.png`

### benchmark-debian.sh

**Purpose:** Run benchmarks on Debian/Ubuntu systems

**Usage:**
```bash
# Run full benchmark suite
./scripts/benchmark-debian.sh

# Fast mode (fewer iterations)
./scripts/benchmark-debian.sh --fast
```

**What it tests:**
- Search performance (vs apt-cache, Nala)
- Info queries
- Explicit package listing
- Update checks

**Requirements:**
- Debian or Ubuntu system
- hyperfine installed (`apt install hyperfine`)
- OMG installed and daemon running

---

## Testing

### test-all.sh

**Purpose:** Comprehensive test suite for all platforms

**Usage:**
```bash
# Run all tests
./scripts/test-all.sh

# Verbose mode
./scripts/test-all.sh -v

# Skip slow tests
./scripts/test-all.sh --fast
```

**What it tests:**
- Unit tests (`cargo test --lib`)
- Integration tests
- CLI smoke tests
- Daemon functionality
- Platform-specific features

**Exit codes:**
- `0` - All tests passed
- `1` - Test failures
- `2` - Build failures

### test-debian.sh

**Purpose:** Test Debian/Ubuntu-specific functionality

**Usage:**
```bash
# Run Debian tests
./scripts/test-debian.sh

# Test in Docker
./scripts/test-debian.sh --docker
```

**What it tests:**
- APT integration
- debian-packaging backend
- Package database parsing
- Dependency resolution

**Requirements:**
- Debian or Ubuntu system (or Docker)
- Build dependencies installed

### docker-test.sh

**Purpose:** Test OMG in isolated Docker containers

**Usage:**
```bash
# Test on all platforms
./scripts/docker-test.sh

# Test specific platform
./scripts/docker-test.sh --platform debian
./scripts/docker-test.sh --platform ubuntu

# Keep containers for debugging
./scripts/docker-test.sh --no-cleanup
```

**Platforms tested:**
- Debian Bookworm
- Ubuntu 24.04

**What it does:**
1. Builds OMG from source in container
2. Runs smoke tests
3. Validates package operations
4. Cleans up containers

### debian-smoke-test.sh

**Purpose:** Quick validation of Debian build

**Usage:**
```bash
./scripts/debian-smoke-test.sh
```

**What it tests:**
- Binary builds successfully
- Help text displays
- Basic search works
- Daemon starts

**Use case:** Quick sanity check before full test suite

---

## Release Management

### generate-changelog.sh

**Purpose:** Generate CHANGELOG.md from git history

**Usage:**
```bash
# Generate full changelog
./scripts/generate-changelog.sh

# Generate for specific version range
./scripts/generate-changelog.sh v0.1.200..HEAD

# Output to file
./scripts/generate-changelog.sh > CHANGELOG.md
```

**Format:**
- Groups commits by type (feat, fix, docs, etc.)
- Links to GitHub issues/PRs
- Follows Keep a Changelog format

### update-changelog.sh

**Purpose:** Add new entry to existing changelog

**Usage:**
```bash
# Add entry for latest commit
./scripts/update-changelog.sh

# Add entry for specific version
./scripts/update-changelog.sh v0.1.204
```

**What it does:**
1. Parses recent commits
2. Categorizes changes
3. Updates CHANGELOG.md
4. Maintains formatting

### extract-release-notes.sh

**Purpose:** Extract release notes from CHANGELOG for GitHub releases

**Usage:**
```bash
# Extract latest version notes
./scripts/extract-release-notes.sh

# Extract specific version
./scripts/extract-release-notes.sh v0.1.204

# Output to file
./scripts/extract-release-notes.sh v0.1.204 > release-notes.md
```

**Used in:** `.github/workflows/release.yml`

---

## Development Tools

### enhance-commit-messages.py

**Purpose:** Improve commit message quality using AI suggestions

**Usage:**
```bash
# Analyze last commit
python3 scripts/enhance-commit-messages.py

# Analyze specific commit
python3 scripts/enhance-commit-messages.py abc1234

# Interactive mode
python3 scripts/enhance-commit-messages.py --interactive
```

**What it does:**
- Checks commit message format
- Suggests improvements
- Validates against Conventional Commits
- Optionally rewrites commit

**Use case:** Pre-push hook or manual quality check

### install-hooks.sh

**Purpose:** Install git hooks for development workflow

**Usage:**
```bash
# Install all hooks
./scripts/install-hooks.sh

# Install specific hook
./scripts/install-hooks.sh pre-commit
./scripts/install-hooks.sh pre-push
```

**Hooks installed:**
- `pre-commit` - Format check, clippy
- `pre-push` - Run tests
- `commit-msg` - Validate commit message format

### sync-wiki.sh

**Purpose:** Synchronize documentation to GitHub wiki

**Usage:**
```bash
# Sync all docs
./scripts/sync-wiki.sh

# Sync specific file
./scripts/sync-wiki.sh docs/performance-tips.md

# Dry run
./scripts/sync-wiki.sh --dry-run
```

**What it syncs:**
- Markdown files from `docs/`
- README.md
- CONTRIBUTING.md

**Requirements:**
- Git credentials configured
- Wiki repository cloned

---

## Common Workflows

### Before Committing

```bash
# Format and lint
cargo fmt
cargo clippy --features arch -- -D warnings

# Run tests
./scripts/test-all.sh

# Check performance
python3 scripts/check-perf-regression.py
```

### Before Releasing

```bash
# Update changelog
./scripts/update-changelog.sh

# Run full test suite
./scripts/test-all.sh

# Test on all platforms
./scripts/docker-test.sh

# Generate release notes
./scripts/extract-release-notes.sh v0.1.204
```

### Performance Regression Investigation

```bash
# Run benchmark
./benchmark-hyperfine.sh

# Check for regressions
python3 scripts/check-perf-regression.py

# Generate comparison charts
python3 scripts/generate-benchmark-chart.py

# Analyze results
cat benchmark_results/search.json
```

---

## Script Conventions

### Shell Scripts

- **Shebang:** `#!/usr/bin/env bash`
- **Error handling:** `set -euo pipefail`
- **Help text:** `--help` flag prints usage
- **Executable:** Marked with `+x` permission

### Python Scripts

- **Shebang:** `#!/usr/bin/env python3`
- **Version:** Python 3.8+
- **Dependencies:** Listed in docstring
- **Help text:** `--help` via argparse

### Exit Codes

- `0` - Success
- `1` - General failure
- `2` - Invalid usage
- `3` - Missing dependencies
- `4` - Configuration error

---

## Contributing

### Adding New Scripts

1. Create script with proper shebang
2. Add usage documentation in docstring/comments
3. Make executable: `chmod +x scripts/your-script.sh`
4. Add entry to this README
5. Test locally before committing

### Script Guidelines

- Keep scripts focused (single responsibility)
- Add error handling and validation
- Print helpful error messages
- Support `--help` flag
- Follow existing naming conventions
- Document all dependencies

---

## Troubleshooting

### Script Fails with "Permission Denied"

```bash
chmod +x scripts/script-name.sh
```

### Python Script Missing Dependencies

```bash
# Install required packages
pip install matplotlib pandas
```

### Docker Tests Fail

```bash
# Ensure Docker is running
systemctl status docker

# Clean up old containers
docker system prune
```

### Benchmark Script Errors

```bash
# Install hyperfine
pacman -S hyperfine  # Arch
apt install hyperfine  # Debian/Ubuntu

# Start daemon
omg daemon
```

---

## Related Documentation

- **[Makefile](../Makefile)** - Common development commands
- **[CONTRIBUTING.md](../CONTRIBUTING.md)** - Contribution guidelines
- **[.github/workflows/](../.github/workflows/)** - CI/CD pipelines

---

**Questions?** See [CONTRIBUTING.md](../CONTRIBUTING.md) or open an issue.
