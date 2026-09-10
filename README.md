# OMG

<div align="center">

[![CI](https://github.com/PyRo1121/omg/actions/workflows/ci.yml/badge.svg)](https://github.com/PyRo1121/omg/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Website](https://img.shields.io/badge/website-getomg.xyz-blue)](https://getomg.xyz)
[![Docs](https://img.shields.io/badge/docs-getomg.xyz%2Fdocs-purple)](https://getomg.xyz/docs)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](rust-toolchain.toml)

**Your stack. One vocabulary.**

*Packages. Runtime versions. Project tasks. Declarative environment records.*  
*One high-performance Rust CLI for the work between writing code and running it.*

[Website](https://getomg.xyz) · [Documentation](https://getomg.xyz/docs) · [Quickstart](docs/quickstart.md) · [Installation](docs/installation.md) · [CLI Reference](docs/cli.md) · [Architecture](docs/architecture.md) · [Security Model](docs/security.md) · [Report Issue](https://github.com/PyRo1121/omg/issues)

</div>

---

> [!IMPORTANT]
> **Alpha Development Notice**: OMG is under active alpha development. Command surfaces, configuration flags, and on-disk formats may evolve. Always test package mutations on disposable VMs or recoverable developer machines. Keep native package tools (`pacman`, `apt`, `dnf`, `brew`) available.

---

## Why OMG?

Modern development environments are fragmented into half a dozen specialized utilities, each with bespoke syntax, conflicting configuration files, and sluggish shell shims. 

OMG consolidates this entire developer workflow into a single, cohesive interface while delegating execution to native platform backends and official toolchains:

| Capability | The Fragmented Toolchain | OMG Unified Workflow |
| :--- | :--- | :--- |
| **System Packages** | `pacman` / `apt` / `dnf` / `brew` / `yay` | `omg search` · `omg install` · `omg update` · `omg why` |
| **Language Runtimes** | `nvm` + `pyenv` + `rustup` + `gvm` + `rbenv` | `omg use <node\|python\|rust\|go\|deno> [version]` |
| **Version Detection** | Separate shims reading `.nvmrc`, `pyproject.toml`, `go.mod` | Auto-detects 18+ configuration & version formats |
| **Project Tasks** | `npm run` vs `pnpm` vs `cargo` vs `make` vs `poetry` | `omg run <dev\|test\|build\|lint>` (auto-resolves runner) |
| **Environment Parity** | Manual READMEs, ad-hoc Dockerfiles, hidden drift | `omg env capture` (generates `omg.lock`) · `omg env check` |
| **AUR Community Safety** | Ad-hoc AUR helpers blindly compiling scripts | Mandatory PKGBUILD review + isolated Bubblewrap builds |
| **Shell Prompt Vital Signs** | Spawning slow subshells (`pacman -Qu`) causing lag | Microsecond prompt status (`omg uc`) via atomic binary snapshots |

---

## 30-Second Quickstart (Zero-Risk Tour)

You can explore OMG immediately on an existing repository without modifying system packages or elevated permissions:

```bash
# 1. Switch or auto-detect a project runtime (e.g. Node 22, Python 3.12, Rust stable)
omg use node 22
omg which node

# 2. Execute any project script (detects package.json, Cargo.toml, Makefile, etc.)
omg run build

# 3. Snapshot and verify environment state against drift
omg env capture
omg env check

# 4. Search platform package repositories safely
omg search ripgrep
omg info ripgrep

# 5. Preview package actions without touching your system
omg install --dry-run ripgrep
```

---

## Installation

Release artifacts are cryptographically signed and attested through GitHub Actions. Verification requires `curl` and GitHub CLI (`gh`).

### Option A: Standard Inspected Install (Recommended)

Review the installer before execution, with telemetry disabled and shell edits bypassed:

```bash
# 1. Download installer for review
curl --proto '=https' --tlsv1.2 -fsSL https://getomg.xyz/install.sh -o omg-install.sh

# 2. Inspect script contents
less omg-install.sh

# 3. Execute with explicit hermetic flags
OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 bash omg-install.sh

# 4. Export to PATH and verify
export PATH="$HOME/.local/bin:$PATH"
omg --version
omg doctor
```

### Option B: Quick Install (Linux & macOS)

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://getomg.xyz/install.sh | bash
```

### Option C: Arch Linux (AUR)

```bash
yay -S omg-bin      # Precompiled release with daemon
# or build from source:
yay -S omg
```

### Option D: Build from Source

Requires Rust 1.95.0 (`rust-toolchain.toml`). Select your platform's backend feature:

```bash
# Arch Linux
cargo build --release --locked --no-default-features --features arch,pgp,license

# Debian / Ubuntu
cargo build --release --locked --no-default-features --features debian,pgp,license

# macOS (Apple Silicon)
cargo build --release --locked --no-default-features --features macos,pgp,license
```

> Windows users can run OMG inside [WSL2](https://learn.microsoft.com/en-us/windows/wsl/install) on a supported Linux distribution (Arch, Debian, or Ubuntu). A PowerShell helper is available at `https://getomg.xyz/install.ps1`. See [Installation Reference](docs/installation.md).

---

## System Architecture

OMG is designed around a dual-component architecture: an ergonomic, high-speed CLI front-end communicating over a Unix domain socket with an optional background daemon (`omgd`), with zero-overhead direct fallback paths:

```
┌───────────────────────────────────────────────────────────────────────────┐
│                                 USER / SHELL                              │
│                                       │                                   │
│                  ┌────────────────────┴────────────────────┐              │
│                  ▼                                         ▼              │
│           ┌──────────────┐                        ┌──────────────────┐    │
│           │   omg CLI    │                        │ Shell Prompt     │    │
│           │  (Front-End) │                        │ (ec/tc/oc/uc)    │    │
│           └──────┬───────┘                        └────────┬─────────┘    │
│                  │                                         │              │
│                  │ Length-delimited Unix Socket (bitcode)  │ Direct read  │
│                  ▼                                         ▼              │
│     ┌───────────────────────────────────────────────────────────────┐     │
│     │                         omgd (Daemon)                         │     │
│     │  ┌─────────────────────────┐     ┌─────────────────────────┐  │     │
│     │  │ In-Memory LRU Cache     │     │ Nucleo Fuzzy Matcher    │  │     │
│     │  │ (moka concurrent engine)│     │ (Package/AUR indexing)  │  │     │
│     │  └─────────────────────────┘     └─────────────────────────┘  │     │
│     │  ┌─────────────────────────┐     ┌─────────────────────────┐  │     │
│     │  │ Atomic State Snapshot   │     │ Hash-Chained Audit Log  │  │     │
│     │  │ (~/.local/share/omg)    │     │ (SHA-256 verification)  │  │     │
│     │  └─────────────────────────┘     └─────────────────────────┘  │     │
│     └────────────────────────────────┬──────────────────────────────┘     │
│                                      │                                    │
│          ┌───────────────────────────┼───────────────────────────┐        │
│          ▼                           ▼                           ▼        │
│   ┌──────────────┐            ┌──────────────┐            ┌─────────────┐ │
│   │ Native ALPM  │            │  Native APT  │            │   Polyglot  │ │
│   │ C FFI (Arch) │            │ (Debian/Ubu) │            │   Runtimes  │ │
│   └──────┬───────┘            └──────┬───────┘            └──────┬──────┘ │
│          │                           │                           │        │
│          ▼                           ▼                           ▼        │
│   ┌──────────────┐            ┌──────────────┐            ┌─────────────┐ │
│   │ Official Pac │            │ dpkg / APT   │            │ 14 Isolated │ │
│   │ + AUR Engine │            │ Repositories │            │ Toolchains  │ │
│   └──────────────┘            └──────────────┘            └─────────────┘ │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Zero-Subprocess ALPM Integration**: On Arch Linux, OMG links directly to `libalpm` via C FFI bindings, querying local and sync databases within process memory.
- **In-Memory Caching & Fuzzy Indexing**: `omgd` leverages `moka` for thread-safe in-memory caching and `nucleo` for instant search ranking.
- **Microsecond Shell Prompts**: Fast prompt counters (`omg uc`, `omg tc`) read directly from an atomic binary status snapshot (`omg.status`) on disk, completely bypassing async runtime initialization.
- **Resilient Direct Fallback**: Non-Arch platforms and standalone CLI invocations operate seamlessly via direct execution even if `omgd` is not running.

---

## Core Capabilities & Everyday Workflows

### 1. Unified Native Package Operations
One clear syntax across package backends. Search repositories, inspect dependencies, review reverse dependencies, and preview changes without switching tools:

```bash
# Search official repositories (and AUR on Arch)
omg search ripgrep
# Or use the quick alias:
omg s ripgrep

# Inspect why a package is installed and its dependency tree
omg info ripgrep
omg why ripgrep

# Preview installation without modifying system packages
omg install --dry-run ripgrep

# Install package (prompts for PKGBUILD review on AUR builds)
omg install ripgrep

# System-wide upgrade
omg update
```

### 2. Polyglot Runtime Management
Isolate and switch runtime toolchains per project or globally with zero external shell dependencies. 

OMG manages **14 native runtimes**: `node`, `python`, `go`, `rust`, `ruby`, `java`, `bun`, `deno`, `pi`, `zig`, `dotnet`, `erlang`, `php`, and `swift` (plus 54 registry developer tools).

```bash
# Switch runtime for the current project session
omg use node 22
omg use python 3.12
omg use rust stable
omg use go 1.22
omg use deno latest

# Automatically switch when changing directories (reads .nvmrc, go.mod, pyproject.toml, etc.)
echo "20.11.0" > .nvmrc
cd .
# ✓ Active Node.js switched to 20.11.0

# Inspect active binary resolution
omg which node
```

#### Shell Integration
Enable automatic directory switching by adding the hook to your shell configuration:
```bash
# Bash: ~/.bashrc
eval "$(omg hook bash)"

# Zsh: ~/.zshrc
eval "$(omg hook zsh)"

# Fish: ~/.config/fish/config.fish
omg hook fish | source
```

### 3. Unified Project Task Runner
Execute project lifecycles across ecosystems without remembering whether a repository uses `npm`, `pnpm`, `bun`, `cargo`, `make`, or `poetry`:

```bash
# Runs "build" via detected package.json / Cargo.toml / Makefile / pyproject.toml
omg run build

# Pass custom arguments directly to the underlying tool
omg run test -- --nocapture
```

### 4. Declarative Environment Locking (`omg.lock`)
Eliminate "works on my machine" issues across developer workstations and CI pipelines:

```bash
# Snapshot active system packages and project runtime versions
omg env capture

# Verify current machine against the recorded specification (exits nonzero on drift)
omg env check
```

### 5. Interactive Terminal Dashboard (TUI)
Full-screen terminal dashboard built with Ratatui for visual package management, health monitoring, and transaction logs:

```bash
omg dashboard
# Or quick alias:
omg dash
```

### 6. High-Speed In-Memory Daemon
An optional background daemon keeps repository indexes and caches warm for instantaneous search and status responses:

```bash
# Start and inspect the background daemon
omg daemon start
omg daemon-status
```

### 7. Supply Chain Security & Auditability
- **CycloneDX 1.5 SBOM**: Generate comprehensive package inventories via `omg audit sbom`.
- **AUR PKGBUILD Review**: Prompts for mandatory code review before building community packages.
- **SLSA / Rekor Verification**: Inspect build provenance and Rekor transparency logs with `omg audit slsa --certificate-identity <identity> <artifact>`.
- **Deterministic Rollbacks**: Explore transaction history and roll back changes with `omg rollback`.

---

## Platform & Backend Support Matrix

| Platform | Architecture | Package Backend | Daemon (`omgd`) | Coverage / Status |
| :--- | :--- | :--- | :---: | :--- |
| **Arch Linux** | `x86_64` | Native `libalpm` + AUR | Supported | Production-grade official repo and AUR engine with PKGBUILD review |
| **Debian** | `x86_64` | Native APT | Direct Fallback | Native APT package search, installation, and queries |
| **Ubuntu** | `x86_64` | Native APT | Direct Fallback | Native APT package search, installation, and queries |
| **Fedora** | `x86_64` | DNF | Direct Fallback | Experimental DNF backend (package mutations under active validation) |
| **macOS** | `aarch64` (Apple Silicon) | Homebrew | Direct Fallback | Native ARM64 binary with Homebrew package integration |
| **Windows** | `x86_64` | WSL2 | Supported | Supported via Linux guest in WSL2 (Arch, Ubuntu, or Debian) |

> [!NOTE]
> Daemon distribution is currently included in Arch Linux release archives. On other platforms, the `omg` CLI operates autonomously via direct execution fallbacks. Review [Installation Limits](docs/installation.md) for backend-specific details.

---

## Security Model & Integrity Disclosures

In accordance with our truthful, evidence-based engineering baseline:

1. **Supply Chain Attestation**: Official release archives and Cargo-generated SBOMs are cryptographically attested via GitHub Actions. The installer verifies digests and signatures using the GitHub CLI (`gh attestation verify`).
2. **AUR Safety Boundaries**: AUR packages contain community-submitted code. OMG enforces interactive PKGBUILD review by default and supports isolated Bubblewrap container builds (`bwrap`).
3. **Audit Limits**:
   - `omg audit sbom` produces a CycloneDX 1.5 JSON inventory with Arch Linux Security Advisory matching. It does not generate full transitive application dependency graphs for Debian or macOS.
   - `omg audit slsa` verifies supported Rekor signatures and Fulcio certificate chains for an artifact; it requires `--certificate-identity` and does not certify SLSA Levels 1–3 or build provenance.
   - Audit log verification (`omg audit verify`) confirms internal SHA-256 hash-chain consistency, not independent root-level authenticity.
   - OMG is not a compliance certification tool for SOC 2, ISO 27001, HIPAA, PCI DSS, or FedRAMP.
4. **Transparent Benchmarks**: We do not claim universal speedups. Performance varies by backend, cache state, repository size, and storage hardware. Inspect our methodology and raw records in [benchmarks/README.md](benchmarks/README.md).

---

## Documentation Hub

- 🚀 **[Quickstart Guide](docs/quickstart.md)** — Run your first runtime and task in 2 minutes.
- 📦 **[Installation Details](docs/installation.md)** — Requirements, checksums, distro configurations.
- 💻 **[Complete CLI Reference](docs/cli.md)** — Every command, flag, and option documented.
- ⚙️ **[Runtime Management](docs/runtimes.md)** — In-depth guide to runtime versioning and shell hooks.
- 🏃 **[Task Runner Guide](docs/task-runner.md)** — Resolution hierarchy, script definitions, and priorities.
- 🏛️ **[Architecture & Internals](docs/architecture.md)** — Deep dive into IPC, socket framing, and caching.
- 🔒 **[Security & Audit](docs/security.md)** — Vulnerability scanning, SBOM generation, and evidence limits.
- 🛠️ **[Cheatsheet](docs/cheatsheet.md)** — High-frequency commands for everyday development.
- 🩺 **[Troubleshooting](docs/troubleshooting.md)** — Resolving path errors, cache misses, and daemon issues.

---

## Contributing & Community

Contributions are welcome! Read [CONTRIBUTING.md](CONTRIBUTING.md) to understand development workflows, QEMU multi-distro test environments, and code standards.

- **Issue Tracker**: [GitHub Issues](https://github.com/PyRo1121/omg/issues) (include `omg --version`, distribution, and command output).
- **Vulnerability Disclosures**: Privately report security concerns via `<olen@latham.cloud>` per [SECURITY.md](SECURITY.md).

---

## License

OMG is open source under the [MIT License](LICENSE).  
Copyright © 2024–2026 Olen Latham.
