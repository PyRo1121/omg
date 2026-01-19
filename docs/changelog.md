# Changelog

**Version History and Release Notes**

---

## [Unreleased]

### Added
- Interactive TUI dashboard (`omg dash`)
- Transaction history and rollback support
- Real-time CVE monitoring via ALSA integration
- Secret scanning with 20+ credential patterns
- Tamper-proof audit logging with hash-chain verification
- CycloneDX 1.5 SBOM generation
- SLSA provenance verification via Sigstore/Rekor
- Team collaboration features (`omg team` commands)
- Container integration (`omg container` commands)
- Ultra-fast shell functions for prompts (`omg-ec`, `omg-tc`, etc.)
- Comprehensive documentation rewrite

### Changed
- Improved shell hook performance
- Enhanced Nucleo-based fuzzy matching
- Updated Sequoia-OpenPGP to PQC-ready version

### Fixed
- Removed unused `rustsec` dependency
- Cleaned up dead code in runtime manager

---

## [0.1.0] - 2026-01-01

### Added

#### Core Features
- Unified package management for Arch Linux (official + AUR)
- Experimental Debian/Ubuntu support via rust-apt
- 7 native runtime managers: Node.js, Python, Go, Rust, Ruby, Java, Bun
- Built-in mise integration for 100+ additional runtimes
- Shell hooks for Zsh, Bash, and Fish
- Shell completions with fuzzy matching (Nucleo)

#### Daemon
- Persistent background daemon (`omgd`)
- In-memory package index with Nucleo
- moka LRU cache for search results
- redb persistence across restarts
- Binary status file for ultra-fast shell queries
- Unix socket IPC with bincode serialization

#### Performance
- 22x faster searches than pacman (6ms vs 133ms)
- Sub-millisecond explicit package listing
- Binary protocol for minimal latency
- Zero subprocess overhead

#### Security
- PGP signature verification (Sequoia-OpenPGP)
- Security grading (LOCKED, VERIFIED, COMMUNITY, RISK)
- Policy enforcement via policy.toml
- Vulnerability scanning (ALSA + OSV.dev)

#### Developer Experience
- Unified task runner (`omg run`)
- Project scaffolding (`omg new`)
- Tool management (`omg tool`)
- Environment lockfiles (`omg env`)

#### Configuration
- XDG-compliant configuration paths
- TOML configuration files
- Customizable AUR build settings
- Runtime backend selection (native/mise)

### Technical Details

#### Dependencies
- **alpm** — libalpm bindings for pacman
- **tokio** — Async runtime
- **clap** — CLI argument parsing
- **serde/bincode** — Serialization
- **reqwest** — HTTP client
- **moka** — Concurrent cache
- **redb** — Embedded database
- **sequoia-openpgp** — PGP verification
- **ratatui** — TUI framework
- **nucleo** — Fuzzy matching

#### Supported Platforms
- Arch Linux (full support)
- Manjaro, EndeavourOS (full support)
- Debian/Ubuntu 22.04+ (experimental)

---

## Versioning

OMG follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API/CLI changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

---

## Upgrade Guide

### From Pre-Release to 0.1.0

```bash
# Pull latest
git pull origin main

# Rebuild
cargo build --release

# Update binaries
cp target/release/{omg,omgd,omg-fast} ~/.local/bin/

# Restart daemon
pkill omgd
omg daemon

# Clear old cache
rm ~/.local/share/omg/cache.redb
```

---

## Deprecations

None at this time.

---

## Future Releases

### Planned for 0.2.0
- Fedora/RHEL support (DNF integration)
- macOS support (Homebrew integration)
- Windows support (Chocolatey/Winget)
- GUI dashboard application
- Enhanced team sync features

### Planned for 1.0.0
- Stable API guarantee
- Long-term support (LTS)
- Enterprise features

---

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution guidelines.

Report issues at [GitHub Issues](https://github.com/PyRo1121/omg/issues).
