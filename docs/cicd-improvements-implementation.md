# CI/CD Improvements - Implementation Guide

This document provides **ready-to-use code** for implementing the recommended CI/CD improvements.

---

## 🚀 Implementation #1: Add cargo-nextest

**Impact**: 40-60% faster test execution
**Time**: 30-60 minutes

### Step 1: Update `ci.yml`

Replace these lines:

```yaml
# BEFORE (lines 126-127)
- name: Run unit tests
  run: cargo test --lib --features ${{ env.ARCH_FEATURES }} --locked
```

With:

```yaml
# AFTER
- name: Install cargo-nextest
  uses: taiki-e/install-action@nextest

- name: Run unit tests
  run: cargo nextest run --lib --features ${{ env.ARCH_FEATURES }} --locked
```

### Step 2: Update `test-matrix.yml`

Replace these sections:

```yaml
# BEFORE (lines 213-223)
- name: Run unit tests
  run: |
    if [ "${{ matrix.distro }}" != "arch" ]; then export PATH="$HOME/.cargo/bin:$PATH"; fi
    FEATURES="${{ matrix.distro == 'arch' && 'arch' || 'debian' }}"
    cargo test --lib --no-default-features --features "$FEATURES" --locked

- name: Run doc tests
  run: |
    if [ "${{ matrix.distro }}" != "arch" ]; then export PATH="$HOME/.cargo/bin:$PATH"; fi
    FEATURES="${{ matrix.distro == 'arch' && 'arch' || 'debian' }}"
    cargo test --doc --no-default-features --features "$FEATURES" --locked
```

With:

```yaml
# AFTER
- name: Install cargo-nextest
  uses: taiki-e/install-action@nextest

- name: Run unit tests
  run: |
    if [ "${{ matrix.distro }}" != "arch" ]; then export PATH="$HOME/.cargo/bin:$PATH"; fi
    FEATURES="${{ matrix.distro == 'arch' && 'arch' || 'debian' }}"
    cargo nextest run --lib --no-default-features --features "$FEATURES" --locked

- name: Run doc tests (still use cargo test for doctests)
  run: |
    if [ "${{ matrix.distro }}" != "arch" ]; then export PATH="$HOME/.cargo/bin:$PATH"; fi
    FEATURES="${{ matrix.distro == 'arch' && 'arch' || 'debian' }}"
    cargo test --doc --no-default-features --features "$FEATURES" --locked
```

**Note**: Doc tests still require `cargo test --doc` as nextest doesn't support them yet.

### Step 3: Update integration tests (lines 279-282)

```yaml
# BEFORE
- name: Run integration tests
  run: |
    cargo test --test integration_suite --features arch --locked
    cargo test --test exhaustive_cli_matrix --features arch --locked
```

```yaml
# AFTER
- name: Install cargo-nextest
  uses: taiki-e/install-action@nextest

- name: Run integration tests
  run: |
    cargo nextest run --test integration_suite --features arch --locked
    cargo nextest run --test exhaustive_cli_matrix --features arch --locked
```

### Step 4: Optional - Add JUnit XML output

```yaml
- name: Run tests with JUnit output
  run: |
    cargo nextest run --lib --features arch --locked \
      --message-format json > nextest-output.json

- name: Upload test results
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: nextest-results
    path: nextest-output.json
```

---

## 📊 Implementation #2: Add Code Coverage

**Impact**: Visibility into untested code
**Time**: 1-2 hours

### Step 1: Create new workflow `.github/workflows/coverage.yml`

```yaml
name: Code Coverage

on:
  push:
    branches: [main]
    paths:
      - 'src/**'
      - 'tests/**'
      - 'Cargo.toml'
  pull_request:
    branches: [main]
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  CARGO_NET_RETRY: 10

jobs:
  coverage:
    name: Generate Coverage Report
    runs-on: ubuntu-latest
    container:
      image: archlinux:latest

    steps:
      - uses: actions/checkout@v4

      - name: Install dependencies
        run: |
          pacman -Syu --noconfirm
          pacman -S --noconfirm base-devel git rust cargo pkgconf openssl libarchive clang cmake

      - name: Fix git permissions
        run: git config --global --add safe.directory "$GITHUB_WORKSPACE"

      - name: Install coverage tools
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-llvm-cov,nextest

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
          key: coverage-registry-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: coverage-registry-

      - name: Generate coverage report
        run: |
          cargo llvm-cov nextest \
            --features arch,pgp,license \
            --locked \
            --lcov \
            --output-path lcov.info

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          token: ${{ secrets.CODECOV_TOKEN }}
          fail_ci_if_error: true
          flags: unittests
          name: codecov-umbrella

      - name: Generate HTML report (artifact)
        run: |
          cargo llvm-cov nextest \
            --features arch,pgp,license \
            --locked \
            --html

      - name: Upload HTML coverage report
        uses: actions/upload-artifact@v4
        with:
          name: coverage-report
          path: target/llvm-cov/html/
          retention-days: 30

      - name: Coverage summary
        run: |
          echo "## 📊 Coverage Report" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          cargo llvm-cov nextest \
            --features arch,pgp,license \
            --locked \
            --summary-only \
            | tee -a $GITHUB_STEP_SUMMARY
```

