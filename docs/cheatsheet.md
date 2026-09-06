---
title: Cheat Sheet
sidebar_position: 99
description: Quick reference for all OMG commands
---

# OMG Cheat Sheet

**Quick reference for the most common OMG commands.** Print this page or bookmark it for fast access.

Every command below matches the current `omg --help` output. When in doubt, run `omg <command> --help`.

---

## 🚀 Installation & Setup

```bash
# Install OMG (Linux/macOS, including Linux inside WSL)
curl -fsSL https://omg.latham.cloud/install.sh | bash

# Shell integration (auto-switch runtimes)
eval "$(omg hook zsh)"      # Zsh
eval "$(omg hook bash)"     # Bash
omg hook fish | source      # Fish

# Verify installation
omg --version
omg status
omg doctor
```

---

## 🌍 Global Options

These flags work with any command:

| Option | Short | Description |
| -------- | ------- | ------------- |
| `--verbose` | `-v` | Increase verbosity (`-v`, `-vv`, `-vvv`) |
| `--quiet` | `-q` | Suppress all output except errors |
| `--json` | | Output in JSON format (for scripting) |
| `--all-commands` | | Show all commands including advanced ones |

---

## 📦 Package Management

### Search & Info

```bash
omg search <query>              # Search official repos + AUR (5-11ms with daemon)
omg search <query> -d           # Detailed source metadata (votes, popularity)
omg search <query> --no-aur     # Official repositories only
omg search <query> -l 10        # Limit results (default: 50)
omg info <package>              # Package details
```

### Install & Remove

```bash
omg install <pkg> [<pkg>...]    # Install packages (system + AUR auto-detected)
omg install -y <pkg>            # Skip confirmation
omg install --dry-run <pkg>     # Preview without installing
omg remove <pkg>                # Remove package
omg remove -r <pkg>             # Also remove unused dependencies
omg remove --dry-run <pkg>      # Preview without removing
```

### Update & Sync

```bash
omg update                      # Update all packages (official + AUR)
omg update --check              # Check for updates without installing
omg update --fast               # Sync + upgrade in one step (no preview)
omg update -T                   # Turbo mode: cached data, parallel extraction
omg update --dry-run            # Preview what would be updated
omg sync                        # Sync package databases only
omg outdated                    # Show packages with available updates
```

### Query & Inspect

```bash
omg explicit                    # List explicitly installed packages
omg explicit -c                 # Print only the count
omg why <package>               # Show why a package is installed
omg why <package> -r            # Show reverse dependencies
omg size                        # Disk usage by package (top 20)
omg size -t firefox             # Dependency tree for a package
omg blame <package>             # When and why a package was installed
```

### Clean

```bash
omg clean -o                    # Remove orphaned packages
omg clean -c                    # Clear package cache
omg clean --aur                 # Clear AUR build directories
omg clean -a                    # Everything above (--all)
omg clean --dry-run -a          # Preview what would be cleaned
```

### Snapshots & Diff

```bash
omg snapshot create -m "Before major upgrade"
omg snapshot list
omg snapshot restore <id> --dry-run   # Preview restore
omg snapshot restore <id>
omg snapshot delete <id>

omg diff <lock-file>                  # Compare current env to a lock file
omg diff --from old.lock new.lock     # Compare two lock files
```

---

## 🔧 Runtime Management

### Install & Switch Versions

```bash
omg use node 20                 # Install & use Node.js 20
omg use node lts                # Use latest LTS (or omit version to detect from file)
omg use python 3.12
omg use rust stable
omg use rust nightly
omg use go 1.21
omg use ruby 3.3
omg use java 21
omg use bun latest
```

Native runtimes: `node`, `python`, `go`, `rust`, `ruby`, `java`, `bun`, and `pi`. Unsupported runtimes fail explicitly.

### Query Versions

```bash
omg which node                  # Which version would be used right now
omg list                        # All installed runtime versions
omg list node                   # Installed Node.js versions
omg list node -a                # Available versions for download
```

