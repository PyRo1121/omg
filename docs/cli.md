---
title: CLI Reference
sidebar_position: 3
description: Complete command reference for all OMG commands
---

# CLI Reference

**Complete Command Reference for OMG**

This guide documents every OMG command with detailed explanations, examples, and use cases. Commands are organized by category.

---

## 📋 Command Overview

| Category | Commands |
| ---------- | ---------- |
| **Package Management** | `search`, `install`, `remove`, `update`, `info`, `clean`, `explicit`, `sync`, `why`, `outdated`, `size`, `blame` |
| **Runtime Management** | `use`, `list`, `which` |
| **Shell Integration** | `hook`, `completions`, `hooks`, `workspace` |
| **Security & Audit** | `audit`, `status`, `doctor` |
| **Task Runner** | `run` |
| **Project Management** | `new`, `tool`, `init`, `self-update` |
| **Environment & Snapshots** | `env`, `snapshot`, `diff` |
| **Team Collaboration** | `team`, `privacy` |
| **Container Management** | `container` |
| **CI/CD & Migration** | `ci`, `migrate` |
| **History & Rollback** | `history`, `rollback` |
| **Dashboard** | `dash`, `stats`, `metrics`, `daemon-status` |
| **Configuration** | `config`, `daemon`, `account`, `generate-man` |
| **Enterprise** | `fleet`, `enterprise` |

---

## 📦 Package Management

### omg search

Search for packages across official repositories and AUR.

```bash
omg search <query> [OPTIONS]
```

**Options:**

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--detailed` | `-d` | Show detailed source metadata (votes, popularity where available) |
| `--no-aur` | | Search official repositories only (skip community sources) |
| `--limit <LIMIT>` | `-l` | Maximum number of results to display (default: 50) |

**Examples:**

```bash
# Basic search
omg search firefox

# Detailed search with AUR votes/popularity
omg search spotify -d

# Limit results
omg search node --limit 10
```

**Performance:**

- With daemon: ~5-11ms
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
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be installed without making changes |

**Examples:**

```bash
# Install single package
omg install neovim

# Install multiple packages
omg install firefox chromium brave-bin

# Install AUR package
omg install visual-studio-code-bin

# Skip confirmation
omg install neovim -y
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
| -------- | ------- | ------------- |
| `--recursive` | `-r` | Also remove unused dependencies |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be removed without making changes |

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
| -------- | ------- | ------------- |
| `--check` | `-c` | Only check for updates, don't install |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be updated without making changes |
| `--fast` | `-f` | Fast mode: sync + upgrade in a single operation (no preview) |
| `--turbo` | `-T` | Turbo mode: skip sync, use cached data, parallel extraction (fastest) |

**Examples:**

```bash
# Update all packages (official + AUR)
omg update

# Check for updates only
omg update --check

# Fast path without preview
omg update --fast
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

- With daemon: ~3-6ms (cached)
- Without daemon: ~50-200ms

---

### omg clean

Clean package caches and remove orphaned packages.

```bash
omg clean [OPTIONS]
```

**Options:**

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--orphans` | `-o` | Remove orphaned packages |
| `--cache` | `-c` | Clear package cache |
| `--aur` | | Clear build directories for source-based installs |
| `--all` | `-a` | Remove all (orphans + cache + aur) |
| `--dry-run` | | Show what would be cleaned without making changes |

**Examples:**

```bash
# Remove orphaned packages
omg clean --orphans

# Clear package cache
omg clean --cache

# Clear AUR build directories
omg clean --aur

# Full cleanup
omg clean --all

# Preview without changing anything
omg clean --dry-run --all
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

- With daemon: <2ms
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

### omg why

Explain why a package is installed by showing its dependency chain.

```bash
omg why <package> [OPTIONS]
```

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--reverse` | `-r` | Show what depends on this package |

**Examples:**

```bash
# See why a package is installed
omg why libxcb

# See what depends on a package
omg why openssl --reverse
```

**Output includes:**

- Dependency chain from explicit packages
- Whether safe to remove
- Number of dependents

---

### omg outdated

Show packages with available updates.

```bash
omg outdated [OPTIONS]
```

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | | Output as JSON |