### Step 2: Sign up for Codecov

1. Go to https://codecov.io/
2. Sign in with GitHub
3. Enable your repository
4. Copy the Codecov token
5. Add it to your repo secrets as `CODECOV_TOKEN`:
   - Go to Settings → Secrets and variables → Actions
   - New repository secret: `CODECOV_TOKEN`

### Step 3: Add coverage badge to README

Add this to the top of your `README.md`:

```markdown
[![codecov](https://codecov.io/gh/yourusername/omg/branch/main/graph/badge.svg?token=YOUR_TOKEN)](https://codecov.io/gh/yourusername/omg)
```

---

## 🔒 Implementation #3: Add cargo-deny

**Impact**: Comprehensive supply chain security
**Time**: 30-45 minutes

### Step 1: Create `deny.toml` in project root

```toml
# deny.toml - Supply chain security policy

[advisories]
# Deny crates with security vulnerabilities
vulnerability = "deny"
# Warn about unmaintained crates
unmaintained = "warn"
# Deny yanked crates
yanked = "deny"
# Warn about security notices
notice = "warn"
# Ignore advisories for specific crates (use sparingly)
ignore = []

[licenses]
# Deny unlicensed crates
unlicensed = "deny"
# Allow specific licenses only
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
    "Zlib",
    "0BSD",
]
# Deny copyleft licenses (GPL, AGPL, etc.)
copyleft = "deny"
# Allow specific copyleft licenses if needed
# allow-osi-fsf-free = "both"
# confidence-threshold = 0.8

# Specific overrides for exceptions
[[licenses.clarify]]
name = "ring"
expression = "MIT AND ISC AND OpenSSL"
license-files = [
    { path = "LICENSE", hash = 0xbd0eed23 }
]

[bans]
# Warn when multiple versions of the same crate are used
multiple-versions = "warn"
# Deny wildcards in dependencies
wildcards = "deny"
# Allow multiple versions of these crates (for common pain points)
skip = [
    { name = "windows-sys", version = "*" },
    { name = "windows_x86_64_msvc", version = "*" },
]
# Deny specific crates (e.g., known bad actors)
deny = [
    # Example: { name = "openssl-sys", reason = "Use rustls instead" },
]
# Skip tree analysis for these crates
skip-tree = []

[sources]
# Deny crates from unknown registries
unknown-registry = "deny"
# Deny crates from unknown git repos
unknown-git = "deny"
# Allow git sources (empty = deny all git dependencies)
allow-git = []
# Allow these specific git repos
# allow-git = [
#     "https://github.com/yourusername/forked-crate",
# ]
```

### Step 2: Update `.github/workflows/audit.yml`

Replace the entire file with:

```yaml
name: Security Audit

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 0 * * *'  # Daily at midnight
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  audit:
    name: "Security Scan"
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Setup Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          prefix-key: "audit"
          cache-on-failure: true

      # Original cargo-audit
      - name: Install cargo-audit
        uses: taiki-e/install-action@cargo-audit

      - name: Run cargo-audit (Vulnerability Scan)
        run: cargo audit

      # NEW: cargo-deny
      - name: Install cargo-deny
        uses: taiki-e/install-action@cargo-deny

      - name: Run cargo-deny (Supply Chain Analysis)
        run: |
          echo "## 🔒 Supply Chain Security Report" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY

          # Check advisories
          echo "### Vulnerabilities" >> $GITHUB_STEP_SUMMARY
          cargo deny check advisories 2>&1 | tee -a $GITHUB_STEP_SUMMARY || true

          # Check licenses
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "### License Compliance" >> $GITHUB_STEP_SUMMARY
          cargo deny check licenses 2>&1 | tee -a $GITHUB_STEP_SUMMARY

          # Check bans
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "### Banned Dependencies" >> $GITHUB_STEP_SUMMARY
          cargo deny check bans 2>&1 | tee -a $GITHUB_STEP_SUMMARY

          # Check sources
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "### Source Verification" >> $GITHUB_STEP_SUMMARY
          cargo deny check sources 2>&1 | tee -a $GITHUB_STEP_SUMMARY

      - name: Generate dependency graph
        if: always()
        run: |
          cargo tree --depth 3 > dependency-tree.txt

      - name: Upload dependency tree
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: dependency-tree
          path: dependency-tree.txt
          retention-days: 30
```

