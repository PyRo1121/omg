# Platform-Specific Testing Strategy

## Overview

OMG supports 5 platform-specific package managers, each with unique APIs and behaviors:

| Platform | Package Manager | Backend | Test Coverage |
|----------|----------------|---------|---------------|
| **Arch Linux** | pacman/yay | libalpm FFI (pure Rust) | ✅ Comprehensive (`arch_tests.rs`) |
| **Debian/Ubuntu** | apt | rust-apt FFI + pure Rust | ✅ Comprehensive (`debian_tests.rs`) |
| **Fedora/RHEL** | dnf/rpm | SQLite + subprocess fallback | ✅ NEW (`fedora_tests.rs`) |
| **macOS** | Homebrew | Pure Rust (filesystem + JSON API) | ✅ NEW (`macos_tests.rs`) |
| **Windows** | Scoop | libscoop (pure Rust) | ✅ NEW (`windows_tests.rs`) |

---

## Testing Architecture

### Test Organization

```
tests/
├── arch_tests.rs              # Arch Linux pacman/yay tests
├── debian_tests.rs            # Debian/Ubuntu APT tests
├── fedora_tests.rs            # Fedora/RHEL DNF tests (NEW)
├── macos_tests.rs             # macOS Homebrew tests (NEW)
├── windows_tests.rs           # Windows Scoop tests (NEW)
└── comprehensive_tests.rs     # Cross-platform smoke tests
```

### Conditional Compilation

All platform-specific tests use `#[cfg(target_os = "...")]` to ensure they only compile on the target platform:

```rust
// Windows-only tests
#![cfg(target_os = "windows")]

// macOS-only tests
#![cfg(target_os = "macos")]

// Linux with Fedora feature
#![cfg(all(target_os = "linux", feature = "fedora"))]
```

---

## Test Categories

Each platform test suite includes 5-7 test modules:

### 1. Integration Tests (Basic Operations)
- Package manager initialization
- Search functionality
- List installed packages
- Package info lookup
- Update checking
- `is_installed()` verification

**Example**:
```rust
#[tokio::test]
async fn test_search_common_package() -> Result<()> {
    let pm = WindowsPackageManager::new();
    let results = pm.search("git").await?;
    assert!(!results.is_empty());
    Ok(())
}
```

### 2. Install/Remove Operations (Ignored by Default)
- Install test package
- Verify installation
- Remove package
- Verify removal

**Requires**: Root/admin privileges, marked with `#[ignore]`

```rust
#[tokio::test]
#[ignore]
async fn test_install_and_remove_package() -> Result<()> {
    // Actual package operations
    pm.install(&["hello".to_string()]).await?;
    assert!(pm.is_installed("hello").await);
    pm.remove(&["hello".to_string()]).await?;
    Ok(())
}
```

### 3. Error Handling
- Invalid package names
- Nonexistent packages
- Permission errors
- Empty input validation

### 4. Performance Tests
- Search latency (<100ms target)
- List installed latency (<500ms target)
- Result consistency across multiple calls

### 5. Platform-Specific Features
- **Windows**: Registry enumeration, libscoop API
- **macOS**: Cellar path detection, INSTALL_RECEIPT parsing
- **Fedora**: RPM database queries, repository metadata parsing
- **Debian**: dpkg status parsing, APT cache reading
- **Arch**: ALPM database access, AUR integration

---

## CI/CD Integration

### Matrix Strategy

The CI pipeline tests all 5 platforms in parallel:

```yaml
# Linux platforms (containers)
- Arch Linux (archlinux:latest)
- Debian (debian:bookworm)
- Fedora (fedora:latest)

# Native platforms
- macOS (macos-14, ARM64)
- Windows (windows-latest, x64)
```

### Test Execution Flow

#### Stage 1: Quick Gate (Linux only)
```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --lib  # Portable unit tests
```

#### Stage 2: Platform Matrix

