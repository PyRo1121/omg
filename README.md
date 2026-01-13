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
# Install a system package (Official or AUR)
omg install visual-studio-code-bin

# Switch to a specific Node.js version
omg use node 20.10.0

# Automatically use the version defined in your project
omg use python

# Run a security audit
omg audit
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

## 📜 License
MIT © 2026 OMG Team