### Step 3: Test locally

```bash
# Install cargo-deny
cargo install cargo-deny --locked

# Run checks
cargo deny check

# Check specific category
cargo deny check advisories
cargo deny check licenses
cargo deny check bans
cargo deny check sources
```

### Step 4: Fix any issues found

Common fixes:
- **Multiple versions**: Update dependencies to use same version
- **License issues**: Check if license is actually acceptable, update `deny.toml` if so
- **Banned dependencies**: Replace with alternatives

---

## 🤖 Implementation #4: Add Renovate

**Impact**: Automated dependency updates
**Time**: 15-30 minutes

### Option A: Renovate (Recommended)

1. Go to https://github.com/apps/renovate
2. Click "Install"
3. Select your repository
4. Create `.github/renovate.json`:

```json
{
  "$schema": "https://docs.renovatebot.com/renovate-schema.json",
  "extends": [
    "config:base",
    ":dependencyDashboard",
    ":semanticCommits",
    ":automergeDigest"
  ],
  "schedule": [
    "after 10pm every weekday",
    "before 5am every weekday",
    "every weekend"
  ],
  "timezone": "America/Chicago",
  "packageRules": [
    {
      "matchManagers": ["cargo"],
      "groupName": "Rust dependencies",
      "semanticCommitType": "chore",
      "semanticCommitScope": "deps"
    },
    {
      "matchDepTypes": ["build-dependencies", "dev-dependencies"],
      "matchManagers": ["cargo"],
      "groupName": "Rust dev dependencies",
      "automerge": true,
      "automergeType": "pr",
      "minimumReleaseAge": "3 days"
    },
    {
      "matchPackagePatterns": ["^serde"],
      "groupName": "serde ecosystem"
    },
    {
      "matchPackagePatterns": ["^tokio"],
      "groupName": "tokio ecosystem"
    },
    {
      "matchPackageNames": ["clap"],
      "enabled": false,
      "description": "Manually manage clap upgrades due to breaking changes"
    }
  ],
  "vulnerabilityAlerts": {
    "labels": ["security"],
    "addLabels": ["priority"],
    "assignees": ["@yourusername"]
  },
  "prConcurrentLimit": 5,
  "prCreation": "not-pending",
  "rebaseWhen": "behind-base-branch",
  "lockFileMaintenance": {
    "enabled": true,
    "schedule": ["before 5am on monday"]
  }
}
```

### Option B: GitHub Dependabot

Create `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "03:00"
      timezone: "America/Chicago"
    open-pull-requests-limit: 5
    reviewers:
      - "yourusername"
    assignees:
      - "yourusername"
    labels:
      - "dependencies"
      - "rust"
    commit-message:
      prefix: "chore(deps)"
      include: "scope"
    groups:
      rust-dependencies:
        patterns:
          - "*"
        exclude-patterns:
          - "clap"  # Manual upgrades for breaking changes

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    labels:
      - "dependencies"
      - "github-actions"
```

---

## ⚡ Implementation #5: Optimize sccache

**Impact**: 10-30% faster builds (on cache-cold)
**Time**: 30-60 minutes

### Option A: Keep Current Setup (Recommended if working well)

Your current setup is good! Skip this unless you're experiencing issues.

### Option B: Combine rust-cache + sccache

```yaml
# In ci.yml, replace current cache setup with:

- name: Setup Rust cache (dependencies)
  uses: Swatinem/rust-cache@v2
  with:
    prefix-key: "arch-v2"
    cache-on-failure: true
    cache-directories: |
      ~/.cargo/registry/index/
      ~/.cargo/registry/cache/
      ~/.cargo/git/db/

- name: Setup sccache (compilation artifacts)
  uses: mozilla-actions/sccache-action@v0.0.7

- name: Configure sccache
  run: |
    echo "RUSTC_WRAPPER=sccache" >> $GITHUB_ENV
    echo "SCCACHE_GHA_ENABLED=true" >> $GITHUB_ENV
```