**Linux (Arch/Debian/Fedora)**:
```bash
# In container with platform-specific deps
cargo build --release --features $PLATFORM
cargo nextest run --lib --features $PLATFORM
cargo nextest run --test ${PLATFORM}_tests  # NEW!
```

**macOS**:
```bash
# Native macOS runner
brew install wget  # Ensure test package exists
cargo test --lib --features macos
cargo test --test macos_tests  # NEW!
```

**Windows**:
```powershell
# Install Scoop if not present
irm get.scoop.sh | iex

# Run tests
cargo test --lib --features windows
cargo test --test windows_tests  # NEW!
```

---

## Running Tests Locally

### Prerequisites

| Platform | Requirements |
|----------|-------------|
| **Arch** | Arch Linux system with pacman |
| **Debian** | Ubuntu/Debian with apt |
| **Fedora** | Fedora/RHEL with dnf |
| **macOS** | macOS with Homebrew installed |
| **Windows** | Windows with Scoop installed |

### Quick Tests (Non-destructive)

```bash
# Run all unit tests for your platform
cargo test --lib

# Run platform-specific integration tests (safe, read-only)
cargo test --test windows_tests  # Windows
cargo test --test macos_tests    # macOS
cargo test --test fedora_tests   # Fedora
```

### Full Integration Tests (Requires Privileges)

```bash
# Run install/remove tests (requires root/admin)
cargo test --test windows_tests -- --include-ignored

# Or specific test
cargo test --test windows_tests windows_libscoop_integration::test_install_and_remove_package -- --ignored
```

---

## Test Data & Fixtures

### Test Packages

Each platform uses a small, stable package for install/remove tests:

| Platform | Test Package | Size | Reason |
|----------|-------------|------|--------|
| Windows | `hello` | ~1MB | Minimal, no dependencies |
| macOS | `hello` | ~50KB | Tiny formula, fast install |
| Fedora | `nano` | ~2MB | Common, minimal deps |
| Debian | `hello` | ~100KB | Official, stable |
| Arch | `hello` | ~15KB | Minimal |

### Search Test Queries

Common queries used across platforms for consistency:

- `"git"` - Should find packages on all platforms
- `"python"` - Tests fuzzy matching
- `"nonexistent-package-xyz-12345"` - Tests error handling

---

## Performance Benchmarks

### Target Latencies

| Operation | Target | Platform Notes |
|-----------|--------|----------------|
| `search()` | <100ms | With mmap/cache |
| `list_installed()` | <500ms | Direct filesystem/DB |
| `info()` | <50ms | Single package lookup |
| `list_updates()` | <2s | Network-dependent |

### Measurement

All `*_performance` test modules include:

```rust
#[tokio::test]
async fn test_search_performance() -> Result<()> {
    let pm = WindowsPackageManager::new();
    let start = Instant::now();
    pm.search("git").await?;
    let duration = start.elapsed();
    
    assert!(duration.as_millis() < 100, 
        "Search too slow: {:?}", duration);
    Ok(())
}
```

---

## Coverage Reporting

### Per-Platform Coverage

Coverage is tracked separately for each platform:

```bash
# Generate platform-specific coverage
cargo tarpaulin --features windows --test windows_tests
cargo tarpaulin --features macos --test macos_tests
cargo tarpaulin --features fedora --test fedora_tests
```

### CI Integration

Coverage reports are generated in CI for:
- Linux platforms (Arch, Debian, Fedora) - via containers
- macOS - via native runner
- Windows - via native runner

Combined coverage report aggregates all platforms.

---

## Debugging Failed Tests

### Check Logs

```bash
# Enable detailed logging
RUST_LOG=debug cargo test --test windows_tests

# Single test with output
cargo test --test macos_tests test_search_common_formula -- --nocapture
```

### Platform-Specific Issues

**Windows (Scoop)**:
- Ensure Scoop is installed: `scoop --version`
- Check SCOOP environment variable: `echo $env:SCOOP`
- Verify buckets: `scoop bucket list`

**macOS (Homebrew)**:
- Verify Homebrew: `brew --version`
- Check Cellar exists: `ls /opt/homebrew/Cellar` or `ls /usr/local/Cellar`
- Update formulae: `brew update`

