# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OMG is a unified package manager and runtime manager written in Rust (Edition 2024, MSRV 1.92+). It replaces multiple package managers (pacman, apt, dnf, homebrew, scoop) and runtime managers (nvm, pyenv, etc.) with a single CLI tool.

**Architecture:** Daemon-client model with Unix socket IPC. The `omgd` daemon handles caching, indexing, and persistence while `omg` CLI communicates via bitcode-serialized messages.

## Build Commands

```bash
# Development
make build              # cargo build --features arch
make test               # cargo test --features arch
make test-lib           # Unit tests only (fast)
make check              # cargo check --features arch

# Quality
make fmt                # cargo fmt
make clippy             # cargo clippy --features arch -- -D warnings
make clippy-strict      # With pedantic+nursery lints
make qa                 # fmt-check + clippy-strict + test-lib

# Release
make release            # Optimized build with LTO
make install            # Install to ~/.local/bin

# Single test
cargo test --features arch test_name
cargo test --features arch module::tests -- --nocapture
```

**Feature flags:** `arch` (Arch/libalpm), `debian` (APT), `fedora` (DNF), `macos` (Homebrew), `windows` (Scoop), `pgp` (signature verification)

## Code Architecture

```
src/
├── bin/
│   ├── omg.rs           # CLI entry point
│   ├── omgd.rs          # Daemon entry point
│   └── omg-fast.rs      # Shell prompt optimizer
├── cli/
│   ├── args.rs          # Clap argument parsing
│   ├── commands.rs      # Command dispatch
│   ├── packages/        # search, install, info, remove, update
│   ├── runtimes.rs      # use, list, which commands
│   └── tea/             # TUI components (Elm-style)
├── core/
│   ├── types.rs         # Package, Runtime types
│   ├── error.rs         # Error handling
│   ├── database.rs      # redb wrapper
│   ├── client.rs        # Daemon IPC client
│   └── security/        # PGP, SBOM, audit
├── daemon/
│   ├── server.rs        # Unix socket server
│   ├── cache.rs         # moka LRU cache
│   └── index.rs         # Nucleo fuzzy indexing
├── package_managers/
│   ├── traits.rs        # PackageManager trait
│   ├── arch.rs          # libalpm bindings
│   ├── alpm_ops.rs      # Direct ALPM operations
│   ├── aur/             # AUR HTTP client
│   └── debian_db/       # APT backend
└── runtimes/            # Node, Python, Go, Rust, Ruby, Java, Bun
```

**Key patterns:**
- `PackageManager` trait for cross-platform backends
- All I/O uses tokio async runtime
- Multi-tier caching: moka (memory) → redb (disk) → binary status (shell prompt)
- IPC via Unix sockets with bitcode serialization

## Code Style Requirements

**Imports order:** `std::` → external crates → `crate::`/`super::`

**Error handling:**
- Application code: `anyhow::Result` with `.context()` for error chains
- Library APIs: `thiserror` for custom error types
- No `.unwrap()` in production code; use `.expect()` with context

**Performance patterns:**
- Use `Arc` over `Clone` for large types in async contexts
- Use `Cow<str>` for conditional ownership
- `#[inline]` on hot-path functions

**Clippy rules enforced:**
- `clippy::pedantic` and `clippy::nursery` as warnings
- `clippy::correctness` level set to deny
- `dbg_macro = "deny"` (never ship debug code)

## Testing

Tests are co-located with modules using `#[cfg(test)]`. Integration tests in `tests/` directory.

```bash
cargo test --features arch                    # All tests
cargo test --features arch --lib              # Unit only
cargo watch -x test                           # TDD mode
cargo tarpaulin --ignore-tests --out html     # Coverage
```

## Performance Targets

- Search: < 10ms
- Info: < 10ms
- Status: < 10ms
- Daemon startup: < 100ms
- Memory: < 50MB resident

Profile with `cargo flamegraph` before optimizing.

## Platform-Specific Notes

- **Arch Linux:** Primary target, uses direct libalpm FFI bindings
- **Debian/Ubuntu:** Uses rust-apt native FFI
- **Fedora/RHEL:** Pure Rust DNF/RPM implementation
- Feature flags control which backends are compiled

## Agent Ecosystem

Custom agents in `.claude/agents/` are specialized for OMG development:

| Agent | Model | Use For |
|-------|-------|---------|
| `Rust-Engineer` | sonnet | Core Rust: ownership, traits, async, FFI, perf |
| `cli-developer` | sonnet | CLI UX: clap, TUI, output, error messages |
| `test-runner` | haiku | Run tests, diagnose failures, report results |
| `security-auditor` | sonnet | Unsafe code, privilege escalation, CVEs |
| `perf-profiler` | sonnet | Benchmarks, flamegraphs, optimization |
| `code-reviewer` | sonnet | Review changes, style compliance, quality gates |
| `swarm-lead` | opus | Orchestrate parallel multi-agent tasks |

**Swarm mode** is enabled for parallel agent teams. Use for:
- Refactoring multiple backends simultaneously
- Running comprehensive test suites in parallel
- Large features spanning many files

**Hooks:** `cargo fmt` runs automatically after Rust file edits.

## Privilege Escalation Pattern

OMG defers sudo until absolutely necessary:
1. `can_write_pacman_db()` checks Linux capabilities first
2. Read-only operations (search, info, update --check) never need root
3. `run_privileged_operation()` handles elevation with `fullupdate` combining sync+upgrade
4. `--dry-run` and `--check` modes skip sync entirely
