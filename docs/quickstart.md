# Quick Start Guide

Get up and running with OMG in under 5 minutes.

---

## Install OMG

### One-Line Install (Recommended)

```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

This installs the `omg`, `omgd`, and `omg-fast` binaries to `~/.local/bin/`.

### Build from Source

```bash
git clone https://github.com/PyRo1121/omg.git
cd omg
cargo build --release
cp target/release/{omg,omgd,omg-fast} ~/.local/bin/
```

### From AUR (Arch Linux)

```bash
yay -S omg-bin
```

---

## Set Up Your Shell

Add the OMG hook to your shell config. This enables automatic runtime version switching when you enter project directories.

**Zsh** — Add to `~/.zshrc`:
```bash
eval "$(omg hook zsh)"
```

**Bash** — Add to `~/.bashrc`:
```bash
eval "$(omg hook bash)"
```

**Fish** — Add to `~/.config/fish/config.fish`:
```fish
omg hook fish | source
```

Then restart your shell:
```bash
exec $SHELL
```

---

## Verify It Works

```bash
omg --version    # Should print version
omg status       # Shows system overview
omg doctor       # Checks everything is configured correctly
```

---

## Your First 60 Seconds with OMG

### Search for a Package

```bash
omg search neovim
```

Notice how fast that was? With the daemon running, searches return in ~6ms.

### Install Something

```bash
omg install neovim
```

OMG automatically detects whether a package is in the official repos or AUR and handles it appropriately.

### Switch Node.js Versions

```bash
omg use node 20
```

If Node.js 20 isn't installed, OMG downloads and installs it automatically. Then it sets it as your active version.

### Check What's Active

```bash
omg which node     # Shows active Node.js version
omg list node      # Shows all installed Node.js versions
```

### Run a Project

Navigate to any project directory and:

```bash
omg run dev
```

OMG detects your project type (package.json, Cargo.toml, Makefile, etc.) and runs the appropriate command with the correct runtime version.

---

## Common Workflows

### Managing System Packages

```bash
omg search <query>        # Find packages
omg search <query> -i     # Interactive mode — select packages to install
omg install <packages>    # Install packages
omg remove <package>      # Remove a package
omg remove <package> -r   # Remove with unused dependencies
omg update                # Update all packages
omg update --check        # Check for updates without installing
```

### Managing Language Runtimes

```bash
# Node.js
omg use node 20           # Install and use Node.js 20
omg use node lts          # Use latest LTS version

# Python
omg use python 3.12       # Install and use Python 3.12

# Rust
omg use rust stable       # Use stable Rust
omg use rust nightly      # Use nightly Rust

# Others
omg use go 1.22           # Go
omg use ruby 3.3          # Ruby
omg use java 21           # Java
omg use bun 1.0           # Bun

# List what's installed
omg list                  # All runtimes
omg list node             # Just Node.js versions
omg list node --available # Versions available for download
```

### Sharing Your Environment

```bash
# Capture your current environment to a lockfile
omg env capture

# Check if your environment matches the lockfile
omg env check

# Share your environment (uploads to GitHub Gist)
export GITHUB_TOKEN=your_token
omg env share

# Sync someone else's environment
omg env sync https://gist.github.com/user/abc123
```

### Running Security Checks

```bash
omg audit                 # Scan for vulnerabilities
omg audit sbom            # Generate software bill of materials
omg audit secrets         # Scan for leaked credentials
```

---

## Enable the Daemon (Recommended)

The daemon keeps a package index in memory, making searches 22x faster. Start it with:

```bash
omg daemon
```

The daemon runs in the background. To have it start automatically, you can:

1. **Add to shell init** — The shell hook can start it automatically
2. **Use systemd** — Create a user service (see [Configuration](./configuration.md))
3. **Start manually** — Run `omg daemon` when you need it

Without the daemon, OMG still works — it just falls back to direct package manager queries.

---

## Project Setup

When you enter a project directory, OMG automatically detects version files and switches runtimes:

| File | Runtime |
|------|---------|
| `.nvmrc` or `.node-version` | Node.js |
| `.python-version` | Python |
| `.ruby-version` | Ruby |
| `.go-version` | Go |
| `.java-version` | Java |
| `rust-toolchain.toml` | Rust |
| `.tool-versions` | Any (mise format) |

Create a version file in your project:

```bash
echo "20.10.0" > .nvmrc
```

Now whenever you `cd` into this directory, OMG automatically switches to Node.js 20.10.0.

---

## Interactive Dashboard

Launch the full-screen dashboard:

```bash
omg dash
```

Navigate with:
- `Tab` — Switch between views
- `r` — Refresh
- `q` — Quit

The dashboard shows packages, runtimes, security alerts, and system activity in real time.

---

## Getting Help

```bash
omg --help              # General help
omg <command> --help    # Help for specific command
omg doctor              # Diagnose issues
```

---

## What's Next?

Now that you're set up:

- **[CLI Reference](./cli.md)** — Every command explained in detail
- **[Runtime Management](./runtimes.md)** — Deep dive into managing language runtimes
- **[Team Collaboration](./team.md)** — Share environments with your team
- **[Security](./security.md)** — Vulnerability scanning and compliance

---

## Quick Reference

| Task | Command |
|------|---------|
| Search packages | `omg search <query>` |
| Install package | `omg install <package>` |
| Remove package | `omg remove <package>` |
| Update all | `omg update` |
| Use runtime | `omg use <runtime> <version>` |
| List runtimes | `omg list` |
| Run task | `omg run <task>` |
| System status | `omg status` |
| Security scan | `omg audit` |
| Dashboard | `omg dash` |

---

**Having trouble?** Run `omg doctor` or check the [Troubleshooting Guide](./troubleshooting.md).
