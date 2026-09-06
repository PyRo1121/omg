# OMG

**The fastest unified package manager for Arch Linux + universal runtime version manager.**

> **Alpha.** OMG is alpha software. The CLI, flags, and on-disk formats can change without compatibility guarantees. Use it on machines you can recover, and file issues when something breaks.

[![Benchmark](https://img.shields.io/badge/search-13ms%20(19x%20vs%20pacman)-brightgreen?style=flat-square)](benchmarks/latest.md)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange?style=flat-square)](https://www.rust-lang.org)
[![codecov](https://codecov.io/gh/pyro1121/omg/branch/main/graph/badge.svg?style=flat-square)](https://codecov.io/gh/pyro1121/omg)

OMG replaces `pacman`, `yay`, `nvm`, `pyenv`, `rustup`, `rbenv`, and `jenv` with a single binary. It queries packages in **about 11ms** (13ms mean) via a background daemon that keeps repository indexes in memory.

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

## Quick Install

### Universal Installer (Linux & macOS)

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
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
cargo install omg --git https://github.com/PyRo1121/omg --locked
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

## 60-Second Tour

```bash
# 1. Search packages across official repos and AUR in ~11ms
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

## Key Features

### Faster Package Operations
Direct `libalpm` integration and an in-memory repository index eliminate subprocess startup overhead. `omg search firefox --no-aur` returns in **13ms mean / 11ms median** compared to **247ms** with `pacman` and **366ms** with `yay --repo`.

### Universal Runtime Manager
Manage Node.js, Bun, Python, Go, Rust, Ruby, Java, and Pi from one CLI. OMG honors existing `.nvmrc`, `.python-version`, `rust-toolchain.toml`, and `.tool-versions` files automatically.

### Seamless AUR & Dependency Resolution
Build and install AUR packages safely unprivileged as your regular user. Multi-package builds run in parallel with build artifact cleanup.

### Unified Task Runner
`omg run <task>` inspects your directory, detects `package.json`, `Cargo.toml`, `Makefile`, `pyproject.toml`, or `deno.json`, and executes the task with the required runtime version pre-loaded.

### Environment Fingerprinting
`omg env capture` records installed packages and active runtimes into `omg.lock`. Teammates can run `omg env check` to detect drift and keep environments consistent.

### Terminal Dashboard
`omg dash` launches an interactive terminal UI for monitoring package updates, disk space, active runtimes, and system health at a glance.

---

## Performance Benchmarks

Measured 2026-09-03 on Arch Linux (kernel 7.2.2, Intel Core i9-14900K, 31 GiB RAM, local pacman sync databases) with [hyperfine](https://github.com/sharkdp/hyperfine) 1.20. Timed commands were preflighted (search/info had to print `firefox`; explicit count was 273). Flags: `--shell=none --output=pipe`, 3 warmup, 20–50 runs.

| Operation | OMG (daemon) | omg-fast | pacman | yay | vs pacman |
| :--- | ---: | ---: | ---: | ---: | :---: |
| **`search firefox --no-aur`** | **13.1 ms** (median 11.4) | 11.2 ms | 247 ms | 366 ms | **19×** |
| **`info firefox`** | 26.4 ms | **10.7 ms** | 226 ms | 543 ms | 9× / **21×** fast |
| **`explicit --count`** | **10.4 ms** | 10.4 ms | 32 ms | 54 ms | **3×** |
| **`status`** | **11.9 ms** | 10.7 ms | — | — | — |
| **`update` discovery** | **829 ms** | — | — | — | — |

`omg search` / `omg info` / `omg status` go through the daemon. `omg-fast` is the thin IPC client (same index, less CLI work). Search speedup uses daemon mean vs `pacman -Ss`. Info’s 21× figure is `omg-fast` vs `pacman -Si`; the full `omg info` path is 9×.

Reproduce: [`./benchmark-hyperfine.sh`](benchmark-hyperfine.sh). This run: [benchmarks/records/20260903_015949-5c43ddcc](benchmarks/records/20260903_015949-5c43ddcc/). History: [benchmarks/latest.md](benchmarks/latest.md).

---

## Supported Language Runtimes

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

## CLI Command Reference

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

Run `omg --help` or see the [CLI documentation](docs/cli.md) for full argument details.

**More docs:** [Installation](docs/installation.md) · [Runtimes](docs/runtimes.md) · [Security](docs/security.md) · [Contributing](CONTRIBUTING.md)

---

## License

OMG is free and open-source software licensed under the [MIT License](LICENSE).

Copyright (c) 2024-2026 Olen Latham.