**Examples:**

```bash
# List all outdated packages
omg outdated

# Machine-readable output
omg outdated --json
```

---

### omg size

Show disk usage by packages.

```bash
omg size [OPTIONS]
```

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--tree <package>` | `-t` | Show dependency tree for package |
| `--limit <N>` | `-l` | Number of packages to show (default: 20) |

**Examples:**

```bash
# Show largest packages
omg size

# Show top 50 packages
omg size --limit 50

# Show dependency tree for a package
omg size --tree firefox
```

---

### omg blame

Show when and why a package was installed.

```bash
omg blame <package>
```

**Examples:**

```bash
# See installation history for a package
omg blame firefox
```

**Output includes:**

- Installation date/time
- Whether installed explicitly or as dependency
- Which package pulled it in (if dependency)
- Transaction ID

---

## 🔧 Runtime Management

### omg use

Install and activate a runtime version.

```bash
omg use <runtime> [version]
```

**Supported Runtimes:**

| Runtime | Aliases | Version Files |
| --------- | --------- | --------------- |
| `node` | `nodejs` | `.nvmrc`, `.node-version` |
| `bun` | `bunjs` | `.bun-version` |
| `python` | `python3` | `.python-version` |
| `go` | `golang` | `.go-version` |
| `rust` | `rustlang` | `rust-toolchain.toml` |
| `ruby` | | `.ruby-version` |
| `java` | | `.java-version` |
| `pi` | | `.tool-versions` |

Unsupported runtime names fail explicitly.

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

# Install and activate an exact Pi release
omg use pi 0.83.0
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

### omg hooks

Manage Git hooks for environment synchronization.

```bash
omg hooks <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `install [--force]` | Install Git hooks for environment synchronization |
| `uninstall` | Uninstall Git hooks |
| `status` | Show installed hooks status |
| `run <hook>` | Run a specific hook manually (pre-commit, post-checkout, post-merge) |

---

### omg workspace

Workspace management for monorepos.

```bash
omg workspace <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `init <name>` | Initialize a new workspace |
| `add <path> [--name]` | Add a project to the workspace |
| `remove <project>` | Remove a project from the workspace |
| `list` | List all projects in the workspace |
| `run <command> [-p] [--filter]` | Run a command across all projects |
| `diff [branch]` | Show environment diff across workspace vs a branch (default: main) |
| `check` | Check all project environments without changing them |
| `status` | Show workspace status |

---

### omg privacy

Manage your privacy settings and data (GDPR/CCPA).

```bash
omg privacy [SUBCOMMAND]
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `status` | Show local privacy settings and the authenticated account-management URL |
| `export [-o <file>]` | Export local OMG data |
| `opt-out` | Disable telemetry collection |
| `opt-in` | Re-enable telemetry collection |

Local exports include archived package history. Each source file is limited to
64 MiB. Larger files cause the export to fail rather than silently omitting or
truncating data. Streaming exports for larger archives are not supported.

---

## 🛡️ Security & Audit

### omg audit

Security audit suite with multiple subcommands.

```bash
omg audit [SUBCOMMAND]
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `scan` | Scan for vulnerabilities (default) |
| `sbom` | Generate CycloneDX 1.5 SBOM |
| `secrets` | Scan for leaked credentials |
| `log` | View audit log entries |
| `verify` | Verify audit log integrity |
| `policy` | Show security policy status |
| `slsa <pkg>` | Check SLSA provenance |
| `licenses` | Scan for software license compliance issues |
| `fix` | Auto-fix vulnerabilities by upgrading packages |
| `export` | Export compliance evidence for audit frameworks |
| `eol` | Check end-of-life status for installed runtimes |

**Options for `log`:**

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--limit` | `-l` | Number of entries to show (default: 20) |
| `--severity` | `-s` | Filter by severity (debug, info, warning, error, critical) |
| `--export` | `-e` | Export logs to a file (CSV or JSON) |

**Examples:**

```bash
# Vulnerability scan (default)
omg audit
omg audit scan

# Generate SBOM
omg audit sbom -o sbom.json

# Scan for secrets
omg audit secrets
omg audit secrets -p /path/to/project

# View audit log
omg audit log
omg audit log --limit 50
omg audit log --severity error

# Export audit logs
omg audit log --export audit.csv
omg audit log --export security_report.json

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
omg status [--fast]
```

