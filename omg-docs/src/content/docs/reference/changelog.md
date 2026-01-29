---
title: ""
description: ""
---

updated-dependencies:

  - dependency-name: toml

dependency-version: 0.9.11+spec-1.1.0

dependency-type: direct:production

update-type: version-update:semver-minor

dependency-group: dependencies

  - dependency-name: zip

dependency-version: 7.1.0

dependency-type: direct:production

update-type: version-update:semver-major

dependency-group: dependencies

  - dependency-name: dashmap

dependency-version: 6.1.0

dependency-type: direct:production

update-type: version-update:semver-major

dependency-group: dependencies

  - dependency-name: criterion

dependency-version: 0.8.1

dependency-type: direct:production

update-type: version-update:semver-minor

dependency-group: dependencies

  - dependency-name: cargo-audit

dependency-version: 0.22.0

dependency-type: direct:production

update-type: version-update:semver-minor

dependency-group: dependencies

  - dependency-name: rand

dependency-version: 0.9.2

dependency-type: direct:production

update-type: version-update:semver-minor

dependency-group: dependencies

...

- Bump version to 0.1.73
### 🧪 Testing

- Add comprehensive unit tests for task detection and resolution

Add unit tests covering ecosystem priority, config loading, priority-based resolution, --using flag override, --all flag behavior, and .omg.toml config overrides. Tests use tempfile for isolated filesystem operations and verify correct task detection across multiple ecosystems.

- Fix regressions in Debian IPC and cache tests
## [0.1.72] - 2026-01-18
### 🐛 Bug Fixes

- Install.sh now uses GitHub releases, fix Enterprise pricing display
## [0.1.71] - 2026-01-18
## [0.1.70] - 2026-01-18
### ⚡ Performance

- Update performance claims from 200x to 22x faster than pacman across site metadata and benchmarks

- Update page title and meta descriptions from "200x Faster" to "22x Faster"
- Revise OpenGraph and Twitter card descriptions to focus on pacman comparison
- Update JSON-LD structured data with accurate performance claims
- Replace "200x faster than yay/paru" with "6ms average query time" in feature list
- Update FAQ responses to remove yay comparisons and cite 22x vs pacman
- Revise benchmark tables
## [0.1.62] - 2026-01-18
## [0.1.61] - 2026-01-18
## [0.1.60] - 2026-01-17
### 🔒 Security

- Add license management system and make PGP verification optional

- Add base64 dependency and downgrade dashmap to 5.5 for compatibility
- Make sequoia-openpgp optional behind pgp feature flag (requires Rust 1.80+)
- Add license subcommand with activate/status/deactivate/check operations
- Add license module with feature gating for audit/sbom/team-sync
- Require Pro tier for vulnerability scanning and SBOM generation
- Require Team tier for audit logs and team sync features
- Update install.sh to try
## [0.1.55] - 2026-01-17
### ⚡ Performance

- Add Debian/Ubuntu performance optimization dependencies and feature-gate Arch-specific code

- Add debian-packaging, rkyv, winnow, gzp, and ar crates to Cargo.toml for pure Rust apt reimplementation with zero-copy deserialization and parallel decompression
- Wrap all Arch-specific code (AUR, ALPM, pacman) in #[cfg(feature = "arch")] guards
- Add #[cfg(not(feature = "arch"))] fallbacks with appropriate error messages
- Change use_debian_backend() from const fn to regular fn for runtime detection
## [0.1.53] - 2026-01-16
## [0.1.50] - 2026-01-16
## [0.1.48] - 2026-01-16
## [0.1.46] - 2026-01-16
## [0.1.44] - 2026-01-16
## [0.1.39] - 2026-01-16
## [0.1.38] - 2026-01-16
## [0.1.36] - 2026-01-16
### ⚡ Performance

- Update documentation with performance benchmarks and runtime improvements

- Add detailed performance benchmarks comparing OMG to pacman/yay (4-56x faster)
- Document annual time savings calculations for individuals and teams
- Add pure Rust storage (redb) and archive handling to feature list
- Document Rust toolchain support with rust-toolchain.toml integration
- List all supported task runner sources (package.json, Cargo.toml, etc.)
- Add fallback behavior for unknown task names
- Document automatic
## [0.1.28] - 2026-01-16
### ⚡ Performance

- Replace colored with owo_colors and switch to nucleo fuzzy matching

- Replace colored crate with owo_colors throughout codebase
- Switch from fuzzy_matcher to nucleo_matcher for 10x faster fuzzy matching
- Replace chrono DateTime operations with jiff Timestamp and strftime
- Update bincode serialization to use new v2 API with legacy config
- Change AUR build defaults: Native method, allow unsafe builds, use metadata archive
- Optimize AUR package updates with parallel PKGBUILD fetching and bulk
## [0.1.18] - 2026-01-15
## [0.1.17] - 2026-01-15
## [0.1.16] - 2026-01-15
### ⚡ Performance

- Add conditional test execution based on environment flags for system, network, and performance tests

Add environment variable checks (OMG_RUN_SYSTEM_TESTS, OMG_RUN_NETWORK_TESTS, OMG_RUN_PERF_TESTS) to skip tests requiring external resources. Update integration test suite documentation with new flags. Fix import ordering in client.rs. Rebuild binaries
## [0.1.15] - 2026-01-15
### ⚡ Performance

- Update Rust edition to 2024 and improve code quality with clippy fixes

Update Cargo.toml to use Rust 2024 edition with minimum version 1.88. Fix repository URL. Refactor code to address clippy warnings: use references in function parameters to avoid unnecessary clones, simplify match arms with pattern matching, replace case-sensitive file extension checks with proper extension comparison, convert async functions to sync where tokio runtime not needed, use clone_from instead of assignment for better performance, and remove
## [0.1.14] - 2026-01-15
### ⚡ Performance

- Add negative caching for missing package info and improve AUR metadata handling with HTTP caching

Add negative cache to track missing package info lookups to avoid repeated failed searches. Implement HTTP conditional requests (ETag/Last-Modified) for AUR metadata downloads to reduce bandwidth. Replace regex-based PKGBUILD parsing with faster string scanning. Add clippy allow for struct_excessive_bools in AurBuildSettings. Fix formatting and remove dead code
- Optimizations
## [0.1.11] - 2026-01-15
## [0.1.9] - 2026-01-15
## [0.1.8] - 2026-01-15
## [0.1.7] - 2026-01-15
## [0.1.5] - 2026-01-13
### ✨ New Features

- **Completion**: Implement fuzzy matching, context awareness, and AUR caching