### Version Files (Auto-Detection)

Create a version file and OMG switches automatically via the shell hook:

```bash
echo "20.10.0" > .nvmrc             # Node.js (.node-version also works)
echo "3.12.0" > .python-version     # Python
echo "1.21.0" > .go-version         # Go
echo "stable" > rust-toolchain.toml # Rust ([toolchain] channel = "stable")
echo "3.3.0" > .ruby-version        # Ruby
echo "21" > .java-version           # Java
echo "1.0.25" > .bun-version        # Bun

cd .    # Trigger auto-detect after creating a file
```

---

## 🏃 Task Runner

```bash
omg run dev                     # Run dev script (auto-detects project type)
omg run build                   # Run build script
omg run test -- --verbose       # Pass arguments after --
omg run build,test -p           # Run multiple tasks in parallel (comma-separated)
omg run build -w                # Watch mode: re-run on file changes
omg run build -u rust           # Force an ecosystem
omg run build -a                # Run across all detected ecosystems
```

Detected project files include `package.json`, `deno.json`, `Cargo.toml`, `Makefile`,
`pyproject.toml`, `Pipfile`, `composer.json`, `pom.xml`, `build.gradle`, and more.

---

## 🔐 Security

### Vulnerability Scanning

```bash
omg audit                       # Scan for vulnerabilities (default subcommand)
omg audit scan                  # Same, explicitly
omg audit fix --dry-run         # Preview vulnerability fixes
omg audit fix -y                # Auto-fix by upgrading packages
omg audit eol                   # Check end-of-life runtimes
```

### SBOM & Secrets

```bash
omg audit sbom                  # Generate CycloneDX 1.5 SBOM
omg audit sbom -o sbom.json     # Write to a specific file
omg audit secrets               # Scan current directory for leaked credentials
omg audit secrets -p ./src      # Scan a specific path
```

### Audit Logs & Policy

```bash
omg audit log                   # View audit log entries (default: last 20)
omg audit log -l 50             # More entries
omg audit log -s error          # Filter by severity
omg audit verify                # Check audit log for tampering
omg audit policy                # Show security policy status
omg audit slsa <package-file>   # Check SLSA provenance
```

---

## 🤝 Environment & Team

```bash
omg env capture                 # Capture environment -> omg.lock
omg env check                   # Check for drift against omg.lock
omg env share                   # Upload environment to a secret GitHub Gist
omg env share --public          # Public Gist instead
omg env share -d "Team env"     # Custom Gist description
export GITHUB_TOKEN=...         # Required for share/sync
omg env sync https://gist.github.com/user/abc123   # Restore from a shared Gist
```

### Team Workspace

```bash
omg team init mycompany/frontend --name "Frontend Team"
omg team join <url>             # Join a team by remote URL
omg team status                 # Team sync status
omg team push                   # Push local environment to team lock
omg team pull                   # Pull team lock and check drift
omg team members                # List members and sync state
omg team dashboard              # Interactive team TUI
omg team golden-path create frontend-setup --node 20 --packages "eslint prettier"
omg team golden-path list
omg team compliance             # Check compliance status
omg team activity --days 30     # Activity stream
```

### Snapshots of Environments Elsewhere

```bash
omg migrate export -o my-setup.json        # Portable manifest of this machine
omg migrate import my-setup.json --dry-run # Preview import (with package mapping)
omg migrate import my-setup.json           # Import and install
```

---

## 🐳 Containers

```bash
omg container status            # Docker/Podman runtime status
omg container shell             # Dev shell with project mounted
omg container run alpine -- echo "hello"
omg container run -i ubuntu -- bash   # Interactive (-i)
omg container build -t myapp    # Build image (default tag omg-dev:latest)
omg container init              # Generate a Dockerfile from detected runtimes
omg container init -b node:20   # With a specific base image
omg container list              # Running containers
omg container images            # Container images
omg container pull node:20      # Pull an image
omg container stop mycontainer  # Stop a container by name/ID
omg container exec mycontainer -- ls -la   # Execute in a running container
```

