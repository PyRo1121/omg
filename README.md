# 🚀 OMG (Oh My God!)

**The Fastest Unified Package Manager for Arch Linux + All Language Runtimes**

![Installs](https://img.shields.io/endpoint?url=https://api.pyro1121.com/api/badge/installs&style=flat-square)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange?style=flat-square)](https://www.rust-lang.org)

OMG is a next-generation package manager designed for 2026 standards. It eliminates the friction of switching between `pacman`, `yay`, `nvm`, `pyenv`, and `rustup` by unifying them into a single, blazing-fast, Rust-native binary.
> **Note**: Supports **Arch Linux** and **Debian/Ubuntu**. RPM-based distros coming soon.

## ⭐ Big Changes
- **World-class performance** across system packages and language runtimes.
- **Swiss‑army‑knife unification** of system packages + language runtimes + tooling in one Rust-native binary.
- **Built for acquisition-grade scale**: single-binary deployment, zero-subprocess architecture, and measurable speed wins over legacy tooling.
- **Go-to-market in motion**: initial user growth planned with upcoming ad campaigns.

## 🧠 About the Founder (VC-Oriented)
**Olen Latham** — solo founder and builder. Focused on solid AI-first engineering and shipping high-performance developer infrastructure.

**Vision**: replace fragmented tooling with a single, best‑in‑class “developer Swiss Army knife” that is faster than anything on the market.

**Business trajectory**: early-stage with no current traction; marketing activation is planned to drive adoption. Long‑term path targets strategic acquisition by a larger software corporation.

It now ships a full **runtime-aware task runner**, Bun-first JavaScript workflows, and native Rust toolchain management with `rust-toolchain.toml` support.

> "22x faster than pacman/yay. Zero subprocess overhead. Zero-trust security."

---

## 📚 Table of Contents
- [Features](#-features)
- [Quick Start](#-quick-start)
- [Real-World Performance](#-real-world-performance)
- [Architecture](#️-architecture)
- [In-Depth Documentation](#-in-depth-documentation)
  - [Docs Hub](#docs-hub)
  - [Project Goals](#project-goals)
  - [How OMG Is Structured](#how-omg-is-structured)
  - [Core Concepts](#core-concepts)
  - [Data & Config Locations](#data--config-locations)
  - [Configuration Reference](#configuration-reference)
  - [CLI Reference](#cli-reference)
  - [Package Management Details](#package-management-details)
  - [Runtime Management](#runtime-management)
  - [Security Model](#security-model)
  - [Environment Lockfiles](#environment-lockfiles)
  - [Shell Integration](#shell-integration)
  - [Tool Management](#tool-management)
  - [Task Runner](#task-runner)
  - [Project Scaffolding](#project-scaffolding)
  - [Daemon & IPC](#daemon--ipc)
  - [History & Rollback](#history--rollback)
  - [Interactive Dashboard](#interactive-dashboard)
  - [Troubleshooting](#troubleshooting)
  - [Development](#development)
- [Roadmap](#-roadmap-the-1-stop-shop-vision)
- [License](#-license)

## ✨ Features

### 🏎️ Blazing Performance
- **Zero Subprocess Strategy**: Direct `libalpm` integration for system packages and pure Rust runtime managers. No more waiting for shell scripts to initialize.
- **Pure Rust Storage**: Embedded `redb` database for package metadata and completion caching. Queries are consistently **< 1 ms**.
- **Shared Client Pooling**: Reused network connections for lightning-fast API lookups and downloads.
- **Pure Rust Archives**: Native tar/zip/xz/zstd handling with no C dependencies.

### 🛠️ Unified Runtime Management
One command to rule them all. No more `.nvmrc` vs `.tool-versions` confusion.
- **Native Support**: Node.js, Bun, Python, Go, Rust, Ruby, and Java with pure Rust implementations.
- **Built-in Mise**: 100+ additional runtimes (Deno, Elixir, Zig, Erlang, Swift, etc.) via **bundled mise** - no separate installation required!
- **Auto-Detection**: OMG detects required versions by climbing the directory tree for config files (`.nvmrc`, `.bun-version`, `.tool-versions`, `.mise.toml`, `.mise.local.toml`, `rust-toolchain.toml`).
- **Seamless Fallback**: When a runtime isn't natively supported, OMG automatically downloads and uses mise - zero user intervention needed.
- **Node + NVM**: Prefers OMG-managed Node installs, but transparently falls back to local NVM versions when present.
- **Rust Toolchains**: Native Rust downloads with `rust-toolchain.toml` support (components, targets, profiles).
- **List Available**: `omg list node --available` shows real-time versions from official upstream APIs.

### 🛡️ Enterprise-Grade Security (2026 Standard)
OMG doesn't just install; it audits, verifies, and protects.
- **SLSA & PGP**: Built-in verification for build provenance and signatures using Sequoia-OpenPGP (PQC-ready).
- **Sigstore/Rekor Integration**: Binary transparency via the Sigstore public good infrastructure.
- **Security Grading**: Every package is assigned a grade from `LOCKED` (Verified SLSA) to `RISK` (Known Vulnerabilities).
- **SBOM Generation**: CycloneDX 1.5 compliant Software Bill of Materials for FDA, FedRAMP, and SOC2 compliance.
- **Secret Scanning**: Detect leaked credentials (AWS keys, GitHub tokens, private keys) before they're committed.
- **Tamper-Proof Audit Logs**: Hash-chained audit entries with integrity verification for compliance.
- **Policy Enforcement**: Define a global policy (`policy.toml`) to block packages that don't meet your team's security standards.

### 👥 Team Sync & Drift Detection
- **Fingerprinting**: Generate a deterministic SHA256 hash of your entire environment (runtimes + packages).
- **Drift Protection**: `omg env check` alerts you the moment your local environment diverges from the project's `omg.lock`.
- **Gist Integration**: Share your exact setup instantly with `omg env share` and `omg env sync <url>`.

### 🐳 Container Integration (Docker/Podman)
- **Auto-Detection**: Automatically detects Docker or Podman (prefers Podman for rootless security).
- **Dev Shells**: `omg container shell` mounts your project and drops you into an interactive container.
- **Build Support**: `omg container build` builds images from Dockerfiles with OMG-optimized defaults.
- **Project Init**: `omg container init` generates a Dockerfile based on detected project runtimes.

### 🏃 Unified Task Runner
Stop guessing if it's `npm run`, `cargo run`, `make`, `maven`, or `gradle`.

```bash
# Automatically detects package.json, Cargo.toml, Makefile, pyproject.toml, etc.
omg run build

# Passes arguments through
omg run test -- --watch
```

OMG automatically **activates the correct runtime version** (e.g., Python virtual env, Node version from `.nvmrc`, Bun from `.bun-version`, or any Mise-managed runtime) before running the task.
For JavaScript projects, it respects `package.json#packageManager` and lockfiles to pick **Bun → pnpm → yarn → npm**, and adds a default `install` task.
If `pnpm` or `yarn` are missing, OMG can enable them via **corepack** on demand.
Supported task sources include `package.json`, `deno.json`, `Cargo.toml`, `Makefile`, `Taskfile.yml`, `pyproject.toml` (Poetry), `Pipfile`, `composer.json`, `pom.xml`, and `build.gradle`.

#### JavaScript Runtime Behavior (Bun-First)
- If `package.json` includes `"packageManager": "bun@1.2.5"`, OMG runs tasks with **Bun** and ensures that version.
- If `package.json` includes `"packageManager": "pnpm@9"` or `"yarn@4"`, OMG will prompt to enable **corepack** if needed.
- If no `packageManager` is set, OMG uses lockfiles in priority order: **bun.lockb → pnpm-lock.yaml → yarn.lock → package-lock.json**.
- `.bun-version` or `.nvmrc` override defaults for the runtime version when present.

#### Runtime Auto-Install Prompts
When you run `omg run ...`, OMG checks for required runtime versions and prompts to install if missing:
- **Rust**: reads `rust-toolchain.toml`/`rust-toolchain` and installs missing toolchains, components, or targets.
- **Node/Bun**: installs the required version if it is not already present (prefers OMG installs, but detects local NVM installs for Node).
- **pnpm/yarn**: if selected by `packageManager`, OMG can enable them via **corepack**.
- **Mise**: `omg run` can be forced to use Mise with `--runtime-backend mise`.

#### Task Runner Detection Matrix
The task runner auto-detects common project files and routes commands accordingly:

| Project File | Runtime/Tool | Example Command |
|--------------|--------------|-----------------|
| `package.json` | bun/pnpm/yarn/npm | `omg run dev` → `bun run dev` |
| `deno.json` | deno | `omg run dev` → `deno task dev` |
| `Cargo.toml` | cargo | `omg run test` → `cargo test` |
| `Makefile` | make | `omg run build` → `make build` |
| `Taskfile.yml` | task | `omg run list` → `task --list` |
| `pyproject.toml` (Poetry) | poetry | `omg run api` → `poetry run api` |
| `Pipfile` | pipenv | `omg run lint` → `pipenv run lint` |
| `composer.json` | composer | `omg run test` → `composer run-script test` |
| `pom.xml` / `build.gradle` | maven/gradle | `omg run test` → `mvn test` |

### 🏗️ Instant Scaffolding
Start new projects with best practices built-in.

```bash
# Create a React project (Vite + TypeScript) and lock Node version
omg new react my-app

# Create a Rust CLI and lock Rust version
omg new rust my-cli
```

### 🧰 Cross-Ecosystem Tools
Install dev tools (`ripgrep`, `jq`, `tldr`) without worrying about *how*.

```bash
# Installs from Pacman (system) if available, or Cargo/NPM (isolated) if not.
omg tool install ripgrep

# OMG manages the isolation and linking automatically.
omg tool list
```

### 🧠 Intelligent Completions
- **Fuzzy Matching**: Powered by **Nucleo** for ultra-fast, high-precision matching. Type `omg i frfx` and get `firefox`.
- **Context Aware**: Tab-completions prioritize versions and tools based on your current project directory.
- **80k+ AUR Cache**: Smooth, lag-free completion for the entire Arch User Repository.
- **Interactive TUI**: New `omg dash` dashboard for real-time system monitoring.

---

## 🚀 Quick Start

### 1. Installation

**Recommended: The One-Line Installer**
Automatically installs dependencies, builds the project, and configures your shell.

```bash
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
```

**Alternative: Manual Build**
If you prefer to build it yourself:

```bash
# Clone the repository
git clone https://github.com/PyRo1121/omg.git
cd omg

# Build release binary
cargo build --release

# Install to your path
cp target/release/omg ~/.local/bin/
```

### 2. Basic Commands
```bash
# Interactive Search (Select packages to install)
omg search vim -i

# Smart Install (Auto-corrects typos)
omg install vscodium-bin

# Install a system package (Official or AUR)
omg install visual-studio-code-bin

# Switch to a specific Node.js version
omg use node 20.10.0

# Run project scripts (context-aware)
omg run dev

# Check system health
omg doctor

# Interactive Dashboard
omg dash

# Install universal tools
omg tool install ripgrep
```

### 3. Team Sync
```bash
# Lock your current environment
omg env capture

# Share with a teammate
omg env share

# Teammate syncs
omg env sync <gist-url>
```

### 4. Container Development
```bash
# Check container runtime status
omg container status

# Start a dev shell with project mounted
omg container shell

# Generate a Dockerfile for your project
omg container init

# Build and run
omg container build -t myapp
omg container run myapp -- npm start
```

---

## 📊 Real-World Performance

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

## 📚 In-Depth Documentation

### Docs Hub
For focused guides beyond the README, see [docs/index.md](docs/index.md).

### Project Goals
OMG is built around three top-level goals:
1. **Unify tools**: one binary for system packages and runtime versions.
2. **Performance-first**: sub-10ms searches, low-latency status checks, and fast runtime switching.
3. **Security by default**: graded policy enforcement, vulnerability awareness, and provenance checks.

### How OMG Is Structured
OMG ships as two binaries backed by a shared Rust library:
- **`omg` (CLI)**: the user-facing interface that executes commands, falls back to direct operations when the daemon is unavailable, and prefers fast-path synchronous queries when possible.
- **`omgd` (daemon)**: a persistent background service that caches package metadata, maintains an in-memory index, and handles ultra-fast requests over a Unix socket.
- **`omg_lib`**: shared logic used by both binaries (CLI modules, daemon modules, runtime managers, security, etc.).

### Core Concepts
**Unified Package + Runtime Management**
- System packages (official repos + AUR) and runtime versions (Node, Python, Go, Rust, Ruby, Java, Bun) are first-class citizens in the same CLI.
- Runtime versions can be auto-detected from files like `.nvmrc`, `.python-version`, `.tool-versions`, and `rust-toolchain.toml`.

**Daemon-Optional, Performance-First**
- The daemon accelerates searches, status, and info lookups through moka caching and Nucleo indexing.
- If the daemon is not running, the CLI transparently falls back to direct libalpm and runtime calls.

**Graded Security**
- Packages are assigned grades (LOCKED, VERIFIED, COMMUNITY, RISK) and evaluated by policy before install.
- Policy can block AUR usage or ban packages by name/license.

### Data & Config Locations
Default paths follow the XDG base directories when available.

**Data directory** (runtime versions, caches, DB):
- Linux default: `~/.local/share/omg/` (XDG data dir)
- Fallback: `~/.omg/`

**Database and caches**:
- redb persistent status: `<data_dir>/cache.redb`
- moka in-memory cache managed by daemon

**Config**:
- `~/.config/omg/config.toml`
- `~/.config/omg/policy.toml` (security policy)

**Runtime versions**:
- `<data_dir>/versions/<runtime>/<version>`

**Socket**:
- Default: `$XDG_RUNTIME_DIR/omg.sock`
- Fallback: `/tmp/omg.sock`

**Environment lock**:
- Project-local: `omg.lock`

**History**:
- `~/.local/share/omg/history.json`

### Configuration Reference
`config.toml` controls daemon and runtime behavior. If the file does not exist, defaults are used.

```toml
# ~/.config/omg/config.toml
shims_enabled = false
data_dir = "/home/you/.local/share/omg"
socket_path = "/run/user/1000/omg.sock"
default_shell = "zsh"
auto_update = false

[aur]
build_concurrency = 8
makeflags = "-j8"
pkgdest = "/home/you/.cache/omg/pkgdest"
srcdest = "/home/you/.cache/omg/srcdest"
cache_builds = true
enable_ccache = false
ccache_dir = "/home/you/.cache/ccache"
enable_sccache = false
sccache_dir = "/home/you/.cache/sccache"
```

### CLI Reference
Below is a high-level map of the command surface. Run `omg <command> --help` for detailed flags.

**Package management**
- `omg search <query>`: search official repos + AUR
- `omg install <pkg...>`: install packages with security grading
- `omg remove <pkg...>`: remove packages (optionally recursive)
- `omg update`: update official + AUR packages
- `omg info <pkg>`: detailed package info (daemon-accelerated)
- `omg clean`: clear caches/orphans
- `omg explicit`: list explicitly installed packages
- `omg sync`: sync package databases

**Runtime management**
- `omg use <runtime> [version]`: install + activate runtime versions
- `omg list [runtime] --available`: list installed or available versions
- `omg which <runtime>`: show active version

**Shell integration**
- `omg hook <shell>`: print shell hook
- `omg hook-env`: internal PATH update hook
- `omg completions <shell>`: install completions

**System & security**
- `omg status`: system overview (packages, updates, runtimes, security)
- `omg doctor`: health checks
- `omg audit`: security audit suite with subcommands:
  - `omg audit scan`: vulnerability scanning (default)
  - `omg audit sbom`: generate CycloneDX 1.5 SBOM
  - `omg audit secrets`: scan for leaked credentials
  - `omg audit log`: view tamper-proof audit log
  - `omg audit verify`: verify audit log integrity
  - `omg audit policy`: show security policy status
  - `omg audit slsa <pkg>`: check SLSA provenance

**Workflow helpers**
- `omg run <task> [-- <args...>]`: task runner
- `omg dash`: interactive TUI dashboard (NEW)
- `omg new <stack> <name>`: project scaffolding
- `omg tool <install|list|remove>`: cross-ecosystem tools

**Team sync & history**
- `omg env <capture|check|share|sync>`: environment lock management
- `omg team init <team-id>`: initialize team workspace with git hooks
- `omg team join <url>`: join existing team by remote URL
- `omg team status`: show team sync status and members
- `omg team push`: push local environment to team lock
- `omg team pull`: pull team lock and check for drift
- `omg team members`: list team members and sync status
- `omg history`: list transactions
- `omg rollback [id]`: rollback (official packages only for now)

**Container management (Docker/Podman)**
- `omg container status`: show container runtime status
- `omg container shell`: interactive dev shell with project mounted
- `omg container run <image> [-- cmd]`: run command in container
- `omg container build [-t tag]`: build container image
- `omg container init`: generate Dockerfile for project
- `omg container list`: list running containers
- `omg container images`: list container images
- `omg container pull <image>`: pull container image
- `omg container stop <container>`: stop running container
- `omg container exec <container> [-- cmd]`: exec in running container

**Daemon**
- `omg daemon`: start daemon in background
- `omgd`: run the daemon directly

**License management (Pro features)**
- `omg license status`: show current license status
- `omg license activate <key>`: activate a license key
- `omg license deactivate`: deactivate current license
- `omg license check <feature>`: check if a feature is available

**Ultra-fast queries (omg-fast)**
- `omg-fast status`: instant system status (3ms)
- `omg-fast ec`: explicit package count (sub-ms)
- `omg-fast tc`: total package count
- `omg-fast uc`: updates count
- `omg-fast oc`: orphan count

### Package Management Details
**Search flow**
1. CLI tries the daemon cache for instant results.
2. If unavailable, it falls back to direct libalpm + AUR searches.
3. Interactive mode can select packages to install immediately.

**Install flow**
1. Package security grade is assigned.
2. Security policy is enforced (AUR rules, license rules, banned packages).
3. Official packages are installed via pacman; AUR packages are built and installed.

**Update flow**
- Official repo updates are downloaded and installed first.
- AUR packages are built in parallel using configured concurrency.

### Runtime Management
**Native runtimes**: node, bun, python, go, rust, ruby, java (pure Rust implementations).
**Extended runtimes**: 100+ more via built-in mise (deno, elixir, zig, erlang, swift, dotnet, php, etc.).

**Version detection**
- OMG scans parent directories for runtime version files.
- Supports: `.nvmrc`, `.node-version`, `.python-version`, `.ruby-version`, `.go-version`, `.java-version`, `.bun-version`, `rust-toolchain.toml`, `.tool-versions`, `.mise.toml`
- `package.json` engines/volta fields are also detected for Node.js

**Commands**
- `omg use node 20.10.0` installs and activates
- `omg use deno 1.40.0` auto-installs mise if needed, then installs deno
- `omg list node --available` shows remote versions
- `omg which python` prints the active version

### Security Model
OMG implements enterprise-grade security with defense-in-depth:

**Security Grades**
- **LOCKED**: SLSA Level 3 + PGP verified (core system packages)
- **VERIFIED**: PGP / checksum verified (official repos)
- **COMMUNITY**: AUR / unsigned sources
- **RISK**: known vulnerabilities found

**Enterprise Security Features**
- **SBOM Generation**: `omg audit sbom` generates CycloneDX 1.5 compliant SBOMs
- **Secret Scanning**: `omg audit secrets` detects 20+ credential types (AWS, GitHub, Stripe, etc.)
- **Audit Logging**: Tamper-proof, hash-chained logs with `omg audit verify`
- **SLSA Verification**: `omg audit slsa <pkg>` checks Rekor transparency logs
- **Vulnerability Scanning**: OSV.dev + Arch Linux Security Advisory integration

**Policy Enforcement** (`~/.config/omg/policy.toml`):

```toml
minimum_grade = "Verified"
allow_aur = false
require_pgp = true
allowed_licenses = ["AGPL-3.0-or-later", "Apache-2.0"]
banned_packages = ["example-bad-package"]
```

See [docs/security.md](docs/security.md) for complete security documentation.

### Environment Lockfiles
OMG captures environment state (runtime versions + explicit packages) into `omg.lock`.

**Common commands**
- `omg env capture`: write current state
- `omg env check`: detect drift
- `omg env share`: upload `omg.lock` as a GitHub Gist
- `omg env sync <gist>`: download and validate a lockfile

> Note: `omg env share` requires `GITHUB_TOKEN` to be set.

### Shell Integration
OMG uses a fast shell hook (similar to tools like mise) to update `PATH` on directory change.

```bash
# Zsh
eval "$(omg hook zsh)"

# Bash
eval "$(omg hook bash)"

# Fish
omg hook fish | source
```

The hook detects runtime version files and prepends the correct runtime `bin` directory to `PATH`.

**Completions**
- `omg completions zsh`
- `omg completions bash`
- `omg completions fish`

### Tool Management
`omg tool` installs CLI tools using the optimal backend (pacman, cargo, npm, pip, or go).

**Examples**
```bash
omg tool install ripgrep
omg tool list
omg tool remove ripgrep
```

Tools are installed into an isolated data directory and symlinked into `<data_dir>/bin`, so they
are accessible via your shell hook PATH.

### Task Runner
`omg run` auto-detects tasks from common files like `package.json`, `Cargo.toml`, `Makefile`,
`pyproject.toml`, `deno.json`, or `composer.json` and executes them with the correct runtime loaded.

```bash
omg run build
omg run test -- --watch
```

### Project Scaffolding
Create new projects with pre-configured runtime locks.

```bash
omg new rust my-cli
omg new react my-app
omg new node api-server
```

### Daemon & IPC
The daemon (`omgd`) provides:
- **In-memory package index** for ultra-fast search.
- **LRU cache** for repeated queries.
- **Background refresh** of runtime versions and status.

IPC uses length-delimited framing with bincode serialization to keep latency minimal.

To start the daemon:
```bash
omg daemon
# or
omgd --foreground
```

### History & Rollback
OMG stores a transaction log (`history.json`) with up to the last 1000 changes.

```bash
omg history
omg rollback
```

Rollback currently supports official packages only (downgrade-based).

### Interactive Dashboard
The real-time dashboard (`omg dash`) provides a unified view of your system health, package updates, and active runtime versions. It uses `ratatui` for a premium terminal experience.

See [docs/tui.md](docs/tui.md) for full documentation.

### Troubleshooting
**Daemon not running**
- Run `omg daemon` or `omgd --foreground`.

**Security audit fails**
- Ensure the daemon is running. `omg audit` requires the daemon.

**PATH not updated**
- Make sure your shell has the `omg hook` installed and restart your shell.

**AUR builds failing**
- Ensure base dependencies (`git`, `curl`, `tar`, `sudo`) are installed.
- Use `omg doctor` to check system health.

### Development
Build/test commands (see AGENTS.md for more):

```bash
cargo build
cargo build --release
cargo check
cargo test
cargo clippy
```

Use `cargo run -- <command>` to exercise CLI paths during development.

---

## 🔮 Roadmap (The "1-Stop Shop" Vision)

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

---

## 📜 License
AGPL-3.0-or-later © 2026 OMG Team
