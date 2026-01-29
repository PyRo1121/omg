---
title: CLI Reference
description: Complete CLI reference for OMG package manager. Documentation for search, install, remove, update, use, list, run, audit, and all other commands with examples and options.
---

Complete Command Reference for OMG

This guide documents every OMG command with detailed explanations, examples, and use cases. Commands are organized by category.

---

## Command Overview

| Category | Commands |
|----------|----------|
| **Package Management** | `search`, `install`, `remove`, `update`, `info`, `clean`, `explicit`, `sync`, `why`, `outdated`, `pin`, `size`, `blame` |
| **Runtime Management** | `use`, `list`, `which` |
| **Shell Integration** | `hook`, `completions` |
| **Security & Audit** | `audit`, `status`, `doctor` |
| **Task Runner** | `run` |
| **Project Management** | `new`, `tool`, `init`, `self-update` |
| **Environment & Snapshots** | `env`, `snapshot`, `diff` |
| **Team Collaboration** | `team` |
| **Container Management** | `container` |
| **CI/CD & Migration** | `ci`, `migrate` |
| **History & Rollback** | `history`, `rollback` |
| **Dashboard** | `dash`, `stats` |
| **Configuration** | `config`, `daemon`, `license` |
| **Enterprise** | `fleet`, `enterprise` |

---

## Package Management

### omg search

Search for packages across official repositories and AUR.

```bash
omg search <query> [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--detailed` | `-d` | Show detailed AUR info (votes, popularity) |
| `--interactive` | `-i` | Interactive mode - select packages to install |

**Examples:**
```bash
# Basic search
omg search firefox

# Interactive search (select to install)
omg search browser -i

# Detailed search with AUR votes/popularity
omg search spotify -d
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
| `--yes` | `-y` | Skip confirmation prompt |

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

---

## Runtime Management

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
```

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
```

---

## Task Runner

### omg run

Run project tasks with automatic runtime detection.

```bash
omg run <task> [-- <args...>] [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--watch` | `-w` | Watch mode: re-run task on file changes |
| `--parallel` | `-p` | Run multiple comma-separated tasks in parallel |

**Supported Project Files:**
| File | Runtime | Example |
|------|---------|---------|
| `package.json` | npm/yarn/pnpm/bun | `omg run dev` -> `npm run dev` |
| `deno.json` | deno | `omg run dev` -> `deno task dev` |
| `Cargo.toml` | cargo | `omg run test` -> `cargo test` |
| `Makefile` | make | `omg run build` -> `make build` |
| `pyproject.toml` | poetry | `omg run serve` -> `poetry run serve` |

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

---

## Security & Audit

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

**Examples:**
```bash
# Vulnerability scan (default)
omg audit

# Generate SBOM with vulnerabilities
omg audit sbom --vulns
omg audit sbom -o sbom.json

# Scan for secrets
omg audit secrets

# View audit log
omg audit log
omg audit log --limit 50
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

## Global Options

These options work with all commands:

| Option | Short | Description |
|--------|-------|-------------|
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

---

## See Also

- [Quick Start Guide](/getting-started/quickstart)
- [Configuration](/configuration/settings)
- [Runtime Management](/runtimes/overview)