**Fedora (DNF)**:
- Check RPM database: `ls -lh /var/lib/rpm/rpmdb.sqlite`
- Verify repositories: `dnf repolist`
- Update metadata: `dnf makecache`

---

## Adding New Platform Tests

### Checklist

When adding a new platform:

1. ✅ Create `tests/PLATFORM_tests.rs`
2. ✅ Add `#[cfg(target_os = "...")]` guard
3. ✅ Implement 5 test modules:
   - Integration tests
   - Install/remove operations (ignored)
   - Error handling
   - Performance tests
   - Platform-specific features
4. ✅ Update CI workflow (`.github/workflows/ci.yml`)
5. ✅ Add platform to this documentation
6. ✅ Update README with platform support status

### Template

```rust
#![cfg(target_os = "YOUR_OS")]

use anyhow::Result;
use omg_lib::package_managers::{YourPackageManager, PackageManager};

mod your_platform_integration {
    use super::*;

    #[tokio::test]
    async fn test_package_manager_creation() {
        let pm = YourPackageManager::new();
        assert_eq!(pm.name(), "your-pm");
    }
    
    // Add more tests...
}

mod your_platform_performance {
    // Performance tests...
}

// Add other modules...
```

---

## Current Test Statistics

| Platform | Test Files | Test Count | Coverage | CI Status |
|----------|-----------|------------|----------|-----------|
| Arch | arch_tests.rs | 25+ | 85%+ | ✅ Passing |
| Debian | debian_tests.rs | 30+ | 90%+ | ✅ Passing |
| Fedora | fedora_tests.rs | 20+ | 70%+ | ✅ NEW |
| macOS | macos_tests.rs | 22+ | 75%+ | ✅ NEW |
| Windows | windows_tests.rs | 18+ | 80%+ | ✅ NEW |

**Total Platform-Specific Tests**: 115+  
**Execution Time**: ~2-5 minutes per platform (parallel)  
**CI Runtime**: ~8-12 minutes total (all platforms)

---

## Future Enhancements

### Planned Improvements

1. **Mock Package Managers**: Cross-platform testing without actual PM installation
2. **Snapshot Testing**: Verify output format consistency
3. **Chaos Testing**: Random operation sequences
4. **Load Testing**: Concurrent package operations
5. **Security Testing**: Malicious package name injection

### Coverage Goals

- **Unit Test Coverage**: 90%+ per platform
- **Integration Test Coverage**: 75%+ per platform
- **Critical Path Coverage**: 100% (install/remove/search)

---

## Code Coverage Reporting

### Overview

OMG uses **cargo-llvm-cov** for code coverage reporting across all platforms. Coverage is collected per-platform and then aggregated into a unified report.

### Running Coverage Locally

#### Install cargo-llvm-cov

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

#### Generate Coverage for Your Platform

**Linux (Arch)**:
```bash
cargo llvm-cov --no-default-features --features arch,pgp,license \
  --lib --test arch_tests --test cross_platform_mock_tests
```

**Linux (Debian)**:
```bash
cargo llvm-cov --no-default-features --features debian,pgp,license \
  --lib --test debian_tests --test cross_platform_mock_tests
```

**Linux (Fedora)**:
```bash
cargo llvm-cov --no-default-features --features fedora,pgp,license \
  --lib --test fedora_tests --test cross_platform_mock_tests
```

**macOS**:
```bash
cargo llvm-cov --no-default-features --features macos,pgp,license \
  --lib --test macos_tests --test cross_platform_mock_tests
```

**Windows**:
```powershell
cargo llvm-cov --no-default-features --features windows,pgp,license `
  --lib --test windows_tests --test cross_platform_mock_tests
```

#### Generate HTML Report

```bash
cargo llvm-cov --no-default-features --features [PLATFORM_FEATURES] \
  --lib --test [PLATFORM_TESTS] --html

