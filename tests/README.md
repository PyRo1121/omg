# OMG Integration Test Suite

The suite mixes production-boundary tests, mock-backend contracts, dependency
characterization tests, and opt-in live-system tests. Passing counts and line
coverage alone do not establish that these tests protect their named behavior.

**Do not run the whole suite against a valuable host.** Some tests bypass the
shared CLI runner, attempt runtime downloads, or call native package tools.
`OMG_TEST_MODE=1` is not a filesystem, network, or privilege sandbox. The
`docker_tests` feature also enables execution flags without proving that the
process is inside a disposable container.

The shared CLI runner isolates HOME, XDG and OMG directories and closes stdin.
`TestProject` preserves its isolated home and package state between calls.
These protections do not apply to direct library calls or independently
constructed processes. Run broad verification in a disposable environment
with no credentials, no host package access, bounded timeouts, and build/test
scratch outside RAM-backed `/tmp`.

## Test-quality audit

The 2026-09-05 audit record is in
[`.audit/deep-audit.tsv`](../.audit/deep-audit.tsv), under `test-audit-*` rows.
It distinguishes experimentally proven false-green tests from open static
review candidates. A `coverage_*.rs` filename is not evidence of a weak test;
many of those files exercise real state transitions, IPC, and error paths.

Four controlled mutations exposed gaps in the old tests: full-secret leakage,
acceptance of altered PGP data, omitted intermediate-certificate expiry checks,
and omitted hostile runtime-pin validation. The strengthened tests reject
those same mutations and pass with unmodified production code.

The audit also replaced crash-string checks with a process-completion oracle,
parsed JSON rather than matching fragments, corrected ambiguous fixtures, and
isolated shell configuration writes. Runtime-detection fixtures now assert an
activation-link transition using seeded installations, not a live download.

Do not interpret the reported pass count as the number of behaviors proven.
Some tests return successfully when prerequisites or opt-in flags are absent.
Remaining work includes these silent skips, production paths replaced by test
replicas, and network dependencies in tests described as hermetic. Broad local
verification covers the Arch feature set; it does not certify every platform,
ignored live-system test, doctest, or fuzz campaign.

## Philosophy

1. **Pin observable behavior**: assertions must be able to fail on a real
   regression; panic-string greps alone are not coverage
2. **Isolate everything**: unique data dirs per invocation; no test may
   mutate process-global state without restoring it
3. **Prove the claimed fault**: establish a passing control, inject one fault,
   and require the expected failure. Repair misleading fixtures or rename
   characterization tests; do not delete tests merely to improve pass counts.
4. **Real-system coverage is opt-in** via the environment flags below

## Running Tests

### Quick Start

```bash
# Run unit tests inside a disposable environment
cargo test --lib

# Run all integration tests
cargo test --test integration_suite

# Run specific test file
cargo test --test version_tests
cargo test --test update_integration
cargo test --test error_tests
cargo bench --features arch
```

### Environment Variables

Tests use environment variables to control behavior:

```bash
# Enable tests that require real system access (pacman, ALPM)
export OMG_RUN_SYSTEM_TESTS=1

# Enable tests that actually install/update packages (use with caution!)
export OMG_RUN_DESTRUCTIVE_TESTS=1
```

### Example: Run System Tests

```bash
OMG_RUN_SYSTEM_TESTS=1 cargo test --test integration_suite --features arch
```

### Example: Run Benchmarks

Use the repository's Criterion benchmarks explicitly when measuring performance:

```bash
cargo bench --features arch
```

## Test Files

### `version_tests.rs`

**Purpose**: Test REAL version parsing and comparison logic

**Features**:

- Tests actual Arch Linux version strings
- Verifies `alpm_types::Version` correctness
- Tests update detection logic
- Validates version comparison operators

**What it tests**:

- Real package versions from Arch repos
- Version comparison (greater than, less than, equality)
- Update detection scenarios
- Edge cases (empty versions, very long versions)

**Running**:

