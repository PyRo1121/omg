---
title: CLI Reference
sidebar_label: CLI Reference
sidebar_position: 2
description: Complete command reference for all OMG commands
---

# CLI Reference

**Complete Command Reference for OMG**

This guide documents every OMG command with detailed explanations, examples, and use cases. Commands are organized by category.

---

## 📋 Command Overview

| Category | Commands |
|----------|----------|
| **Package Management** | `search`, `install`, `remove`, `update`, `info`, `clean`, `explicit`, `sync` |
| **Runtime Management** | `use`, `list`, `which` |
| **Shell Integration** | `hook`, `hook-env`, `completions` |
| **Security & Audit** | `audit`, `status`, `doctor` |
| **Task Runner** | `run` |
| **Project Management** | `new`, `tool` |
| **Team Collaboration** | `env`, `team` |
| **Container Management** | `container` |
| **History & Rollback** | `history`, `rollback` |
| **Dashboard** | `dash`, `stats` |
| **License & Daemon** | `license`, `daemon` |

---

## 📦 Package Management

### omg search

Search for packages across official repositories and AUR.

```bash
omg search <query> [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--interactive` | `-i` | Interactive mode — select packages to install |
| `--limit <N>` | `-l` | Maximum results (default: 20) |
| `--aur` | `-a` | Search AUR only |
| `--official` | `-o` | Search official repos only |

**Examples:**
```bash
# Basic search
omg search firefox

# Interactive search (select to install)
omg search browser -i

# Limit results
omg search vim --limit 50

# Search AUR only
omg search -a spotify

# Search official repos only
omg search -o linux
```

**Performance:**
- With daemon: ~6ms
- Without daemon: ~50-200ms

---

### omg install

Install packages from official repositories or AUR.

```bash
omg install <packages...> [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--asdeps` | | Install as dependency |
| `--confirm` | | Skip confirmation prompt |

**Examples:**
```bash
# Install single package
omg install neovim

# Install multiple packages
omg install firefox chromium brave-bin

# Install AUR package
omg install visual-studio-code-bin

# Install as dependency
omg install --asdeps libfoo
```

**Security:**
- Packages are graded (LOCKED, VERIFIED, COMMUNITY, RISK)
- Policy enforcement applied before installation
- PGP signatures verified for official packages

---

### omg remove

Remove installed packages.

```bash
omg remove <packages...> [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--recursive` | `-r` | Also remove unneeded dependencies |

**Examples:**
```bash
# Remove single package
omg remove firefox

# Remove with dependencies
omg remove firefox -r

# Remove multiple packages
omg remove pkg1 pkg2 pkg3
```

---

### omg update

Update all packages or check for updates.

```bash
omg update [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--check` | `-c` | Only check for updates, don't install |

**Examples:**
```bash
# Update all packages (official + AUR)
omg update

# Check for updates only
omg update --check
```

**Update Flow:**
1. Sync package databases
2. Update official packages first
3. Build and update AUR packages
4. Record transaction in history

---

### omg info

Display detailed package information.

```bash
omg info <package>
```

**Examples:**
```bash
# Get info about a package
omg info firefox

# Get info about AUR package
omg info visual-studio-code-bin
```

**Output includes:**
- Package name and version
- Description
- Repository (official/AUR)
- Dependencies
- Installation status
- Security grade

**Performance:**
- With daemon: ~6.5ms (cached)
- Without daemon: ~50-200ms

---

### omg clean

Clean package caches and remove orphaned packages.

```bash
omg clean [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--orphans` | `-o` | Remove orphaned packages |
| `--cache` | `-c` | Clear package cache |
| `--aur` | `-a` | Clear AUR build cache |
| `--all` | | Clear everything |

**Examples:**
```bash
# Remove orphaned packages
omg clean --orphans

# Clear package cache
omg clean --cache

# Clear AUR build cache
omg clean --aur

# Full cleanup
omg clean --all
```