---

## ⚙️ Configuration

```bash
omg config                      # Interactive/config command root
omg config list                 # List all configuration values
omg config get telemetry.enabled
omg config set telemetry.enabled false
omg config validate             # Validate config file syntax and values
omg config path                 # Show configuration file path
omg config reset -y             # Reset to defaults
```

### Config Files

```bash
~/.config/omg/config.toml   # General settings
~/.config/omg/policy.toml   # Security policy
```

### Common Config Options (`~/.config/omg/config.toml`)

```toml
telemetry_enabled = false       # Runtime telemetry is opt-in

[aur]
build_concurrency = 8           # Parallel AUR builds
review_pkgbuild = true          # Require PKGBUILD review before building
```

See [Configuration](./configuration.md) for the complete reference — every key above is read by OMG; anything not listed there is ignored.

### Security Policy (`~/.config/omg/policy.toml`)

```toml
minimum_grade = "Community"     # Risk | Community | Verified | Locked
allow_aur = true                # Set false to forbid AUR packages
require_pgp = false             # Require signatures for everything
allowed_licenses = ["MIT", "Apache-2.0"]
banned_packages = []
```

---

## 🛠️ Daemon

```bash
omgd                            # Start the daemon (blocks in the foreground)
omgd --socket /path/to/sock     # Custom socket path
omg daemon-status               # Show detailed daemon status
omg daemon                      # Start the daemon from the CLI
omg-fast status                 # Instant status from the daemon's snapshot
```

There is no `omg daemon start/stop/restart` subcommand — the daemon is started with
`omg daemon`/`omgd` and stopped by killing the process (or your service manager).
To run it under systemd, see [Configuration](./configuration.md).

### omg-fast (shell-prompt queries)

```bash
omg-fast ec        # Explicit count
omg-fast tc        # Total count
omg-fast uc        # Updates count
omg-fast oc        # Orphan count
omg-fast status    # Full status
omg-fast s vim     # Search
omg-fast i vim     # Package info
```

---

## 🔍 Diagnostics

```bash
omg doctor                      # System health check
omg doctor --network            # Also test mirror connectivity
omg doctor --eol                # Also check end-of-life runtimes
omg doctor --turbo              # Prime sudo credentials; remove legacy file capabilities
omg status                      # Quick status overview
omg status --fast               # Counts only, skip full dependency scan
omg --help                      # Show help
omg <cmd> --help                # Command help
```

---

## 📜 History & Rollback

```bash
omg history                     # Recent transactions (default: 20)
omg history -l 5                # Last 5 entries
omg history -s firefox          # Search history for a package
omg history -t install          # Filter by type (install/remove/update/sync)
omg history --from 2026-01-01 --to 2026-02-01
omg rollback                    # Roll back most recent transaction
omg rollback <transaction-id>   # Roll back to a specific transaction
omg rollback <id> -y            # Auto-confirm
```

---

## 👥 Other Commands

```bash
omg new rust my-cli             # Scaffold projects: rust|react|node|python|go
omg tool install ripgrep        # Cross-ecosystem dev tools
omg tool list                   # Installed tools
omg tool search docker          # Search the tool registry
omg tool registry               # Browse all registry tools
omg tool update all             # Update every installed tool
omg tool remove ripgrep         # Remove a tool
omg ci init github              # Generate CI config (github|gitlab|circleci)
omg ci validate                 # Validate environment vs CI expectations
omg ci cache                    # Recommended cache paths
omg workspace init my-monorepo  # Workspace management (see omg workspace --help)
omg hooks install               # Git hooks for env sync (see omg hooks --help)
omg privacy status              # Privacy settings & data (GDPR/CCPA)
omg account status              # Optional dashboard account (replaces `omg license`)
omg fleet status                # Enterprise fleet status
omg stats                       # Usage statistics
omg metrics                     # Prometheus-style metrics (Unix)
omg dash                        # Interactive TUI dashboard (alias: omg d)
omg self-update                 # Update OMG (alias: omg up)
omg generate-man                # Generate man pages
```

