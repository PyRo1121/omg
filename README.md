# OMG

**The fastest unified package manager for Arch Linux + universal runtime version manager.**

> **Alpha.** OMG is alpha software. The CLI, flags, and on-disk formats can change without compatibility guarantees. Use it on machines you can recover, and file issues when something breaks.

[![Benchmark evidence](https://img.shields.io/badge/benchmarks-scope%20and%20raw%20data-blue?style=flat-square)](benchmarks/README.md)
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
OMG uses direct `libalpm` integration and an in-memory repository index on Arch. A recorded local development run measured daemon-backed search at 13.1 ms mean. That run did not establish equivalent search output or cross-distribution release performance. See [benchmark scope and evidence](benchmarks/README.md).

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

## Release smoke evidence and performance

The published `v0.1.218` x86_64 artifacts were tested locally in disposable Docker
containers using runner commit `e77c60ef`. These are single-run smoke durations,
not CLI latency benchmarks. Each includes package-index preparation, assertions,
and container cleanup. Archive downloads and image pulls are excluded.

- Arch passed search, install, and remove in 4, 2, and 4 seconds.
- Debian passed those cases in 11, 12, and 13 seconds.
- Ubuntu passed those cases in 24, 26, and 24 seconds.
- Fedora search and install failed in 12 and 9 seconds. Removal setup failed in
  9 seconds before removal executed. Fedora is not passing package coverage.

[Recorded results and artifact digests](benchmarks/records/release-smoke-v0.1.218-local.json)
identify the binaries and pinned container images. A separate
[CI run of the same runner](https://github.com/PyRo1121/omg/actions/runs/33912270669)
contains transcripts and uploaded cleanup evidence. These results prove only
these package cases on these images, not every command or every Linux distribution.
The [local headless QEMU runner](scripts/README.md#benchmark-qemush) now passes
boot, reboot, sudo, and package lifecycles on all four supported x86_64 baselines.
The [live receipt](benchmarks/records/qemu-four-distros-20260905.json) contains
artifact hashes and 240 warm timing samples. Debian and Fedora use fixed local
candidates. These results do not mean their published artifacts are fixed, that
every CLI command passes, or that debug-build timings establish release speedups.

Run the current suite from a shell with Docker access:

```bash
./scripts/release-smoke.sh --release v0.1.218 --distro all \
  --evidence-dir "$HOME/.cache/build-targets/omg-smoke-evidence"
```

The command currently exits nonzero because Fedora does not pass. See
[benchmark methodology, limitations, and the researched QEMU design](benchmarks/README.md)
for the distinction between smoke coverage and performance. Historical
[Arch development measurements](benchmarks/records/20260903_015949-5c43ddcc/)
remain available, but are not release-wide speedup claims.

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
