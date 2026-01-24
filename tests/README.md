# Production-Ready Tests for OMG

These tests exercise **REAL code paths** with **REAL package managers**.

## Philosophy

1. **NO MOCKS**: All tests use real implementations
2. **Exercise Real Paths**: Database operations, version comparisons, IPC communication
3. **Measure Performance**: Verify <10ms targets are met
4. **Error Handling**: Verify graceful failures with helpful messages

## Running Tests

### Quick Start

```bash
# Run all unit tests (no system modifications)
cargo test --lib

# Run all integration tests
cargo test --test integration_suite

# Run specific test file
cargo test --test version_tests
cargo test --test update_tests
cargo test --test error_tests
cargo test --test benchmarks
```

### Environment Variables

Tests use environment variables to control behavior:

```bash
# Enable tests that require real system access (pacman, ALPM)
export OMG_RUN_SYSTEM_TESTS=1

# Enable tests that actually install/update packages (use with caution!)
export OMG_RUN_DESTRUCTIVE_TESTS=1

# Enable performance assertions (may fail on slow systems)
export OMG_RUN_PERF_TESTS=1
```

### Example: Run System Tests

```bash
OMG_RUN_SYSTEM_TESTS=1 cargo test --test integration_suite --features arch
```

### Example: Run Performance Tests

```bash
OMG_RUN_PERF_TESTS=1 cargo test --test benchmarks --release --features arch
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

### `update_tests.rs`

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
OMG_RUN_SYSTEM_TESTS=1 cargo test --test update_tests --features arch
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

### `benchmarks.rs`

**Purpose**: Verify OMG meets performance targets

**Features**:
- Measures command execution time
- Tests repeatable performance
- Validates cold vs warm start
- Checks memory efficiency

**What it tests**:
- CLI commands complete in target time
- Search operations are fast
- Update check is instant
- Performance is consistent across runs

**Performance Targets**:
- Version/help: <50ms
- Status command: <200ms
- Search: <100ms
- Info command: <100ms
- Update check: <2s

**Running**:
```bash
OMG_RUN_PERF_TESTS=1 cargo test --test benchmarks --release --features arch
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
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy

      - name: Install Arch Linux packages (for ALPM)
        run: |
          sudo apt-get update
          sudo apt-get install -y libalpm-dev

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry

      - name: Run tests
        env:
          OMG_RUN_SYSTEM_TESTS: 1
        run: |
          cargo test --features arch --all
          cargo test --test version_tests --features arch
          cargo test --test update_tests --features arch
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
RUST_LOG=debug cargo test --test update_tests
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
   - Update logic → `update_tests.rs`
   - Error scenarios → `error_tests.rs`
   - Performance → `benchmarks.rs`
   - General integration → `integration_suite.rs`
3. **Use REAL code paths**: No mocks, no stubs
4. **Add helpful assertions**: Verify real behavior, not just "doesn't crash"
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
cargo test --test benchmarks --release --features arch -- --nocapture

# Make your changes

# After changes
cargo test --test benchmarks --release --features arch -- --nocapture

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

### "Skipping perf test"

If tests skip with this message:
```bash
export OMG_RUN_PERF_TESTS=1
```

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
