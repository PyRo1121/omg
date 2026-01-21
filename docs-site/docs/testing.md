---
title: Testing Guide
sidebar_position: 60
description: How to run tests and contribute
---

# OMG Testing Guide

OMG maintains a world-class testing system with segmented test categories for pre-push verification.

**Test Coverage: 200+ tests across 10 segments**

## Quick Start

```bash
# Run all tests (recommended before pushing)
./scripts/test-all.sh

# Quick tests only (skip integration/comprehensive)
./scripts/test-all.sh --quick

# Run specific segment
./scripts/test-all.sh --segment core
./scripts/test-all.sh --segment runtimes
./scripts/test-all.sh --segment comprehensive

# Verbose output
./scripts/test-all.sh --verbose
```

## Test Segments (10 Total)

| Segment | Description | Tests | Command |
|---------|-------------|-------|---------|
| **lint** | Format check + Clippy | 2 | `--segment lint` |
| **build** | Compilation check | 1 | `--segment build` |
| **core** | Core module unit tests | 30+ | `--segment core` |
| **runtimes** | Runtime manager tests | 25+ | `--segment runtimes` |
| **cli** | CLI argument parsing | 5 | `--segment cli` |
| **packages** | Package manager tests | 5 | `--segment packages` |
| **security** | Input validation tests | 50+ | `--segment security` |
| **property** | Property-based fuzzing | 30+ | `--segment property` |
| **comprehensive** | All CLI commands | 100+ | `--segment comprehensive` |
| **integration** | Full integration tests | 50+ | `--segment integration` |

## Test Categories

### 1. Lint (Segment: `lint`)
- **Format Check**: `cargo fmt -- --check`
- **Clippy**: `cargo clippy -- -D warnings`

### 2. Core Module Tests (Segment: `core`)
- Database operations (`core::database`)
- Completion engine (`core::completion`)
- Container detection (`core::container`)
- Security/secrets (`core::security`)
- System info (`core::sysinfo`)
- Error handling (`core::error`)

### 3. Runtime Manager Tests (Segment: `runtimes`)
- Common utilities (`runtimes::common`)
- Node.js manager (`runtimes::node`)
- Python manager (`runtimes::python`)
- Go manager (`runtimes::go`)
- Bun manager (`runtimes::bun`)
- Ruby manager (`runtimes::ruby`)
- Java manager (`runtimes::java`)
- Rust manager (`runtimes::rust`)
- Mise integration (`runtimes::mise`)

### 4. CLI Tests (Segment: `cli`)
- Argument parsing (`cli::args`)
- Hook detection (`hooks`)

### 5. Package Manager Tests (Segment: `packages`)
- Pacman DB queries (`package_managers::pacman_db`)
- Parallel sync (`package_managers::parallel_sync`)

### 6. Security Tests (Segment: `security`)
- Input validation (command injection, path traversal)
- Privilege tests (no unnecessary root)
- Filesystem security (temp file handling)

### 7. Property-Based Tests (Segment: `property`)
- Fuzz testing with `proptest`
- Random input generation

### 8. Integration Tests (Segment: `integration`)
- End-to-end Arch Linux tests
- Full command execution

## Running Individual Tests

```bash
# Run a specific test module
rustup run nightly cargo test --features arch --lib core::error

# Run a specific test function
rustup run nightly cargo test --features arch test_version_cmp

# Run with output shown
rustup run nightly cargo test --features arch -- --nocapture

# Run integration tests
rustup run nightly cargo test --features arch --test security_tests
```

## Git Hooks

Install pre-push hooks to automatically run tests:

```bash
./scripts/install-hooks.sh
```

This installs:
- **pre-commit**: Format check on staged `.rs` files
- **pre-push**: Quick test suite before pushing

Skip hooks temporarily:
```bash
git push --no-verify
```

## Test Coverage

Current test count: **76+ unit tests**

| Module | Tests |
|--------|-------|
| Core (database, completion, container, security, error) | 15 |
| Runtimes (common, node, python, go, bun, ruby, java, rust) | 25 |
| CLI (args, hooks) | 8 |
| Package managers | 4 |
| Security tests | 12 |
| Property tests | 12+ |

## Writing New Tests

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_feature_works() {
        let temp = TempDir::new().unwrap();
        // Test implementation
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_case() {
        let result = function_that_fails();
        assert!(result.is_err());
    }
}
```

### Async Test Template

```rust
#[tokio::test]
async fn test_async_feature() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

## CI/CD Integration

The test script returns exit codes:
- `0` - All tests passed
- `1` - Test failure

Example GitHub Actions:

```yaml
- name: Run tests
  run: ./scripts/test-all.sh --quick
```

## Troubleshooting

### Tests fail with "permission denied"
Run with appropriate permissions or use `sudo` for integration tests.

### Clippy warnings
Run `cargo clippy --fix` to auto-fix some issues.

### Format errors
Run `cargo fmt` to fix formatting.

### Nightly required
The project requires nightly Rust. Use `rustup run nightly cargo ...` or ensure nightly is active.
