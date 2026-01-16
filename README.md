# 🚀 OMG (Oh My God!)

**The Fastest Unified Package Manager for Arch Linux + All Language Runtimes**

OMG is a next-generation package manager designed for 2026 standards. It eliminates the friction of switching between `pacman`, `yay`, `nvm`, `pyenv`, and `rustup` by unifying them into a single, blazing-fast, Rust-native binary.
It now ships a full **runtime-aware task runner**, Bun-first JavaScript workflows, and native Rust toolchain management with `rust-toolchain.toml` support.

> "50-200x faster than traditional tools. Zero subprocess overhead. Zero-trust security."

---

## 📚 Table of Contents
- [Features](#features)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [In-Depth Documentation](#in-depth-documentation)
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
  - [Troubleshooting](#troubleshooting)
  - [Development](#development)
- [Roadmap](#roadmap-the-1-stop-shop-vision)
- [License](#license)

## ✨ Features

### 🏎️ Blazing Performance
- **Zero Subprocess Strategy**: Direct `libalpm` integration for system packages and pure Rust runtime managers. No more waiting for shell scripts to initialize.
- **Pure Rust Storage**: Embedded `redb` database for package metadata and completion caching. Queries are consistently **< 1 ms**.
- **Shared Client Pooling**: Reused network connections for lightning-fast API lookups and downloads.
- **Pure Rust Archives**: Native tar/zip/xz/zstd handling with no C dependencies.

### 🛠️ Unified Runtime Management
One command to rule them all. No more `.nvmrc` vs `.tool-versions` confusion.
- **Supported**: Node.js, Bun, Python, Go, Rust, Ruby, and Java.
- **Auto-Detection**: OMG detects required versions by climbing the directory tree for config files (`.nvmrc`, `.bun-version`, `.tool-versions`, `.mise.toml`, `.mise.local.toml`, `rust-toolchain.toml`).
- **Mise Runtime Support**: When a runtime isn't native, OMG falls back to `mise` (e.g., `omg run` honors `.mise.toml` and `.tool-versions`).
- **Node + NVM**: Prefers OMG-managed Node installs, but transparently falls back to local NVM versions when present.
- **Rust Toolchains**: Native Rust downloads with `rust-toolchain.toml` support (components, targets, profiles).
- **List Available**: `omg list node --available` shows real-time versions from official upstream APIs.

### 🛡️ Graded Security (2026 Standard)
OMG doesn't just install; it audits.
- **SLSA & PGP**: Built-in verification for build provenance and signatures.
- **Security Grading**: Every package is assigned a grade from `LOCKED` (Verified SLSA) to `RISK` (Known Vulnerabilities).
- **Policy Enforcement**: Define a global policy (`omg.policy.toml`) to block packages that don't meet your team's security standards.

### 👥 Team Sync & Drift Detection
- **Fingerprinting**: Generate a deterministic SHA256 hash of your entire environment (runtimes + packages).
- **Drift Protection**: `omg env check` alerts you the moment your local environment diverges from the project's `omg.lock`.
- **Gist Integration**: Share your exact setup instantly with `omg env share` and `omg env sync <url>`.

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
- **Fuzzy Matching**: Powered by `fuzzy-matcher` (SkimMatcherV2). Type `omg i frfx` and get `firefox`.
- **Context Aware**: Tab-completions prioritize versions and tools based on your current project directory.
- **80k+ AUR Cache**: Smooth, lag-free completion for the entire Arch User Repository.

### 🐧 Debian/Ubuntu Support (Experimental)
- **Backend**: Native `rust-apt` bindings (no `apt` subprocess).
- **Feature flag**: build with `--features debian` (requires `libapt-pkg-dev`).
- **Scope**: Official repo operations (search/info/install/remove/update). AUR features remain Arch-only.

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

---

## 📊 Real-World Performance

OMG achieves sub-5ms performance on all core operations through a persistent daemon that maintains an in-memory index of Arch packages.

**Benchmark Environment:**
- **CPU:** Intel i9-14900K (32 cores, 5.8GHz turbo)
- **RAM:** 31GB
- **Kernel:** Linux 6.18.3-arch1-1
- **Iterations:** 10 (with 2 warmup runs)

### Search, Info, and Status Commands

| Command | OMG (Daemon) | pacman | yay | Speedup |
|---------|--------------|--------|-----|---------:|
| **search** | **4.70ms** ✨ | 126.40ms | 1516.80ms | **27x faster** |
| **info** | **4.80ms** ✨ | 128.80ms | 267.50ms | **56x faster** |
| **status** | **4.60ms** ✨ | N/A | N/A | *OMG only* |
| **explicit** | **4.50ms** ✨ | 11.50ms | 20.90ms | 2.5x faster |

### Why These Numbers Matter

**Human Perception:**
- < 100ms = feels instant
- 100-500ms = noticeable delay
- > 500ms = clearly slow

OMG operates in the imperceptible range. Your fingers literally move faster than OMG responds.

**Annual Time Savings (10 package ops/day):**
| Tool | Per Year | 10-person team |
|------|----------|----------------|
| pacman | 12.6 minutes | 2.1 hours |
| yay | 151.7 minutes | 25.3 hours |
| **OMG** | **0.5 minutes** | **83 minutes reclaimed** |

**Verification**
Want to reproduce these numbers?
```bash
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/benchmark.sh | bash
```

---

## 🛠️ Architecture

OMG is split into two components:
1.  **`omg`**: A thin, high-performance CLI client.
2.  **`omgd`**: A persistent daemon that maintains an in-memory package cache and handles LMDB interactions.

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
- The daemon accelerates searches, status, and info lookups through caching and indexing.
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
- LMDB cache: `<data_dir>/db/`
- Daemon persistent cache: `<data_dir>/cache.mdb`

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
- `omg audit`: security audit (daemon required)

**Workflow helpers**
- `omg run <task> [-- <args...>]`: task runner
- `omg new <stack> <name>`: project scaffolding
- `omg tool <install|list|remove>`: cross-ecosystem tools

**Team sync & history**
- `omg env <capture|check|share|sync>`: environment lock management
- `omg history`: list transactions
- `omg rollback [id]`: rollback (official packages only for now)

**Daemon**
- `omg daemon`: start daemon in background
- `omgd`: run the daemon directly

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
Supported runtimes: **node, bun, python, go, rust, ruby, java**.

**Version detection**
- OMG scans parent directories for runtime version files.
- `.tool-versions` supports multiple runtimes in one file.

**Commands**
- `omg use node 20.10.0` installs and activates
- `omg list node --available` shows remote versions
- `omg which python` prints the active version

### Security Model
OMG assigns security grades based on package metadata and vulnerability scan results:
- **LOCKED**: SLSA + PGP verified
- **VERIFIED**: PGP / checksum verified
- **COMMUNITY**: AUR / unsigned sources
- **RISK**: known vulnerabilities found

You can enforce a policy in `~/.config/omg/policy.toml`:

```toml
minimum_grade = "Verified"
allow_aur = false
require_pgp = true
allowed_licenses = ["MIT", "Apache-2.0"]
banned_packages = ["example-bad-package"]
```

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

Rollback currently supports official packages only (downgrade-based) and is evolving.

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

### Debian Docker Smoke Test

Build the Debian container with automated smoke tests:

```bash
docker build -f Dockerfile.debian -t omg-debian-smoke .
```

Run the automated smoke test suite:

```bash
docker run --rm omg-debian-smoke
```

The smoke test automatically runs 20+ tests covering:
- OMG version and help commands
- System health checks (`omg doctor`)
- Database synchronization (`omg sync`)
- Package search and info queries
- Package installation and removal
- Update checks and status commands
- Error handling for invalid packages

All tests run as root to properly test apt operations.

For interactive manual testing:

```bash
docker run --rm -it omg-debian-smoke bash
```

Inside the container you can run any omg command:

```bash
omg search curl
omg info vim
omg install hello
omg remove hello
omg status
```

---

## 🔮 Roadmap (The "1-Stop Shop" Vision)

We are building the last dev tool you'll ever need.

- [x] **`omg run <task>`**: Unified task runner. Detects 10+ project types (`package.json`, `Cargo.toml`, `Makefile`, `pyproject.toml`, etc.) and runs scripts with the correct runtime version pre-loaded.
- [x] **`omg new <stack>`**: Instant project scaffolding. `omg new react`, `omg new rust-cli`, or `omg new python-flask` sets up a best-practice environment with locked runtime versions.
- [x] **`omg doctor`**: System health check. Verifies PATHs, mirrors, PGP keys, and runtime integrity to debug environment issues instantly.
- [x] **`omg tool`**: Cross-ecosystem binary manager. Install dev tools (`ripgrep`, `jq`, `tldr`) from any source (Pacman, NPM, Cargo, Pip) into a single managed path.

---

## 📜 License
MIT © 2026 OMG Team