open target/llvm-cov/html/index.html
```

### CI/CD Coverage Workflow

Coverage is automatically collected in CI for all platforms:

#### Coverage Jobs (STAGE 2c)

**Per-Platform Coverage Collection**:
- Runs in parallel with platform build jobs
- Generates LCOV format coverage data
- Uploads coverage artifacts for aggregation

**Platforms Covered**:
1. Arch Linux (container: `archlinux:latest`)
2. Debian (container: `debian:bookworm`)
3. Fedora (container: `fedora:latest`)
4. macOS ARM64 (runner: `macos-14`)
5. Windows x64 (runner: `windows-latest`)

#### Coverage Aggregation (STAGE 2d)

**Merge Process**:
1. Downloads all platform coverage artifacts
2. Uses `lcov` to merge coverage reports
3. Generates unified coverage summary
4. Uploads to Codecov

**GitHub Actions Summary**:
- Overall coverage percentage
- Top 50 most-covered files
- Coverage trends over time

### Codecov Integration

Coverage reports are uploaded to [Codecov](https://codecov.io/gh/pyro1121/omg):

**Features**:
- Pull request coverage comments
- Coverage diff between branches
- Sunburst visualization per platform
- Historical coverage tracking
- Coverage badges for README

**Configuration**:
- Token stored in `secrets.CODECOV_TOKEN`
- Reports flagged with `unittests` and `integration`
- Non-blocking: Coverage failures don't fail CI

### Coverage Targets

| Category | Target | Current |
|----------|--------|---------|
| Overall | 85% | [![codecov](https://codecov.io/gh/pyro1121/omg/branch/main/graph/badge.svg)](https://codecov.io/gh/pyro1121/omg) |
| Unit Tests | 90%+ | - |
| Integration | 75%+ | - |
| Critical Paths | 100% | - |

### Interpreting Coverage Reports

**What Coverage Measures**:
- ✅ **Line Coverage**: Percentage of lines executed
- ✅ **Branch Coverage**: Percentage of conditional branches taken
- ❌ **NOT**: Code quality, correctness, or test thoroughness

**Coverage Blind Spots**:
- Panic paths and error recovery
- Platform-specific conditional compilation
- Unsafe code blocks (tested but not counted)
- Const evaluation and macros

**Best Practices**:
- Aim for high coverage, but prioritize meaningful tests
- 100% coverage ≠ bug-free code
- Focus on critical paths first
- Use coverage to find untested edge cases

### Debugging Low Coverage

#### Find Untested Code

```bash
cargo llvm-cov --open
```

Look for red-highlighted lines in the HTML report.

#### Add Targeted Tests

```rust
#[test]
fn test_uncovered_error_path() {
    let result = function_with_low_coverage(invalid_input);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Expected error message"
    );
}
```

#### Exclude Unreachable Code

```rust
#[cfg(not(tarpaulin_include))]
fn debug_only_function() {
    // Development/debug code not intended for production
}
```

### Coverage in Pull Requests

**Automated Checks**:
1. Coverage collected on all platforms
2. Codecov comments on PR with coverage diff
3. CI shows coverage summary in job output
4. No PR blocked on coverage (informational only)

**Review Guidelines**:
- New code should have ≥80% coverage
- Coverage drops >5% require justification
- Critical paths (install/remove) must be tested

---

## Best Practices

### DO
- ✅ Use `#[cfg(target_os)]` for platform-specific code
- ✅ Mark destructive tests with `#[ignore]`
- ✅ Check for required tools before testing (skip if missing)
- ✅ Clean up after install/remove tests
- ✅ Use consistent test package names
- ✅ Include performance assertions
- ✅ Test error paths, not just happy paths

### DON'T
- ❌ Run install/remove tests without `#[ignore]`
- ❌ Assume package managers are installed
- ❌ Leave test packages installed
- ❌ Hard-code absolute paths
- ❌ Skip error handling tests
- ❌ Ignore performance regressions

---

**Maintained by**: OMG Team  
**Last Updated**: 2026-01-31  
**Version**: 1.0