---

### omg explicit

List explicitly installed packages.

```bash
omg explicit [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--count` | `-c` | Only show count |

**Examples:**
```bash
# List all explicit packages
omg explicit

# Get count only
omg explicit --count
```

**Performance:**
- With daemon: 1.2ms
- Without daemon: ~14ms

---

### omg sync

Synchronize package databases.

```bash
omg sync
```

**Examples:**
```bash
# Sync databases
omg sync
```

---

## 🔧 Runtime Management

### omg use

Install and activate a runtime version.

```bash
omg use <runtime> [version]
```

**Supported Runtimes:**
| Runtime | Aliases | Version Files |
|---------|---------|---------------|
| `node` | `nodejs` | `.nvmrc`, `.node-version` |
| `bun` | `bunjs` | `.bun-version` |
| `python` | `python3` | `.python-version` |
| `go` | `golang` | `.go-version` |
| `rust` | `rustlang` | `rust-toolchain.toml` |
| `ruby` | | `.ruby-version` |
| `java` | | `.java-version` |

**100+ Additional Runtimes** (via built-in mise):
- Deno, Elixir, Erlang, Zig, Nim, Swift, Kotlin, .NET, PHP, Perl, Lua, Julia, R, and more

**Examples:**
```bash
# Install and use Node.js 20
omg use node 20.10.0

# Install and use latest LTS
omg use node lts

# Install Python 3.12
omg use python 3.12.0

# Use Rust stable
omg use rust stable

# Use Rust nightly
omg use rust nightly

# Install Deno (uses built-in mise)
omg use deno 1.40.0

# Install Elixir (uses built-in mise)
omg use elixir 1.16.0
```

**How It Works:**
1. Checks if version is installed
2. Downloads if not installed
3. Creates/updates `current` symlink
4. Updates PATH via shell hook

---

### omg list

List installed or available runtime versions.

```bash
omg list [runtime] [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--available` | `-a` | Show versions available for download |

**Examples:**
```bash
# List all installed versions for all runtimes
omg list

# List installed Node.js versions
omg list node

# List available Node.js versions
omg list node --available

# List available Python versions
omg list python --available
```

---

### omg which

Show which version of a runtime would be used.

```bash
omg which <runtime>
```

**Examples:**
```bash
# Check active Node.js version
omg which node

# Check active Python version
omg which python

# Check active Rust version
omg which rust
```

**Version Detection Order:**
1. Project-level version file (`.nvmrc`, etc.)
2. Parent directory version files (walking up)
3. Global `current` symlink

---

## 🐚 Shell Integration

### omg hook

Print the shell hook script.

```bash
omg hook <shell>
```

**Supported Shells:**
- `zsh`
- `bash`
- `fish`

**Examples:**
```bash
# Get Zsh hook
omg hook zsh

# Add to ~/.zshrc
eval "$(omg hook zsh)"

# Add to ~/.bashrc
eval "$(omg hook bash)"

# Add to ~/.config/fish/config.fish
omg hook fish | source
```

**Hook Features:**
- PATH modification on directory change
- Runtime version detection
- Ultra-fast package count functions

---

### omg completions

Generate shell completion scripts.

```bash
omg completions <shell> [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `--stdout` | Print to stdout instead of installing |

**Examples:**
```bash
# Install Zsh completions
omg completions zsh > ~/.zsh/completions/_omg

# Install Bash completions
omg completions bash > /etc/bash_completion.d/omg

# Install Fish completions
omg completions fish > ~/.config/fish/completions/omg.fish
```

---

## 🛡️ Security & Audit

### omg audit

Security audit suite with multiple subcommands.

```bash
omg audit [SUBCOMMAND]
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `scan` | Scan for vulnerabilities (default) |
| `sbom` | Generate CycloneDX 1.5 SBOM |
| `secrets` | Scan for leaked credentials |
| `log` | View audit log entries |
| `verify` | Verify audit log integrity |
| `policy` | Show security policy status |
| `slsa <pkg>` | Check SLSA provenance |