| Option | Short | Description |
|--------|-------|-------------|
| `--fast` | `-f` | Use fast path (counts only, skips full dependency scan) |

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
omg doctor [OPTIONS]
```

| Option | Description |
| -------- | ------------- |
| `--network` | Test network connectivity to package mirrors |
| `--eol` | Check for end-of-life runtime versions |
| `--turbo` | Enable turbo mode check (zero-sudo package operations via Linux capabilities) |

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

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--watch` | `-w` | Watch mode: re-run task on file changes |
| `--parallel` | `-p` | Run multiple comma-separated tasks in parallel |
| `--using <ecosystem>` | `-u` | Ecosystem to use (e.g., node, rust, python, make) |
| `--all` | `-a` | Run task across all detected ecosystems |

**Supported Project Files:**

| File | Runtime | Example |
| ------ | --------- | --------- |
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
omg run test -- --verbose

# Watch mode - re-run on file changes
omg run test --watch

# Run multiple tasks in parallel
omg run build,test,lint --parallel
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
| ------- | ------------- |
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
| ------------ | ------------- |
| `install <name>` | Install a tool |
| `list` | List installed tools |
| `remove <name>` | Remove a tool |
| `update <name>` | Update a tool (or `all` to update everything) |
| `search <query>` | Search for tools in the registry |
| `registry` | Show all available tools grouped by category |

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

# Update all tools
omg tool update all

# Search for docker-related tools
omg tool search docker

# Browse all available tools
omg tool registry
```

**Tool Registry:**

OMG includes a curated registry of 60+ popular developer tools across categories:

- **search**: ripgrep, fd, fzf
- **files**: bat, eza
- **git**: delta, lazygit
- **system**: htop, btop, dust, duf, procs
- **dev**: hyperfine, tokei, just, watchexec
- **node**: yarn, pnpm, tsx, nodemon, prettier, eslint
- **rust**: cargo-watch, cargo-edit, cargo-nextest, bacon
- **python**: black, ruff, mypy, poetry
- **docker**: dive, lazydocker
- **deploy**: vercel, netlify-cli, wrangler

**Tool Resolution:**

1. Check the built-in registry for optimal source
2. Fall back to interactive selection if not in registry
3. Install to isolated `~/.local/share/omg/tools/`

---

### omg init

Interactive first-run setup wizard.

```bash
omg init [OPTIONS]
```

**Options:**

| Option | Description |
| -------- | ------------- |
| `--defaults` | Use defaults, no prompts |
| `--skip-shell` | Skip shell hook setup |
| `--skip-daemon` | Skip daemon setup |

**Examples:**

```bash
# Interactive setup
omg init

# Non-interactive with defaults
omg init --defaults

# Skip shell configuration
omg init --skip-shell
```

**Setup includes:**

1. Shell detection and hook installation
2. Daemon startup preference
3. Initial environment capture
4. Completion installation

---

### omg self-update

Update OMG to the latest version.

```bash
omg self-update [aliases: up]
```

**Features:**

- **Atomic Binary Replacement**: Replaces the current binary with the latest version from [GitHub Releases](https://github.com/PyRo1121/omg/releases).
- **Progress Tracking**: Real-time progress bar showing download speed and estimated time remaining.
- **Verification**: Automatically verifies the signature of the downloaded binary before installation.

**Examples:**

```bash
# Update OMG
omg self-update

# Using alias
omg up
```

---

### omg config

Get or set configuration values.

```bash
omg config [SUBCOMMAND]
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `get <key>` | Get a configuration value |
| `set <key> <value>` | Set a configuration value |
| `list` | List all configuration values |
| `validate` | Validate configuration file syntax and values |
| `reset [-y]` | Reset configuration to defaults |
| `path` | Show configuration file path |

**Examples:**

```bash
# List all configuration
omg config list

# Get a specific value
omg config get data_dir

# Set AUR build concurrency
omg config set aur.build_concurrency 8

# Disable telemetry
omg config set telemetry.enabled false
```

**Configuration keys:**

