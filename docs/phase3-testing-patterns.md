# Phase 3: Testing Patterns and Guidelines

## Overview

This document outlines the testing patterns, fixtures, and best practices used in the OMG project. It serves as a guide for maintaining consistency and quality across our test suite.

## Test Organization

### Test Structure

Tests are organized into several categories:

1. **Unit Tests** (`#[cfg(test)] mod tests`)
   - Located in source files alongside implementation
   - Test individual functions and methods in isolation
   - Fast, deterministic, no I/O

2. **Integration Tests** (`tests/` directory)
   - Test complete workflows and CLI commands
   - May involve file system, process execution
   - Located in separate test files

3. **Property-Based Tests** (`tests/property_tests.rs`)
   - Uses proptest framework
   - Tests properties that should hold for any input
   - Discovers edge cases automatically

4. **Test Fixtures** (`tests/common/` and `src/core/testing/`)
   - Reusable test data builders
   - Common test scenarios
   - Shared test infrastructure

### Test File Locations

```
src/
  core/
    testing/
      fixtures.rs        # Core domain fixtures (Package, Update, SecurityPolicy)
  */mod.rs               # Unit tests in #[cfg(test)] modules

tests/
  common/
    mod.rs               # Test infrastructure
    fixtures.rs          # Integration test fixtures
    assertions.rs        # Custom assertions
    mocks.rs             # Mock implementations
    runners.rs           # Test runners

  cli_integration.rs     # CLI command tests
  property_tests.rs      # Property-based tests
  property_tests_v2.rs   # Additional property tests
  arch_tests.rs          # Arch Linux specific tests
  debian_*.rs            # Debian specific tests
  benchmarks.rs          # Performance benchmarks
```

## Fixture Usage Patterns

### Core Domain Fixtures

Located in `src/core/testing/fixtures.rs`:

#### PackageFixture

Builder pattern for creating test packages:

```rust
use omg::core::testing::fixtures::PackageFixture;

// Basic usage
let pkg = PackageFixture::new()
    .name("firefox")
    .version("122.0-1")
    .description("Web browser")
    .installed(true)
    .build();

// Predefined packages
let firefox = PackageFixture::firefox().build();
let git = PackageFixture::git().build();
let pacman = PackageFixture::pacman().build();

// Collections
let installed = PackageFixture::installed_packages();
let available = PackageFixture::available_packages();
let search_results = PackageFixture::search_results();
```

#### UpdateFixture

Builder for update scenarios:

```rust
use omg::core::testing::fixtures::UpdateFixture;

// Build custom updates
let updates = UpdateFixture::new()
    .add_major("kernel")
    .add_minor("git")
    .add_patch("firefox")
    .build();

// Predefined scenarios
let updates = UpdateFixture::typical_system();
```

#### SecurityPolicyFixture

Builder for security policies:

```rust
use omg::core::testing::fixtures::SecurityPolicyFixture;

let policy = SecurityPolicyFixture::new()
    .strict()
    .build();

let policy = SecurityPolicyFixture::new()
    .permissive()
    .build();
```

### Integration Test Fixtures

Located in `tests/common/fixtures.rs`:

#### Static Test Data

```rust
use crate::common::fixtures;

// Package names
fixtures::packages::UNIVERSAL      // Cross-distro packages
fixtures::packages::ARCH_ONLY      // Arch-specific
fixtures::packages::DEBIAN_ONLY    // Debian-specific
fixtures::packages::NONEXISTENT    // Known non-existent
fixtures::packages::POPULAR        // Popular packages
fixtures::packages::DEV_TOOLS      // Development tools

// Runtime versions
fixtures::runtimes::NODE_VERSIONS
fixtures::runtimes::PYTHON_VERSIONS
fixtures::runtimes::GO_VERSIONS

// Version file content
fixtures::version_files::NVMRC_SIMPLE
fixtures::version_files::TOOL_VERSIONS_MULTI
fixtures::version_files::MISE_TOML_SIMPLE

// Security policies
fixtures::policies::STRICT_POLICY
fixtures::policies::ENTERPRISE_POLICY

// Lock files
fixtures::locks::VALID_LOCK
fixtures::locks::INVALID_LOCK_TOML

// Input validation
fixtures::validation::INJECTION_ATTEMPTS
fixtures::validation::UNICODE_INPUTS
fixtures::validation::EMPTY_INPUTS
```

#### TestProject Builder

Managed temporary test environment:

```rust
use crate::common::TestProject;

// Basic usage
let project = TestProject::new();
project.create_file("package.json", "{}");
let result = project.run(&["status"]);

// With predefined project types
let project = TestProject::new()
    .with_node_project()
    .with_tool_versions(&[("node", "20.0.0")]);

let project = TestProject::new()
    .with_python_project();

let project = TestProject::new()
    .with_rust_project();
```

## Property-Based Testing Strategy

### When to Use Property Testing

Use property-based tests for:

1. **Input Validation**: Test that invalid inputs are handled gracefully
2. **Parsing Logic**: Version parsing, config parsing, etc.
3. **Security**: Ensure no command injection, path traversal, etc.
4. **Invariants**: Properties that must hold for all inputs
5. **Performance**: Ensure operations stay within time bounds
6. **Concurrency**: Test thread-safety properties

### Property Test Categories

#### 1. Security Properties

```rust
proptest! {
    #[test]
    fn prop_no_command_injection(input in ".*") {
        let result = run_omg(&["search", &input]);
        // Should never execute injected commands
        prop_assert!(!result.stdout.contains("root:"));
        prop_assert!(!result.stderr.contains("panicked"));
    }
}
```

#### 2. Parsing Properties

```rust
proptest! {
    #[test]
    fn prop_version_parsing_never_panics(
        major in 0u32..1000,
        minor in 0u32..1000,
        patch in 0u32..1000
    ) {
        let version = format!("{major}.{minor}.{patch}");
        let result = parse_version(&version);
        prop_assert!(result.is_ok() || result.is_err());
        // Never panics
    }
}
```

#### 3. Idempotence Properties

```rust
proptest! {
    #[test]
    fn prop_operation_idempotent(input in ".*") {
        let result1 = operation(&input);
        let result2 = operation(&input);
        prop_assert_eq!(result1, result2);
    }
}
```

#### 4. Performance Properties

```rust
proptest! {
    #[test]
    fn prop_operation_bounded(len in 0usize..10000) {
        let input = "a".repeat(len);
        let start = Instant::now();
        operation(&input);
        prop_assert!(start.elapsed() < Duration::from_secs(1));
    }
}
```

### Proptest Configuration

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn my_property(...) {
        // Test with 100 random cases
    }
}
```

Adjust case count based on:
- Fast tests: 100-1000 cases
- Medium tests: 20-50 cases
- Slow tests: 5-10 cases
- Very slow tests: Use fuzz tests with environment flag

## Arrange-Act-Assert (AAA) Pattern

All tests should follow the AAA pattern for clarity and maintainability.

### Pattern Structure

```rust
#[test]
fn test_example() {
    // ===== ARRANGE =====
    // Set up test data and conditions
    let fixture = PackageFixture::firefox();
    let package = fixture.build();
    let expected_name = "firefox";

    // ===== ACT =====
    // Execute the operation being tested
    let result = package.name.clone();

    // ===== ASSERT =====
    // Verify the results
    assert_eq!(result, expected_name);
    assert!(package.installed == false);
}
```

### Good Examples

#### Unit Test

```rust
#[test]
fn test_package_fixture_builder() {
    // Arrange
    let name = "test-pkg";
    let version = "1.0.0";

    // Act
    let pkg = PackageFixture::new()
        .name(name)
        .version(version)
        .build();

    // Assert
    assert_eq!(pkg.name, name);
    assert_eq!(pkg.version.to_string(), version);
}
```

#### Integration Test

```rust
#[test]
fn test_search_command() {
    // Arrange
    let query = "firefox";
    let project = TestProject::new();

    // Act
    let result = project.run(&["search", query]);

    // Assert
    result.assert_success();
    result.assert_stdout_contains(query);
}
```

#### Property Test

```rust
proptest! {
    #[test]
    fn prop_version_comparison(
        major1 in 0u32..100,
        major2 in 0u32..100
    ) {
        // Arrange
        let v1 = format!("{major1}.0.0");
        let v2 = format!("{major2}.0.0");

        // Act
        let result = compare_versions(&v1, &v2);

        // Assert
        if major1 > major2 {
            prop_assert!(result > 0);
        } else if major1 < major2 {
            prop_assert!(result < 0);
        } else {
            prop_assert_eq!(result, 0);
        }
    }
}
```

### Anti-Patterns to Avoid

#### Bad: Mixing Arrange and Act

```rust
// DON'T DO THIS
#[test]
fn bad_test() {
    let pkg = PackageFixture::new().name("test").build(); // arrange + act mixed
    assert_eq!(pkg.name, "test");
}
```

#### Bad: No Clear Sections

```rust
// DON'T DO THIS
#[test]
fn bad_test() {
    assert_eq!(
        PackageFixture::new().name("test").build().name,
        "test"
    );
}
```

#### Good: Clear Sections

```rust
// DO THIS
#[test]
fn good_test() {
    // Arrange
    let expected = "test";
    let fixture = PackageFixture::new().name(expected);

    // Act
    let pkg = fixture.build();

    // Assert
    assert_eq!(pkg.name, expected);
}
```

## Test Assertions

### Standard Assertions

```rust
assert!(condition);
assert_eq!(left, right);
assert_ne!(left, right);
assert!(result.is_ok());
assert!(result.is_err());
```

### Custom Assertions (tests/common/assertions.rs)

```rust
use crate::common::assertions::*;