**Examples:**
```bash
# Vulnerability scan (default)
omg audit
omg audit scan

# Generate SBOM with vulnerabilities
omg audit sbom --vulns
omg audit sbom -o sbom.json

# Scan for secrets
omg audit secrets
omg audit secrets -p /path/to/project

# View audit log
omg audit log
omg audit log --limit 50
omg audit log --severity error

# Verify log integrity
omg audit verify

# Show policy status
omg audit policy

# Check SLSA provenance
omg audit slsa /path/to/package.pkg.tar.zst
```

---

### omg status

Display system status overview.

```bash
omg status
```

**Output includes:**
- Package counts (total, explicit, orphans)
- Available updates
- Active runtime versions
- Security vulnerabilities
- Daemon status

---

### omg doctor

Run system health checks.

```bash
omg doctor
```

**Checks performed:**
- PATH configuration
- Shell hook installation
- Daemon connectivity
- Mirror availability
- PGP keyring status
- Runtime integrity

---

## 🏃 Task Runner

### omg run

Run project tasks with automatic runtime detection.

```bash
omg run <task> [-- <args...>] [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `--list` | List available tasks |
| `--runtime-backend <backend>` | Force runtime backend (native, mise, native-then-mise) |

**Supported Project Files:**
| File | Runtime | Example |
|------|---------|---------|
| `package.json` | npm/yarn/pnpm/bun | `omg run dev` → `npm run dev` |
| `deno.json` | deno | `omg run dev` → `deno task dev` |
| `Cargo.toml` | cargo | `omg run test` → `cargo test` |
| `Makefile` | make | `omg run build` → `make build` |
| `Taskfile.yml` | task | `omg run build` → `task build` |
| `pyproject.toml` | poetry | `omg run serve` → `poetry run serve` |
| `Pipfile` | pipenv | `omg run lint` → `pipenv run lint` |
| `composer.json` | composer | `omg run test` → `composer run-script test` |
| `pom.xml` | maven | `omg run test` → `mvn test` |
| `build.gradle` | gradle | `omg run test` → `gradle test` |

**Examples:**
```bash
# Run development server
omg run dev

# Run tests with arguments
omg run test -- --watch

# List available tasks
omg run --list

# Force mise backend
omg run --runtime-backend mise dev
```

**JavaScript Package Manager Priority:**
1. `packageManager` field in package.json
2. Lockfile detection: `bun.lockb` → `pnpm-lock.yaml` → `yarn.lock` → `package-lock.json`
3. Default: bun (if available) → npm

---

## 🏗️ Project Management

### omg new

Create new projects from templates.

```bash
omg new <stack> <name>
```

**Available Stacks:**
| Stack | Description |
|-------|-------------|
| `rust` | Rust CLI project |
| `react` | React + Vite + TypeScript |
| `node` | Node.js project |
| `python` | Python project |
| `go` | Go project |

**Examples:**
```bash
# Create Rust CLI project
omg new rust my-cli

# Create React project
omg new react my-app

# Create Node.js API
omg new node api-server
```

---

### omg tool

Manage cross-ecosystem CLI tools.

```bash
omg tool <SUBCOMMAND>
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `install <name>` | Install a tool |
| `list` | List installed tools |
| `remove <name>` | Remove a tool |

**Examples:**
```bash
# Install ripgrep
omg tool install ripgrep

# Install jq
omg tool install jq

# List installed tools
omg tool list

# Remove a tool
omg tool remove ripgrep
```

**Tool Resolution:**
1. Check system package manager (pacman)
2. Fall back to cargo/npm/pip/go as appropriate
3. Install to `~/.local/share/omg/tools/`

---

## 🤝 Team Collaboration

### omg env

Manage environment lockfiles.