- `data_dir` — Data directory path (read-only via CLI)
- `socket` — Daemon socket path (read-only via CLI)
- `telemetry.enabled` — Enable/disable telemetry
- `aur.build_concurrency`, `aur.enable_ccache`, `aur.enable_sccache`, `aur.secure_makepkg`, `aur.makeflags` — AUR build tuning

---

## 📸 Environment & Snapshots

### omg snapshot

Create and restore environment snapshots.

```bash
omg snapshot <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `create` | Create a new snapshot |
| `list` | List all snapshots |
| `restore <id>` | Restore a snapshot |
| `delete <id>` | Delete a snapshot |

**Examples:**

```bash
# Create snapshot with message
omg snapshot create -m "Before major upgrade"

# List snapshots
omg snapshot list

# Preview restore
omg snapshot restore abc123 --dry-run

# Restore snapshot
omg snapshot restore abc123

# Delete old snapshot
omg snapshot delete abc123
```

Creation and deletion share a persistent `.index.lock` in the snapshots directory.
A competing mutation fails with a retry message. Both commands validate the index
before changing snapshot files. This does not make the snapshot file and index a
crash-atomic pair.

---

### omg diff

Compare two environment lock files.

```bash
omg diff [OPTIONS] <to>
```

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--from <file>` | `-f` | First file (default: current environment) |

**Examples:**

```bash
# Compare current env to a lock file
omg diff teammate-omg.lock

# Compare two lock files
omg diff --from old.lock new.lock
```

**Output shows:**

- Packages added
- Packages removed
- Version changes
- Runtime differences

---

## 🤝 Team Collaboration

### omg env

Manage environment lockfiles.

```bash
omg env <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
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
| ------------ | ------------- |
| `init <team-id>` | Initialize team workspace |
| `join <url>` | Join existing team |
| `status` | Show team sync status |
| `push` | Push local environment to team |
| `pull` | Pull team environment |
| `members` | List team members |
| `dashboard` | Interactive team TUI |
| `roles list` | List available roles and permissions |
| `golden-path` | Manage setup templates |
| `compliance` | Check compliance status |
| `activity` | View team activity stream |

**Examples:**

```bash
# Initialize team workspace
omg team init mycompany/frontend --name "Frontend Team"

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

# Create golden path template
omg team golden-path create frontend-setup --node 20 --packages "eslint prettier"

# Check compliance and export a report file
omg team compliance --export compliance-report.json

# View activity
omg team activity --days 30
```

**Roles:** admin, lead, developer, readonly

---

## 🐳 Container Management

### omg container

Docker/Podman integration.

```bash
omg container <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
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

## 🔄 CI/CD & Migration

### omg ci

Generate CI/CD configuration for your project.

```bash
omg ci <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `init <provider>` | Generate CI config (github, gitlab, circleci) |
| `validate` | Validate environment matches CI expectations |
| `cache` | Show recommended cache paths |

**Examples:**

```bash
# Generate GitHub Actions workflow
omg ci init github

# Generate GitLab CI config
omg ci init gitlab

# Validate CI environment
omg ci validate

# Get cache paths for CI
omg ci cache
```

**Generated config includes:**

- OMG installation step
- Cache configuration keyed to `omg.lock`
- Environment validation
- Task execution via `omg run`

---

### omg migrate

Cross-distro migration tools.

```bash
omg migrate <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
|------------|-------------|
| `export` | Export environment to portable manifest |
| `import <file>` | Import from manifest |

**Examples:**

```bash
# Export current environment
omg migrate export -o my-setup.json

# Preview import
omg migrate import my-setup.json --dry-run

# Import and install
omg migrate import my-setup.json
```

**Manifest includes:**

- All installed packages with versions
- Runtime versions
- Configuration settings
- Automatic package name mapping between distros

---

## 🏢 Enterprise Features

### omg fleet

Fleet management for multi-machine environments.

```bash
omg fleet <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
|------------|-------------|
| `status` | Show fleet health across machines |

**Examples:**

```bash
# View fleet status
omg fleet status
```

**Status shows:**

- Compliance percentage
- Machines by state (compliant, drifted, offline)
- Team breakdown

---

### omg enterprise

Enterprise administration features.

```bash
omg enterprise <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `reports` | Generate executive reports (JSON) |
| `policy show` | Show current policies |
| `audit-export` | Export compliance evidence |
| `license-scan` | Scan for license compliance |