---

## 🎨 Interactive TUI

```bash
omg dash                    # Launch interactive dashboard
```

Keyboard shortcuts in the TUI:

| Key | Action |
| ----- | -------- |
| `/` | Search packages |
| `r` | Refresh |
| `Tab` | Switch views/panels |
| `q` | Quit |

---

## 🚀 Quick Workflows

### New Project Setup

```bash
# 1. Install OMG
curl -fsSL https://omg.latham.cloud/install.sh | bash

# 2. Set up shell
eval "$(omg hook zsh)"

# 3. Install runtimes
omg use node 20
omg use python 3.12

# 4. Lock environment
omg env capture

# 5. Share with team
git add omg.lock && git commit -m "Lock environment"
```

---

### Team Onboarding

```bash
# New developer:
git clone <repo>
cd <repo>
omg env check          # Compare against the committed omg.lock
omg use node 20        # Install the pinned runtimes
omg install ripgrep fd # Install pinned packages
omg run dev            # Start coding!
# Or restore directly from a teammate's shared Gist:
# omg env sync https://gist.github.com/user/abc123
```

---

### CI/CD Setup

```yaml
# .github/workflows/ci.yml
- name: Install OMG
  run: curl -fsSL https://omg.latham.cloud/install.sh | bash

- name: Check environment matches omg.lock
  run: omg env check

- name: Build
  run: omg run build
```

---

## 🎯 Performance Tips

### Speed Up Searches

```bash
# Ensure daemon is running (5-11ms searches!)
omg daemon

# Without daemon: ~100-150ms direct queries
# With daemon: ~5-11ms (12-24x faster!)
```

### Parallel Operations

```bash
# Install multiple packages at once
omg install ripgrep fd bat -y

# Update all packages
omg update
```

---

## 🐛 Troubleshooting

**Shell hook not working:**

```bash
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
exec $SHELL
```

**Daemon not running:**

```bash
omg daemon-status          # Is it up?
omgd                       # Run it in the foreground to see errors
```

**PATH issues:**

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
exec $SHELL
```

**Auto-detect not working:**

```bash
cat .nvmrc                 # Should contain just the version: 20.10.0
cd .                       # Force re-detect
```

More help: [Troubleshooting Guide](./troubleshooting.md).

---

## 🔗 Quick Links

- **📚 Full Docs:** [GitHub docs](https://github.com/PyRo1121/omg/tree/main/docs)
- **🚀 Quick Start:** [docs/quickstart.md](./quickstart.md)
- **💻 CLI Reference:** [docs/cli.md](./cli.md)
- **🔧 Configuration:** [docs/configuration.md](./configuration.md)
- **🐛 Troubleshooting:** [docs/troubleshooting.md](./troubleshooting.md)
- **❓ FAQ:** [docs/faq.md](./faq.md)

---

## 💡 Pro Tips

1. **Use the daemon for speed** — start it with `omg daemon` for 5-11ms searches.
2. **Version files = auto-switch** — create `.nvmrc`, `.python-version` etc. and let the shell hook switch for you.
3. **Lock early, lock often** — run `omg env capture` after every major change.
4. **Preview before acting** — `--dry-run` exists on `install`, `remove`, `update`, `clean`, `snapshot restore`, `migrate import`, and `audit fix`.
5. **JSON output for scripts** — global `--json` works on any command.
6. **Security first** — review `omg audit policy` and tune `~/.config/omg/policy.toml`.
7. **Share environments via Gist** — `omg env share`, then teammates run `omg env sync <gist-url>`.

---

**Print this page or save as PDF for offline reference!**

For detailed explanations, see the [Full Documentation](./index.md).