```bash
cargo test --test version_tests --features arch
```

### `update_integration.rs`

**Purpose**: Test REAL update command functionality

**Features**:

- Tests `omg update --check` behavior
- Verifies `--yes` flag handling
- Tests non-interactive mode errors
- Measures update check performance

**What it tests**:

- Update check returns correct status
- `--yes` flag works without TTY
- Helpful error messages in non-interactive mode
- Update command doesn't hang

**Running**:

```bash
OMG_RUN_SYSTEM_TESTS=1 cargo test --test update_integration --features arch
```

### `error_tests.rs`

**Purpose**: Verify errors are handled gracefully

**Features**:

- Tests helpful error messages
- Verifies panic prevention
- Tests permission error handling
- Validates network error messages

**What it tests**:

- Invalid input shows helpful errors
- Missing permissions suggest sudo
- Network errors suggest checking connection
- Corrupted database is handled gracefully

**Running**:

```bash
cargo test --test error_tests --features arch
```

### `benches/`

**Purpose**: Measure performance with Criterion benchmarks rather than pass/fail timing assertions.

The benchmark targets cover package operations, daemon behavior, decompression, database access, and I/O. Results are statistical and should be compared across runs instead of enforced by host-dependent millisecond thresholds.

**Running**:

```bash
cargo bench --features arch
```

### `integration_suite.rs`

**Purpose**: Comprehensive integration testing

**Features**:

- Tests all major commands
- Validates CLI argument parsing
- Tests shell completion generation
- Verifies environment management

**What it tests**:

- All subcommands work correctly
- Help text is complete
- Configuration management
- Runtime switching
- Team sync workflows

**Running**:

```bash
OMG_RUN_SYSTEM_TESTS=1 cargo test --test integration_suite --features arch
```

## Code Coverage

To generate coverage reports:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML coverage report
cargo tarpaulin --out Html --features arch

# Generate terminal coverage report
cargo tarpaulin --features arch
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Install Arch Linux packages (for ALPM)
        run: |
          sudo apt-get update
          sudo apt-get install -y libalpm-dev

      - name: Cache cargo registry
        uses: Swatinem/rust-cache@v2

      - name: Run tests
        env:
          OMG_RUN_SYSTEM_TESTS: 1
        run: |
          cargo test --features arch --all
          cargo test --test version_tests --features arch
          cargo test --test update_integration --features arch
          cargo test --test error_tests --features arch
```

## Test Categories

### Unit Tests (`cargo test --lib`)

Fast tests that don't require system access:

- Version parsing logic
- Type definitions
- Error type creation
- Configuration parsing

### Integration Tests (`cargo test --test *`)

Tests that run the full binary:

- CLI argument parsing
- Command execution
- Output verification
- Error handling

### System Tests (`OMG_RUN_SYSTEM_TESTS=1`)

Tests that require real package managers:

- pacman/alpm operations
- Real package database access
- File system operations
- Network access (optional)

### Destructive Tests (`OMG_RUN_DESTRUCTIVE_TESTS=1`)

Tests that modify the system:

- Actual package installation
- System updates
- Package removal

**⚠️ Use with caution** - These tests make real changes to the system!

## Debugging Failed Tests

### Enable Test Output

```bash
# Show all test output
cargo test -- --nocapture

# Show test names
cargo test -- --test-threads=1 -- --nocapture