```bash
omg env <SUBCOMMAND>
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `capture` | Capture current state to `omg.lock` |
| `check` | Check for drift against `omg.lock` |
| `share` | Share via GitHub Gist |
| `sync <url>` | Sync from a shared Gist |

**Examples:**
```bash
# Capture current environment
omg env capture

# Check for drift
omg env check

# Share environment (requires GITHUB_TOKEN)
export GITHUB_TOKEN=your_token
omg env share

# Sync from shared environment
omg env sync https://gist.github.com/user/abc123
```

---

### omg team

Team workspace management.

```bash
omg team <SUBCOMMAND>
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `init <team-id>` | Initialize team workspace |
| `join <url>` | Join existing team |
| `status` | Show team sync status |
| `push` | Push local environment to team |
| `pull` | Pull team environment |
| `members` | List team members |

**Examples:**
```bash
# Initialize team workspace
omg team init mycompany/frontend

# Join existing team
omg team join https://github.com/mycompany/env-config

# Check status
omg team status

# Push changes
omg team push

# Pull updates
omg team pull

# List members
omg team members
```

---

## 🐳 Container Management

### omg container

Docker/Podman integration.

```bash
omg container <SUBCOMMAND>
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `status` | Show container runtime status |
| `shell` | Interactive dev shell |
| `run <image>` | Run command in container |
| `build` | Build container image |
| `init` | Generate Dockerfile |
| `list` | List running containers |
| `images` | List images |
| `pull <image>` | Pull image |
| `stop <container>` | Stop container |
| `exec <container>` | Execute in container |

**Examples:**
```bash
# Check container runtime
omg container status

# Interactive dev shell
omg container shell

# Run command in container
omg container run alpine -- echo "hello"

# Build image
omg container build -t myapp

# Generate Dockerfile
omg container init

# List containers
omg container list

# Stop container
omg container stop mycontainer
```

---

## 📜 History & Rollback

### omg history

View transaction history.

```bash
omg history [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--limit <N>` | `-l` | Number of entries (default: 20) |

**Examples:**
```bash
# View recent history
omg history

# View last 5 transactions
omg history --limit 5
```

---

### omg rollback

Rollback to a previous state.

```bash
omg rollback [transaction-id]
```

**Examples:**
```bash
# Interactive rollback
omg rollback

# Rollback specific transaction
omg rollback abc123
```

---

## 📊 Dashboard

### omg dash

Launch interactive TUI dashboard.

```bash
omg dash
```

**Keyboard Controls:**
| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Refresh |
| `Tab` | Switch view |

---

### omg stats

Display usage statistics.

```bash
omg stats
```

---

## 🔑 License & Daemon

### omg license

License management for Pro features.

```bash
omg license <SUBCOMMAND>
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `status` | Show license status |
| `activate <key>` | Activate license |
| `deactivate` | Deactivate license |
| `check <feature>` | Check feature availability |

---

### omg daemon

Start the background daemon.

```bash
omg daemon
```

For direct daemon control:
```bash
omgd --foreground  # Run in foreground
omgd --socket /path/to/socket  # Custom socket path
```

---

## ⚡ Ultra-Fast Queries

### omg-fast

Instant system queries for shell prompts.

```bash
omg-fast <subcommand>
```

**Subcommands:**
| Subcommand | Description | Latency |
|------------|-------------|---------|
| `status` | System status | 3ms |
| `ec` | Explicit count | &lt;1ms |
| `tc` | Total count | &lt;1ms |
| `uc` | Updates count | &lt;1ms |
| `oc` | Orphan count | &lt;1ms |

**Examples:**
```bash
# Get package counts for shell prompt
omg-fast ec
omg-fast tc

# Full status
omg-fast status
```

---

## 🌍 Global Options

These options work with all commands:

| Option | Short | Description |
|--------|-------|-------------|
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

---

## 📚 See Also

- [Introduction](/)
- [Configuration](./configuration.md)
- [Runtime Management](./runtimes.md)
- [Security & Compliance](./security.md)
