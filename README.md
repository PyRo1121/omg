# 🚀 OMG (Oh My God!)

**The Fastest Unified Package Manager for Arch Linux + All Language Runtimes**

OMG is a next-generation package manager designed for 2026 standards. It eliminates the friction of switching between `pacman`, `yay`, `nvm`, `pyenv`, and `rustup` by unifying them into a single, blazing-fast, Rust-native binary.

> "50-200x faster than traditional tools. Zero subprocess overhead. Zero-trust security."

---

## ✨ Features

### 🏎️ Blazing Performance
- **Zero Subprocess Strategy**: Direct `libalpm` integration for system packages and pure Rust runtime managers. No more waiting for shell scripts to initialize.
- **LMDB Backend**: 4GB memory-mapped database for package metadata and completion caching. Queries are consistently **< 1ms**.
- **Shared Client Pooling**: Reused network connections for lightning-fast API lookups and downloads.

### 🛠️ Unified Runtime Management
One command to rule them all. No more `.nvmrc` vs `.tool-versions` confusion.
- **Supported**: Node.js, Bun, Python, Go, Rust, Ruby, and Java.
- **Auto-Detection**: OMG automatically detects the required version by climbing the directory tree for configuration files.
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

OMG automatically **activates the correct runtime version** (e.g., Python virtual env, Node version from `.nvmrc`) before running the task.

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
- **Fuzzy Matching**: Powered by `nucleo-matcher`. Type `omg i frfx` and get `firefox`.
- **Context Aware**: Tab-completions prioritize versions and tools based on your current project directory.
- **80k+ AUR Cache**: Smooth, lag-free completion for the entire Arch User Repository.

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

## 📊 Benchmarks (Target)

| Operation | traditional (yay/nvm) | OMG | Improvement |
|-----------|-----------------------|-----|-------------|
| **Version Switch** | 150ms | **1.2ms** | **125x** |
| **Package Search** | 450ms | **8ms** | **56x** |
| **Shell Startup** | 800ms | **<5ms** | **160x** |

---

## 🛠️ Architecture

OMG is split into two components:
1.  **`omg`**: A thin, high-performance CLI client.
2.  **`omgd`**: A persistent daemon that maintains an in-memory package cache and handles LMDB interactions.

Communication happens over a high-speed Unix Domain Socket with a JSON-RPC protocol.

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