**Examples:**

```bash
# Generate monthly report (reports are written as JSON)
omg enterprise reports --report-type monthly

# Export SOC2 compliance evidence
omg enterprise audit-export --framework soc2 --period 2025-Q4

# Scan for license issues and export CSV results
omg enterprise license-scan --export csv

# Show current policies
omg enterprise policy show
```

**Report types:** monthly, quarterly, custom
**Compliance frameworks:** soc2, iso27001, fedramp, hipaa, pci-dss

> Note: policy management beyond `policy show` and self-hosted registry
> management are not available in the CLI.

---

## 📜 History & Rollback

### omg history

View transaction history.

```bash
omg history [OPTIONS]
```

**Options:**

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--limit <N>` | `-l` | Number of entries (default: 20) |
| `--search <pkg>` | `-s` | Search for a specific package in history |
| `--type <type>` | `-t` | Filter by transaction type (install, remove, update, sync) |
| `--from <date>` | | Filter transactions from this date (YYYY-MM-DD) |
| `--to <date>` | | Filter transactions until this date (YYYY-MM-DD) |

**Examples:**

```bash
# View recent history
omg history

# View last 5 transactions
omg history --limit 5

# Search history for a package
omg history --search firefox
```

---

### omg rollback

Rollback to a previous state.

```bash
omg rollback [transaction-id] [-y]
```

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--yes` | `-y` | Auto-confirm without prompting (required in non-interactive mode) |

**Examples:**

```bash
# Interactive rollback (most recent transaction)
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
| ----- | -------- |
| `q` | Quit |
| `r` | Refresh |
| `/` | Search packages |
| `Tab` | Switch view |

---

### omg stats

Display usage statistics.

```bash
omg stats
```

---

### omg metrics

Show system metrics (Prometheus-style). Unix only.

```bash
omg metrics
```

---

### omg daemon-status

Show detailed daemon status.

```bash
omg daemon-status
```

---

### omg generate-man

Generate man pages for OMG commands.

```bash
omg generate-man [--output <dir>]
```

| Option | Short | Description |
|--------|-------|-------------|
| `--output <dir>` | `-o` | Output directory for man pages (default: ~/.local/share/man/man1) |

---

## 🔑 Dashboard account & daemon

### omg account

Optional link between this machine and the OMG dashboard. Linking attributes opted-in usage; every local command works without it.

`omg license`, `omg license check`, and `omg license pricing` were removed in 0.1.215.

```bash
# Removed
omg license
omg license check
omg license pricing

# Use instead
omg account status
omg account link <token>
omg account unlink
```

```bash
omg account <SUBCOMMAND>
```

**Subcommands:**

| Subcommand | Description |
| ------------ | ------------- |
| `link <token>` | Link this machine with a dashboard token |
| `status` | Show whether this machine is linked |
| `unlink` | Remove the local dashboard identity |

### omg daemon

Start the background daemon.

```bash
omg daemon
```

For direct daemon control:

```bash
omgd  # Run the daemon (it blocks in the foreground; use systemd or `omg daemon` to manage it)
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
| ------------ | ------------- | --------- |
| `status` | System status | 3ms |
| `ec` | Explicit count | &lt;1ms |
| `tc` | Total count | &lt;1ms |
| `uc` | Updates count | &lt;1ms |
| `oc` | Orphan count | &lt;1ms |
| `s <query>` | Search packages | daemon speed |
| `i <package>` | Package info | daemon speed |

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
| -------- | ------- | ------------- |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |
| `--verbose` | `-v` | Increase verbosity (-v, -vv, -vvv) |
| `--quiet` | `-q` | Suppress all output except errors |
| `--json` | | Output in JSON format (for scripting) |
| `--all-commands` | | Show all commands including advanced ones |

---

## 📚 See Also

- [Quick Start Guide](./quickstart.md)
- [Configuration](./configuration.md)
- [Runtime Management](./runtimes.md)
- [Security & Compliance](./security.md)