// Command result assertions
result.assert_success();
result.assert_failure();
result.assert_stdout_contains("text");
result.assert_stderr_contains("error");
result.assert_duration_under(Duration::from_secs(1));

// Property test assertions
prop_assert!(condition);
prop_assert_eq!(left, right);
prop_assert_ne!(left, right);
```

## Test Configuration

### Environment Variables

Tests respect these environment variables:

- `OMG_TEST_MODE=1` - Enable test mode
- `OMG_RUN_SYSTEM_TESTS=1` - Enable system integration tests
- `OMG_RUN_NETWORK_TESTS=1` - Enable network tests
- `OMG_RUN_DESTRUCTIVE_TESTS=1` - Enable destructive tests
- `OMG_RUN_FUZZ_TESTS=1` - Enable fuzz testing
- `OMG_TEST_DISTRO=arch|debian|ubuntu` - Override distro detection
- `NO_COLOR=1` - Disable color output

### Conditional Test Execution

```rust
use crate::common::{TestConfig, require_system_tests};

#[test]
fn test_requiring_system_access() {
    require_system_tests!();

    // Test code that requires system access
}

#[test]
fn test_with_config() {
    let config = TestConfig::default();
    if config.skip_if_no_network("test_name") {
        return;
    }

    // Network test code
}
```

### Test Isolation

Tests are isolated using:

1. **Temporary directories**: Each test gets unique temp dirs for data/config
2. **Environment isolation**: Tests set isolated env vars via TestProject
3. **Serial execution**: Use `#[serial]` for tests that can't run concurrently

```rust
use serial_test::serial;

#[test]
#[serial]  // Runs serially, not in parallel
fn test_modifying_global_state() {
    // Test that modifies global state
}
```

## Best Practices

### 1. Use Fixtures Over Manual Setup

```rust
// DON'T
#[test]
fn manual_setup() {
    let pkg = Package {
        name: "firefox".to_string(),
        version: parse_version_or_zero("122.0-1"),
        description: "Browser".to_string(),
        source: PackageSource::Official,
        installed: false,
    };
}

// DO
#[test]
fn with_fixture() {
    let pkg = PackageFixture::firefox().build();
}
```

### 2. Test One Thing Per Test

```rust
// DON'T
#[test]
fn test_everything() {
    // Tests parsing AND validation AND formatting
}

// DO
#[test]
fn test_version_parsing() {
    // Only tests parsing
}

#[test]
fn test_version_validation() {
    // Only tests validation
}
```

### 3. Use Descriptive Names

```rust
// DON'T
#[test]
fn test1() { }

#[test]
fn test_pkg() { }

// DO
#[test]
fn test_package_fixture_builder_sets_name() { }

#[test]
fn test_version_comparison_major_takes_precedence() { }
```

### 4. Test Error Cases

```rust
#[test]
fn test_invalid_version_returns_error() {
    // Arrange
    let invalid = "not-a-version";

    // Act
    let result = parse_version(invalid);

    // Assert
    assert!(result.is_err());
}
```

### 5. Use Property Tests for Security

All user input should have property tests ensuring:
- No command injection
- No path traversal
- No buffer overflows
- Graceful handling of malformed data

### 6. Keep Tests Fast

- Unit tests: < 1ms
- Integration tests: < 100ms
- Slow tests: Use `#[ignore]` or environment flags

### 7. Make Tests Deterministic

Avoid:
- Random values (unless using proptest)
- System time (use fixed timestamps in tests)
- Network calls (mock or use environment flags)
- File system state (use temporary directories)

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Test File

```bash
cargo test --test cli_integration
```

### Specific Test

```bash
cargo test test_search_command
```

### With Features

```bash
cargo test --features arch
```

### Property Tests with More Cases

```bash
PROPTEST_CASES=1000 cargo test property_tests
```

### Fuzz Tests

```bash
OMG_RUN_FUZZ_TESTS=1 cargo test fuzz
```

### System Tests

```bash
OMG_RUN_SYSTEM_TESTS=1 cargo test
```

## Maintenance Guidelines

### Adding New Fixtures

1. Add to appropriate fixture file (core vs integration)
2. Follow builder pattern for complex fixtures
3. Provide both builders and predefined instances
4. Document usage in this file

### Adding New Property Tests

1. Identify the property to test
2. Choose appropriate input strategy
3. Set reasonable case count
4. Document the property being tested

### Updating Test Patterns

1. Update this document first
2. Add examples to test files
3. Update existing tests to follow patterns
4. Review in code review

## Summary

- **Fixtures**: Use builders for consistent test data
- **Property Tests**: Test invariants and edge cases automatically
- **AAA Pattern**: Keep tests readable with clear sections
- **Isolation**: Each test runs in its own environment
- **Fast**: Unit tests fast, integration tests reasonable, slow tests flagged
- **Deterministic**: Tests produce same results every time
- **Maintainable**: Clear names, one thing per test, good documentation
