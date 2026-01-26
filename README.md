# OMG

**Stop switching between 7 package managers.**

![Installs](https://img.shields.io/endpoint?url=https://api.pyro1121.com/api/badge/installs&style=flat-square)
[![Benchmark](https://img.shields.io/badge/search-6ms%20(22x%20faster)-brightgreen?style=flat-square)](benchmark.sh)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange?style=flat-square)](https://www.rust-lang.org)

OMG is the unified dev tool you've been waiting for. **One command** replaces `pacman`, `yay`, `nvm`, `pyenv`, `rustup`, `rbenv`, and `jenv`.

## The Numbers That Matter

| Metric | Value |
|--------|-------|
| **22x faster** | than pacman/yay (6ms vs 133ms searches) |
| **59-483x faster** | than apt-cache/Nala on Debian/Ubuntu |
| **Zero context switching** | system packages + 8 language runtimes in one CLI |
| **Enterprise-secure** | SLSA, PGP, SBOM, audit logs built-in (not bolted on) |
| **Team-synchronized** | pin your exact environment in `omg.lock`, share it, sync instantly |

### Real-World Impact

A 10-person team saves **39 minutes per engineer per year** just on package queries. For 50 people? **$2,350–$2,650** in reclaimed productivity. And that's before factoring in zero context-switching brain tax.

---

## Before & After

```bash
# ❌ Before: 7 tools, 7 syntaxes, 7 config files
pacman -Ss firefox          # system packages
yay -S spotify              # AUR
nvm install 20              # Node.js
nvm use 20
pyenv install 3.12          # Python
pyenv global 3.12
rustup default stable       # Rust
rbenv install 3.2.0         # Ruby

# ✅ After: Just OMG
omg search firefox
omg install spotify
omg use node 20
omg use python 3.12
omg use rust stable
omg use ruby 3.2.0
```

---

## Quick Start

```bash
# Install (one line)
curl -fsSL https://pyro1121.com/install.sh | bash

# Search packages (22x faster than pacman)
omg search vim

# Install anything (system packages or AUR)
omg install visual-studio-code-bin

# Switch runtimes instantly
omg use node 20
omg use python 3.12

# Run project tasks (auto-detects package.json, Cargo.toml, Makefile, etc.)
omg run dev

# Lock your environment for your team
omg env capture
omg env share
```

> **Supports**: Arch Linux, Debian/Ubuntu. RPM-based distros coming soon.

---

## Why OMG?

### 🏎️ Performance
Direct `libalpm`/`rust-apt` integration—no subprocess overhead. Persistent daemon with in-memory index. Your fingers move faster than OMG responds.

### 🛠️ Unified Runtimes
Node.js, Bun, Python, Go, Rust, Ruby, Java—all native. Plus 100+ more via bundled mise. Auto-detects `.nvmrc`, `.python-version`, `rust-toolchain.toml`, `.tool-versions`.

### 🛡️ Enterprise Security
SLSA provenance, PGP verification, CycloneDX SBOM, secret scanning, tamper-proof audit logs. Security grading on every install. Policy enforcement via `policy.toml`.

### 👥 Team Sync
`omg.lock` captures your exact environment. `omg env check` detects drift. `omg env share` syncs your team instantly via GitHub Gist.

### 🏃 Task Runner
`omg run build` auto-detects `package.json`, `Cargo.toml`, `Makefile`, `pyproject.toml`, `deno.json`—runs with the correct runtime version pre-loaded.

### 🐳 Container Integration
`omg container shell` for dev shells, `omg container build` for images, `omg container init` to generate Dockerfiles from detected runtimes.

### 🧠 Intelligent Completions
Fuzzy matching via Nucleo. Type `omg i frfx` → `firefox`. 80k+ AUR packages cached for lag-free completion.

---

## 📊 Benchmarks

OMG achieves ~6ms performance on all core operations through a persistent daemon that maintains an in-memory index of packages.

### Arch Linux (pacman/yay)

**Benchmark Environment:**
- **CPU:** Intel i9-14900K (32 cores, 5.8GHz turbo)
- **RAM:** 31GB
- **Kernel:** Linux 6.18.3-arch1-1
- **Iterations:** 10 (with 2 warmup runs)

| Command | OMG (Daemon) | pacman | yay | Speedup |
|---------|--------------|--------|-----|---------:|
| **search** | **6ms** ✨ | 133ms | 150ms | **22x faster** |
| **info** | **6.5ms** ✨ | 138ms | 300ms | **21x faster** |
| **status** | **7ms** ✨ | N/A | N/A | — |
| **explicit** | **1.2ms** ✨ | 14ms | 27ms | **12x faster** |

> 💡 **Note:** yay benchmarked with `--repo` flag (no AUR network calls) for fair comparison.

### Debian/Ubuntu (apt)

**Benchmark Environment:**
- **OS:** Ubuntu 24.04 (Docker)
- **Iterations:** 5 (with 2 warmup runs)

| Command | OMG (Daemon) | apt-cache | Nala | vs apt | vs Nala |
|---------|--------------|-----------|------|-------:|--------:|
| **search** | **11ms** ✨ | 652ms | 1160ms | **59x** | **105x** |
| **info** | **27ms** ✨ | 462ms | 788ms | **17x** | **29x** |
| **explicit** | **2ms** ✨ | 601ms | 966ms | **300x** | **483x** |

OMG parses `/var/lib/dpkg/status` and APT's Packages files directly, bypassing slow Python/apt-cache overhead. The daemon maintains an in-memory index for instant cached searches.

### Why These Numbers Matter

**Human Perception:**
- < 100ms = feels instant
- 100-500ms = noticeable delay
- > 500ms = clearly slow

OMG operates in the imperceptible range. Your fingers literally move faster than OMG responds.

**Annual Time & Cost Savings:**

*Based on 50 package operations/day (typical active development) and $150K avg. software engineer salary ($72/hr):*

| Metric | vs pacman | vs yay | 10-person team |
|--------|-----------|--------|----------------|
| **Time saved/engineer/year** | 39 min | 44 min | 6.5–7.3 hours |
| **Dollar savings/year** | $47 | $53 | **$470–$530** |

> 💰 For a 50-person engineering org, that's **$2,350–$2,650/year** in reclaimed productivity—and that's just package queries. Factor in the cognitive benefit of instant feedback and the ROI compounds.

**Verification**
Want to reproduce these numbers?
```bash
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/benchmark.sh | bash
```

---

## 🛠️ Architecture

OMG is split into two components:
1.  **`omg`**: A thin, high-performance CLI client.
2.  **`omgd`**: A persistent daemon that maintains an in-memory package index and handles redb persistence.

Communication happens over a high-speed Unix Domain Socket using a custom binary protocol (Length-Delimited framing + Bincode) for zero-latency communication.

---

## 📚 Documentation

**Full documentation**: [pyro1121.com/docs](https://pyro1121.com/docs) | [docs/](docs/index.md)

| Guide | Description |
|-------|-------------|
| [Quick Start](docs/quickstart.md) | Install and first commands |
| [CLI Reference](docs/cli.md) | All commands with examples |
| [Configuration](docs/configuration.md) | Config files and policy |
| [Runtimes](docs/runtimes.md) | Node, Python, Go, Rust, Ruby, Java, Bun |
| [Security](docs/security.md) | SBOM, vulnerability scanning, audit logs |
| [Shell Integration](docs/shell-integration.md) | Hooks and completions |
| [Team Sync](docs/team.md) | Environment locks and drift detection |
| [Changelog](docs/changelog.md) | Release history and version notes |
| [Troubleshooting](docs/troubleshooting.md) | Common issues |

### Shell Setup

```bash
# Add to ~/.zshrc (or ~/.bashrc)
eval "$(omg hook zsh)"
```

### Key Commands

```bash
omg search <query>          # Search packages (22x faster)
omg install <pkg>           # Install with security grading
omg use node 20             # Switch runtime version
omg run build               # Run project tasks
omg env capture             # Lock environment
omg audit                   # Security scan
omg dash                    # Interactive TUI
```

---

## 🔮 Roadmap

We are building the last dev tool you'll ever need.

### Current Features ✅
- [x] **`omg run <task>`**: Unified task runner. Detects 10+ project types (`package.json`, `Cargo.toml`, `Makefile`, `pyproject.toml`, etc.) and runs scripts with the correct runtime version pre-loaded.
- [x] **`omg new <stack>`**: Instant project scaffolding. `omg new react`, `omg new rust-cli`, or `omg new python-flask` sets up a best-practice environment with locked runtime versions.
- [x] **`omg doctor`**: System health check. Verifies PATHs, mirrors, PGP keys, and runtime integrity to debug environment issues instantly.
- [x] **`omg tool`**: Cross-ecosystem binary manager. Install dev tools (`ripgrep`, `jq`, `tldr`) from any source (Pacman, NPM, Cargo, Pip) into a single managed path.
- [x] **`omg dash`**: Interactive TUI dashboard. Real-time visualization of system status, vulnerabilities, and runtime versions.

### Planned Features 🚧
- [x] **Debian/Ubuntu Support**: Full APT integration (59-483x faster than apt-cache/Nala)
- [ ] **Fedora/RPM Support**: DNF/YUM package manager integration
- [ ] **macOS Support**: Homebrew integration for macOS users
- [ ] **Windows Support**: Chocolatey/Winget integration for Windows
- [x] **Container Integration**: Docker/Podman support for containerized environments (`omg container shell/run/build/init`)
- [ ] **GUI Dashboard**: Desktop application for visual package management
- [x] **Team Features**: Shared environment locks with collaborative workflows (`omg team init/join/status/push/pull`)

## 🧪 Testing & TDD

OMG adheres to a strict **Test-Driven Development (TDD)** protocol to ensure "absolute everything" is tested.

- **Red-Green-Refactor**: No feature is implemented without a failing test first.
- **100% Memory Safety**: Zero `unsafe` blocks are allowed in application logic.
- **Property-Based Testing**: Critical parsers and CLI commands are verified against thousands of random inputs via `proptest`.
- **Hardware-Limited Performance**: Benchmarks are required for every hot-path change to prevent performance regressions.

### Run the Suite
```bash
# Run all tests
cargo test

# Run TDD watch mode (requires cargo-watch)
make tdd

# Generate coverage report (requires cargo-tarpaulin)
make coverage
```

---

## 📜 License

**OMG is source-available commercial software.**

**Copyright © 2024-2026 OMG Team. All rights reserved.**

### Free for Individuals & Open Source
- ✅ Free for personal projects
- ✅ Free for open source projects
- ✅ Source code is public for transparency

### Paid for Commercial Use
- 💰 **Team License:** $99/month or $999/year (up to 25 developers)
- 💰 **Business License:** $199/month or $1,999/year (up to 75 developers)
- 💰 **Enterprise License:** Custom pricing (unlimited developers)

### Why Source-Available?

We believe in **transparency** (public source code) and **sustainability** (commercial funding). This model allows us to:
- Fund full-time development
- Provide professional support
- Ship features faster
- Keep OMG competitive

The source code is public on GitHub for security auditing and learning, but commercial use requires a paid license.

### Do I Need a Commercial License?

**YES** if you're:
- Working at a for-profit company (even for personal projects at work)
- Using OMG on company infrastructure
- Building commercial products with OMG
- A contractor/consultant using OMG for clients

**NO** if you're:
- An individual using OMG for personal projects
- Working on open source projects
- A student or educator

See **[COMMERCIAL-LICENSE](COMMERCIAL-LICENSE.md)** for full pricing and details.

### Third-Party Components

OMG incorporates third-party open source software:
- **[mise](https://github.com/jdx/mise)** - Runtime version management (MIT License, © 2025 Jeff Dickey)
- Various Rust crates (MIT/Apache-2.0 licenses)

See [NOTICE](NOTICE) and [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md) for complete attribution.

### Files

- [`LICENSE`](LICENSE) - Full license terms
- [`COMMERCIAL-LICENSE`](COMMERCIAL-LICENSE.md) - Pricing and purchasing
- [`NOTICE`](NOTICE) - Copyright and third-party notices
- [`THIRD-PARTY-LICENSES.md`](THIRD-PARTY-LICENSES.md) - Third-party licenses

### Contact for Licensing

📧 Email: **olen@latham.cloud**

For questions about commercial licensing, pricing, or purchasing.