# Run one test with full output
cargo test --test version_tests test_real_arch_package_versions -- --nocapture
```

### Check Test Logs

```bash
# Run tests with logging
RUST_LOG=debug cargo test --test update_integration
```

### Use Rust Backtrace

```bash
# Get detailed backtrace on panic
RUST_BACKTRACE=1 cargo test --test error_tests
```

## Contributing Tests

### Writing a New Test

1. **Determine test type**: Unit, integration, system, or performance
2. **Choose appropriate file**:
   - Version parsing → `version_tests.rs`
   - Update logic → `update_integration.rs`
   - Error scenarios → `error_tests.rs`
   - Performance → `benches/` Criterion targets
   - General integration → `integration_suite.rs`
3. **Drive real seams**: the shared `common::run_omg` runner (isolated dirs)
   or direct `omg_lib` API calls — never a mock that only exists inside the
   test
4. **Add helpful assertions**: verify real behavior, not just "doesn't
   panic"; assert the specific message/exit code
5. **Document the test**: Explain what and why

### Test Naming Convention

```rust
#[test]
fn test_<module>_<feature>_<condition>() {
    // Example: test_update_check_shows_updates
    // Example: test_version_comparison_with_release_numbers
}
```

### Test Organization

Organize tests into logical modules:

```rust
mod module_name {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // Test code
    }

    #[test]
    fn test_edge_case() {
        // Test edge case
    }
}
```

## Performance Regression Detection

### Benchmark Your Changes

```bash
# Before changes
cargo bench --features arch

# Make your changes

# After changes
cargo bench --features arch

# Compare results
```

### Acceptable Variance

Performance tests allow some variance (±20%) for:

- System load differences
- CI environment variability
- Cold vs warm cache

Consistent failures across multiple runs indicate a real regression.

## Security Testing

Tests verify security aspects:

1. **Input Validation**: Rejects dangerous inputs
2. **Privilege Separation**: Correctly requires root
3. **Injection Prevention**: Handles special characters
4. **Path Safety**: Validates file paths

## Troubleshooting

### Skip accounting

Tests that skip themselves at runtime (missing system tests, non-Arch host,
empty package index, …) print a `[omg-skip] <reason>` line. Recover the true
skip count for a run with:

```bash
cargo test 2>&1 | grep -c '\[omg-skip\]'
```

A green run with a large skip count is **not** full coverage — wire this into
CI so silent coverage loss is visible. Prefer `#[ignore = "reason"]` for
statically-known skips so they appear in `cargo test -- --ignored` listings.

### Live-service lanes (manual)

`tests/integration/security_real_world.rs` exercises real keyservers and
package backends. It stays `#[ignore]`d and runs by hand with
`cargo test -- --ignored`, never in CI: live services cannot gate merges.
Keyserver behavior is otherwise pinned by the inline unit tests in
`src/core/security/keyserver.rs`, which run offline fixtures and are the
coverage boundary for that module.

### "Skipping system test"

If tests skip with this message:

```bash
export OMG_RUN_SYSTEM_TESTS=1
```

### "Skipping destructive test"

If tests skip with this message:

```bash
export OMG_RUN_DESTRUCTIVE_TESTS=1
```

**Warning**: This will actually install/remove packages!

### ALPM Not Available

If tests fail with "ALPM not available":

```bash
# Install libalpm-dev
sudo apt-get install libalpm-dev  # Ubuntu/Debian
sudo pacman -S alpm-lib         # Arch
```

## Test Data

### Temporary Files

Tests use `tempfile` crate for temporary directories:

```rust
use tempfile::TempDir;

let temp_dir = TempDir::new().unwrap();
// Use temp_dir.path() for test data
// Automatically cleaned up on drop
```

### Test Packages

Tests use well-known packages:

- `pacman` - Core package manager
- `firefox` - Popular browser (extra repo)
- `git` - Version control (extra repo)
- `bash` - Core shell (core repo)

These packages are:

- Available on all Arch systems
- Stable and maintained
- Small enough for fast tests

## Best Practices

1. **Test REAL behavior**, not just code paths
2. **Use meaningful assertions**, not just `assert!(!result.is_empty())`
3. **Test error messages** are helpful
4. **Measure performance** with actual timing
5. **Clean up resources** using `Drop` or `TempDir`
6. **Document edge cases** in comments
7. **Avoid flaky tests** - use reliable, deterministic scenarios

## Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [ALPM Documentation](https://archlinux.org/pacman/alpm.3.html)
- [OMG Architecture](../README.md#-architecture)
- [CLAP Derive](https://docs.rs/clap/latest/clap/)
