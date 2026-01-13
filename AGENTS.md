# AGENTS.md - OMG Package Manager

> **The Fastest Unified Package Manager for Arch Linux + All Language Runtimes**

Guidelines for AI coding agents working on this Rust codebase.

## Build Commands

```bash
cargo build                    # Development build
cargo build --release          # Release build (optimized with LTO)
cargo check                    # Check without building (fast feedback)
cargo run -- <command>         # Run the CLI (e.g., cargo run -- search firefox)
cargo run --bin omgd           # Run the daemon
```

## Testing Commands

```bash
cargo test                              # Run all tests
cargo test test_database_open           # Run a single test by name
cargo test core::database::tests        # Run tests in a specific module
cargo test -- --nocapture               # Run tests with output shown
cargo test search                       # Run tests matching a pattern
cargo test --lib                        # Run only unit tests (no integration tests)
```

## Linting and Formatting

```bash
cargo fmt                              # Format code
cargo fmt -- --check                   # Check formatting without changes
cargo clippy                           # Run clippy linter
cargo clippy -- -W clippy::pedantic    # Clippy with pedantic warnings (project goal)
```

## Project Structure

```
src/
  bin/omg.rs, omgd.rs          # CLI and daemon binaries
  lib.rs                        # Library root
  cli/args.rs, commands.rs      # CLI argument parsing and command implementations
  core/types.rs, error.rs       # Shared types and error handling
  core/database.rs, client.rs   # LMDB wrapper and daemon IPC client
  daemon/server.rs, cache.rs    # Unix socket server and LRU cache
  package_managers/traits.rs    # PackageManager trait
  package_managers/arch.rs      # Arch pacman implementation
  package_managers/aur.rs       # AUR client
  runtimes/node.rs, python.rs, go.rs, rust.rs, ruby.rs, java.rs, bun.rs
  config/settings.rs            # Configuration with serde
```

## Code Style Guidelines

### Imports
Order imports: 1) std::, 2) external crates, 3) crate::/super::

```rust
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::{Package, PackageSource};
```

### Error Handling
- Use `anyhow::Result` for application-level functions
- Use `thiserror` for custom error types in library APIs
- Add context with `.context()` or `.with_context()`

```rust
#[derive(Error, Debug)]
pub enum OmgError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### Async Patterns
- Use `tokio` with `#[tokio::main]`
- Use `async_trait` for async trait methods
- Use `tokio::join!` for parallel async operations

```rust
#[async_trait]
pub trait PackageManager: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<Package>>;
}
```

### Naming Conventions
- Types: `PascalCase` (e.g., `PackageManager`, `NodeVersion`)
- Functions/methods: `snake_case` (e.g., `list_installed`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `NODE_DIST_URL`)
- Enum variants: `PascalCase` (e.g., `Runtime::Node`)

### Documentation
- Module-level: `//!`
- Public items: `///`

### Structs and Types
- Derive: `Debug`, `Clone`, `Serialize`, `Deserialize`
- Use `#[serde(default)]` for optional config fields
- Implement `Default` for configuration structs

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub shims_enabled: bool,
    pub data_dir: PathBuf,
}
```

### Performance Patterns
- Use `parking_lot::RwLock` over `std::sync::RwLock`
- Use `DashMap` for concurrent hash maps
- Prefer direct libalpm over spawning pacman subprocess
- Target <1ms query latency for cached operations

### CLI with Clap
```rust
#[derive(Parser, Debug)]
#[command(name = "omg")]
pub struct Cli {
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(visible_alias = "s")]
    Search { query: String },
}
```

### Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_open() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("test.db"));
        assert!(db.is_ok());
    }
}
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive |
| `tokio` | Async runtime |
| `anyhow` / `thiserror` | Error handling |
| `serde` / `toml` | Configuration serialization |
| `heed` | LMDB database bindings |
| `alpm` | Direct libalpm bindings (10-100x faster) |
| `reqwest` | HTTP client for downloads |
| `dashmap` / `parking_lot` | Concurrent data structures |
| `tracing` | Structured logging |
| `colored` / `indicatif` | Terminal output |

## Architecture Notes

- **Daemon (`omgd`)**: Persistent process with Unix socket IPC, in-memory cache
- **CLI (`omg`)**: Thin client connecting to daemon or falling back to direct calls
- **LMDB**: 4GB mmap for package metadata caching
- **Runtimes**: Pure Rust implementations (no subprocess for version switching)
- **Security**: PGP verification (sequoia-openpgp), SLSA (sigstore)

## Performance Targets

- Version switch: <2ms (vs 100-200ms for nvm/pyenv)
- Package search: <10ms (direct ALPM)
- Shell startup overhead: <10ms
- Daemon cache hit: <1ms
