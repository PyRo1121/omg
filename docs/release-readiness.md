# Release Readiness Checklist

Use this checklist before tagging a release to keep builds reproducible and platform coverage complete.

## 1) Local Quality Gates

```bash
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## 2) Feature-Scoped Validation

Run feature-scoped checks to match CI behavior and avoid false negatives from mutually exclusive platform stacks.

```bash
# Portable baseline
cargo clippy --all-targets --no-default-features --features pgp,license --locked -- -D warnings

# Arch
cargo clippy --all-targets --no-default-features --features arch,license --locked -- -D warnings

# Debian (requires libapt-pkg-dev)
cargo clippy --all-targets --no-default-features --features debian --locked -- -D warnings

# Fedora
cargo clippy --all-targets --no-default-features --features fedora,license --locked -- -D warnings

# macOS
cargo clippy --all-targets --no-default-features --features macos,license --locked -- -D warnings
```

## 3) Platform Build Prerequisites

- Debian/Ubuntu (`--features debian`): `libapt-pkg-dev`, `clang`, `cmake`
- Arch (`--features arch`): `libalpm` toolchain
- Fedora (`--features fedora`): rpm/sqlite development stack
- macOS: Xcode Command Line Tools

## 4) CI Expectations

- Quick gate passes (`fmt`, portable `clippy`, `check`, portable tests)
- Linux matrix passes (Arch, Debian, Fedora)
- Native macOS job passes; WSL is covered by the matching Linux distribution job
- Coverage job completes and uploads merged report

## 5) Ship Criteria

- No new warnings/errors in scoped `clippy` runs
- No regressions in critical tests (`e2e_package_operations`, daemon cache/lifecycle)
- README and CONTRIBUTING reflect current Rust requirement (1.93+)
- Release artifacts build successfully on all target platforms
