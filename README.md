# OMG

**The fastest unified package manager for Arch Linux + universal runtime version manager.**

[![Benchmark](https://img.shields.io/badge/search-5--11ms%20(12--24x%20faster)-brightgreen?style=flat-square)](benchmarks/latest.md)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange?style=flat-square)](https://www.rust-lang.org)
[![codecov](https://codecov.io/gh/pyro1121/omg/branch/main/graph/badge.svg?style=flat-square)](https://codecov.io/gh/pyro1121/omg)

OMG replaces `pacman`, `yay`, `nvm`, `pyenv`, `rustup`, `rbenv`, and `jenv` with a single binary. It queries packages in **5-11ms** via a lightweight background daemon that keeps repository indexes in memory.

---

## Before & After

```bash
# Before: 7 tools, 7 syntaxes, 7 configuration files
pacman -Ss firefox          # Official repositories
yay -S spotify              # AUR packages
nvm install 22 && nvm use 22# Node.js
pyenv install 3.12          # Python
rustup default stable       # Rust
rbenv install 3.3.0         # Ruby

# After: Just OMG
omg search firefox
omg install spotify
omg use node 22
omg use python 3.12
omg use rust stable
omg use ruby 3.3.0
```

---

## ⚡ Quick Install

### Universal Installer (Linux & macOS)

```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

### Arch Linux (AUR)

```bash
# Prebuilt binary (fastest)
yay -S omg-bin

# Build from source
yay -S omg
```

### From Source

```bash
cargo install omg --locked
```

### Shell Integration

Add instant directory-based runtime switching to your shell configuration:

```bash
# Zsh (~/.zshrc)
eval "$(omg hook zsh)"

# Bash (~/.bashrc)
eval "$(omg hook bash)"

# Fish (~/.config/fish/config.fish)
omg hook fish | source
```

---

## 🚀 60-Second Tour

```bash
# 1. Search packages across official repos and AUR in ~6ms
omg search ripgrep

# 2. Install official packages or AUR packages with auto-elevation
omg install visual-studio-code-bin

# 3. Switch language versions instantly
omg use node 22
omg use python 3.12

# 4. Run project scripts with auto-detected runtime versions
omg run dev

# 5. Lock your exact package and runtime environment for teammates
omg env capture

# 6. Launch the interactive terminal dashboard
omg dash
```

---

## 💡 Key Features

### 🏎️ 12-24x Faster Package Operations
Direct `libalpm` integration and an in-memory repository index eliminate subprocess startup overhead. Search results return in 5-11ms compared to 130-150ms with `pacman` and `yay`.

### 🛠️ Universal Runtime Manager
Manage Node.js, Bun, Python, Go, Rust, Ruby, Java, and Pi from one CLI. OMG honors existing `.nvmrc`, `.python-version`, `rust-toolchain.toml`, and `.tool-versions` files automatically.

### 📦 Seamless AUR & Dependency Resolution
Build and install AUR packages safely unprivileged as your regular user. Multi-package builds run in parallel with automatic makedepend cleanup.

### 🏃 Unified Task Runner
`omg run <task>` inspects your directory, detects `package.json`, `Cargo.toml`, `Makefile`, `pyproject.toml`, or `deno.json`, and executes the task with the required runtime version pre-loaded.

### 🔒 Environment Fingerprinting
`omg env capture` records installed packages and active runtimes into `omg.lock`. Teammates can run `omg env check` to detect drift and keep environments consistent.

### 📊 Terminal Dashboard
`omg dash` launches an interactive terminal UI for monitoring package updates, disk space, active runtimes, and system health at a glance.

---

## 📊 Performance Benchmarks

Measured on Arch Linux (Linux 6.18, AMD Ryzen / Intel i9, local pacman sync databases):

| Operation | OMG (Daemon) | pacman | yay | Speedup |
| :--- | :--- | :--- | :--- | :---: |
| **`search`** | **5.4-11.1ms** | 133ms | 150ms | **12-24x faster** |
| **`info`** | **3.4-6.1ms** | 138ms | 300ms | **21-38x faster** |
| **`explicit`** | **< 2ms** | 14ms | 27ms | **7-14x faster** |
| **`status`** | **< 10ms** | N/A | N/A | Instant overview |

*Detailed benchmark methodology and reproduction scripts are documented in [benchmarks/latest.md](benchmarks/latest.md).*

---

## 🌐 Supported Language Runtimes

OMG natively handles version switching and installation for major ecosystems:

| Runtime | Version Detection Files | Default Target |
| :--- | :--- | :--- |
| **Node.js** | `.nvmrc`, `.node-version`, `package.json` | Official prebuilt binaries |
| **Python** | `.python-version`, `pyproject.toml` | Standalone optimized builds |
| **Rust** | `rust-toolchain.toml`, `rust-toolchain` | Official rustup toolchains |
| **Go** | `.go-version`, `go.mod` | Official archive distributions |
| **Bun** | `.bun-version`, `package.json` | Official release builds |
| **Ruby** | `.ruby-version`, `Gemfile` | Ruby-build provider |
| **Java** | `.java-version` | Adoptium Temurin OpenJDK |

---

## 💻 CLI Command Reference

| Command | Description |
| :--- | :--- |
| `omg search <query>` | Search packages in official repositories and AUR |
| `omg install <pkg...>` | Install system or community packages |
| `omg remove <pkg...>` | Remove packages (`-r` removes unneeded dependencies) |
| `omg update` | Upgrade all system and AUR packages |
| `omg use <runtime> [ver]` | Install and switch to a language version |
| `omg run <task>` | Execute project scripts with correct runtimes |
| `omg doctor` | Diagnose environment, PATH, and mirror configuration |
| `omg clean` | Remove orphaned packages and clean package caches |
| `omg dash` | Open the interactive terminal dashboard |
| `omg why <pkg>` | Trace dependency chains explaining why a package is installed |
| `omg size` | View package disk usage breakdown and dependency trees |

Run `omg --help` or see the [CLI Documentation](docs/cli.md) for full argument details.

---

## 📜 License

OMG is free and open-source software licensed under the [MIT License](LICENSE).

Copyright (c) 2024-2026 OMG Team.