### Option C: Use S3 Backend (for very large projects)

Only if you have >10GB cache needs:

```yaml
- name: Configure sccache with S3
  env:
    SCCACHE_BUCKET: your-rust-cache-bucket
    SCCACHE_REGION: us-east-1
    SCCACHE_S3_USE_SSL: true
    AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
    AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
  run: |
    echo "RUSTC_WRAPPER=sccache" >> $GITHUB_ENV
    sccache --start-server
```

**Setup AWS S3**:
1. Create S3 bucket: `your-rust-cache-bucket`
2. Create IAM user with S3 access
3. Add secrets to GitHub: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`

---

## 🧪 Testing Your Changes

### Test Locally

```bash
# Install tools
cargo install cargo-nextest cargo-llvm-cov cargo-deny

# Test nextest
cargo nextest run --lib --features arch

# Test coverage
cargo llvm-cov nextest --features arch --html
# Open target/llvm-cov/html/index.html

# Test cargo-deny
cargo deny check

# View coverage
xdg-open target/llvm-cov/html/index.html
```

### Test in CI

1. Create a test branch
2. Push changes
3. Open PR to trigger workflows
4. Review results in GitHub Actions
5. Check that:
   - ✅ Tests run faster with nextest
   - ✅ Coverage report generates
   - ✅ cargo-deny passes (or reports expected warnings)

---

## 📊 Expected Results

### Before Improvements
```
CI Duration: ~15-20 minutes
├── Lint: ~3 min
├── Arch build & test: ~8 min
├── Debian build & test: ~7 min
└── Ubuntu build & test: ~7 min

Coverage: Not tracked
Security: Basic (cargo-audit only)
Dependency updates: Manual
```

### After Improvements
```
CI Duration: ~10-13 minutes (35% faster)
├── Lint: ~2 min
├── Arch build & test: ~4 min (50% faster with nextest)
├── Debian build & test: ~4 min
├── Ubuntu build & test: ~4 min
└── Coverage: +2 min (new capability)

Coverage: 📊 Tracked with Codecov badge
Security: 🔒 Comprehensive (cargo-audit + cargo-deny)
Dependency updates: 🤖 Automated (Renovate)
```

---

## 🚨 Troubleshooting

### cargo-nextest issues

**Problem**: Tests fail with nextest but pass with cargo test
**Solution**: Check for test interdependencies. Nextest runs tests in parallel with better isolation.

### cargo-llvm-cov issues

**Problem**: Coverage generation fails
**Solution**: Ensure you have LLVM installed in container:
```yaml
- name: Install LLVM
  run: pacman -S --noconfirm llvm
```

### cargo-deny issues

**Problem**: Too many warnings/errors
**Solution**: Start with warnings, gradually make stricter:
```toml
[bans]
multiple-versions = "warn"  # Start with warn, not deny
```

### Renovate/Dependabot spam

**Problem**: Too many PRs
**Solution**: Adjust PR limits and grouping:
```json
"prConcurrentLimit": 3,  // Reduce from 5
"schedule": ["every weekend"]  // Less frequent
```

---

## 📚 Additional Resources

- [cargo-nextest book](https://nexte.st/)
- [cargo-llvm-cov docs](https://github.com/taiki-e/cargo-llvm-cov)
- [cargo-deny guide](https://embarkstudios.github.io/cargo-deny/)
- [Renovate docs](https://docs.renovatebot.com/)
- [Codecov docs](https://docs.codecov.com/)

---

## ✅ Implementation Checklist

Phase 1 (Quick Wins):
- [ ] Add cargo-nextest to ci.yml
- [ ] Add cargo-nextest to test-matrix.yml
- [ ] Create deny.toml config
- [ ] Update audit.yml with cargo-deny
- [ ] Set up Renovate or Dependabot
- [ ] Test locally
- [ ] Create test PR

Phase 2 (Enhanced Visibility):
- [ ] Create coverage.yml workflow
- [ ] Sign up for Codecov
- [ ] Add CODECOV_TOKEN secret
- [ ] Add coverage badge to README
- [ ] Test coverage workflow
- [ ] Review sccache performance

Phase 3 (Monitoring):
- [ ] Monitor CI performance improvements
- [ ] Review Renovate PRs
- [ ] Check coverage trends
- [ ] Adjust deny.toml as needed

---

**Last Updated**: 2026-01-26
**Status**: Ready for implementation
