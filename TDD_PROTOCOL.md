# OMG Test-Driven Development (TDD) Protocol

This document defines the mandatory TDD process for the OMG project. To ensure "absolute everything" is tested, every new feature, bug fix, or refactor must follow this sequence.

## 1. The Red-Green-Refactor Cycle

### Phase 1: Red (Write a Failing Test)
- Before writing a single line of implementation code, write a unit test or integration test that describes the desired behavior.
- Run the test and ensure it **fails** (typically with a compilation error or assertion failure).
- If it passes, the test is either redundant or you're testing the wrong thing.

### Phase 2: Green (Write Minimal Code)
- Write the **minimum** amount of code required to make the test pass.
- Do not optimize or add "just in case" features yet.
- Ensure all tests (including the new one) pass.

### Phase 3: Refactor (Clean Up)
- Clean up the code while keeping the tests green.
- Remove duplication, improve naming, and optimize performance.
- Ensure 100% memory safety (no `unsafe`).

## 2. Testing "Absolute Everything"

### Unit Tests (Internal Logic)
- Every non-trivial private function must have a unit test in its respective module.
- Use `#[cfg(test)] mod tests { ... }` at the bottom of each file.
- Test edge cases: empty strings, max values, null inputs, malformed paths.

### Property-Based Tests (Parser & Logic)
- Use `proptest` for any logic that involves parsing (versions, TOML, database indices).
- Define invariants: "Parsing then serializing should return the original data."

### Integration Tests (CLI & IPC)
- Test the full command-line experience in `tests/integration_suite.rs`.
- Test the daemon IPC protocol in `tests/daemon_security_tests.rs`.

### Negative Testing (Error Handling)
- You MUST test failure modes. What happens if `/var/lib/pacman` is read-only? What if the network is down?
- Every `Result` return path must be exercised by at least one test.

## 3. Mandatory Requirements
1. **No Unsafe**: All tests must verify safe behavior.
2. **Performance Regressions**: Any performance-critical change must be accompanied by a benchmark in `benches/`.
3. **Coverage**: New code must maintain or increase the project's test coverage.

## 4. TDD Workflow Command
```bash
# Watch for changes and run tests continuously
cargo watch -x test
```
