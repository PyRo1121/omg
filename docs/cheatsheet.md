---
title: Cheat Sheet
sidebar_position: 99
description: Quick reference for all OMG commands
---

# OMG Cheat Sheet

**Quick reference for the most common OMG commands.** Print this page or bookmark it for fast access.

---

## 🚀 Installation & Setup

```bash
# Install OMG (Linux/macOS/Windows)
curl -fsSL https://pyro1121.com/install.sh | bash

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

## 📦 Package Management

### Search & Info
```bash
omg search <query>          # Search packages (6ms!)
omg search <query> -i       # Interactive selection
omg info <package>          # Package details
```

### Install & Remove
```bash
omg install <pkg>           # Install package
omg install <pkg1> <pkg2>   # Install multiple
omg install -y <pkg>        # Skip confirmation
omg remove <pkg>            # Remove package
omg remove -r <pkg>         # Remove + dependencies
```

### Update & Upgrade
```bash
omg update                  # Update all packages
omg update --check          # Check without installing
omg upgrade <pkg>           # Upgrade specific package
```

### Query
```bash
omg list                    # List installed packages
omg list --explicit         # Explicitly installed only
omg which <pkg>             # Check if installed
```

---

## 🔧 Runtime Management

### Install & Switch Versions
```bash
omg use node 20             # Install & use Node.js 20
omg use node lts            # Use latest LTS
omg use python 3.12         # Python 3.12
omg use rust stable         # Rust stable
omg use rust nightly        # Rust nightly
omg use go 1.21             # Go 1.21
omg use ruby 3.3            # Ruby 3.3
omg use java 21             # Java 21
omg use bun latest          # Bun latest
```

### Query Versions
```bash
omg which node              # Show active Node.js version
omg list node               # List installed versions
omg list node --available   # Show available versions
omg list                    # List all installed runtimes
```

### Version Files (Auto-Detection)
```bash
# Create version files (auto-detected by OMG)
echo "20.10.0" > .nvmrc                 # Node.js
echo "3.12.0" > .python-version         # Python
echo "stable" > rust-toolchain          # Rust
echo "1.21.0" > .go-version             # Go

# OMG auto-switches when you cd into directory
cd .  # Trigger auto-detect
```

---

## 🌍 Environment Management

### Lock & Sync
```bash
omg env capture             # Capture environment → omg.lock
omg env sync                # Sync from omg.lock
omg env check               # Check for drift
omg env diff                # Show differences
```

### Share with Team
```bash
omg env share               # Upload to GitHub Gist
omg env share --public      # Public Gist
omg env download <url>      # Download shared environment
```

---

## ▶️ Task Runner

```bash
omg run dev                 # Run dev script (auto-detects)
omg run build               # Run build script
omg run test                # Run tests
omg run <task>              # Run any task

# Project types supported:
# - package.json (npm/yarn/pnpm/bun)
# - Cargo.toml (Rust)
# - Makefile (Make)
# - pyproject.toml (Python)
# - go.mod (Go)
# - deno.json (Deno)
```

---

## 🔐 Security

### Vulnerability Scanning
```bash
omg audit                   # Scan for vulnerabilities
omg audit scan              # Full security scan
omg audit fix               # Auto-fix vulnerabilities
omg scan <package>          # Scan specific package
```

### SBOM Generation
```bash
omg sbom generate           # Generate SBOM
omg sbom --format json      # JSON format
omg sbom --format cyclonedx # CycloneDX format
```

### Audit Logs
```bash
omg audit log               # View audit log
omg audit log --tail 50     # Last 50 entries
omg audit export            # Export for compliance
```

---

## 🐳 Containers

```bash
omg container shell         # Dev shell with runtimes
omg container build         # Build container image
omg container init          # Generate Dockerfile
omg container run <cmd>     # Run command in container
```

---

## 👥 Team Features

### Team Dashboard
```bash
omg team init               # Initialize team
omg team join <token>       # Join team
omg team status             # Team health overview
omg team push               # Push environment update
omg team pull               # Pull team environment
```

### Fleet Management (Enterprise)
```bash
omg fleet status            # Fleet overview
omg fleet push              # Push to entire fleet
omg fleet remediate         # Auto-fix drift
omg fleet report            # Compliance report
```

---

## 🎨 Interactive TUI

```bash
omg dash                    # Launch interactive dashboard
# OR
omg                         # (same)

# Keyboard shortcuts in TUI:
# /          Search packages
# Enter      Select/Install
# Tab        Switch panels
# q          Quit
```

---

## ⚙️ Configuration

### Config Files
```bash
~/.config/omg/config.toml   # General settings
~/.config/omg/policy.toml   # Security policy
```

### Common Config Options
```toml
# ~/.config/omg/config.toml
shims_enabled = false       # Use PATH (faster)
auto_update = true          # Auto-update runtimes
default_shell = "zsh"       # Default shell

[aur]
build_concurrency = 8       # Parallel AUR builds

[security]
scan_on_install = true      # Scan on install
```

---

## 🛠️ Daemon

```bash
omgd                        # Start daemon (background)
omg daemon start            # Start daemon
omg daemon stop             # Stop daemon
omg daemon restart          # Restart daemon
omg daemon status           # Daemon status
```

---

## 🔍 Diagnostics

```bash
omg doctor                  # System health check
omg status                  # Quick status overview
omg version                 # Show version
omg --help                  # Show help
omg <cmd> --help            # Command help
```

---

## 🚀 Quick Workflows

### New Project Setup
```bash
# 1. Install OMG
curl -fsSL https://pyro1121.com/install.sh | bash

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
omg env sync                # Install everything from omg.lock
omg run dev                 # Start coding!
```

---

### CI/CD Setup
```yaml
# .github/workflows/ci.yml
- name: Install OMG
  run: curl -fsSL https://pyro1121.com/install.sh | bash

- name: Sync environment
  run: omg env sync

- name: Build
  run: omg run build
```

---

### Multi-Runtime Project
```bash
# Full-stack app (Node + Python + Rust)
omg use node 20
omg use python 3.12
omg use rust stable

# Create version files
echo "20.10.0" > .nvmrc
echo "3.12.0" > .python-version
echo "stable" > rust-toolchain

# Lock everything
omg env capture
```

---

## 🎯 Performance Tips

### Speed Up Searches
```bash
# Ensure daemon is running (6ms searches!)
omgd &

# Without daemon: ~100-150ms
# With daemon: ~6ms (22x faster!)
```

### Parallel Operations
```bash
# Install multiple packages at once
omg install ripgrep fd bat -y

# Update all packages in parallel
omg update
```

### Cache Management
```bash
# Clear cache (if needed)
rm -rf ~/.cache/omg

# Rebuild cache
omg status  # Automatically rebuilds
```

---

## 🐛 Troubleshooting

### Common Issues

**Shell hook not working:**
```bash
# Add to shell config
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
exec $SHELL
```

**Daemon not starting:**
```bash
# Check if running
omg daemon status

# Restart
omg daemon restart

# Check logs
journalctl -u omgd --no-pager -n 50
```

**PATH issues:**
```bash
# Add OMG to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
exec $SHELL
```

**Auto-detect not working:**
```bash
# Verify version file format
cat .nvmrc  # Should be: 20.10.0 (not: node 20.10.0)

# Force reload
cd .
```

**Runtime not switching:**
```bash
# Check hook is loaded
type omg  # Should show: "omg is a function"

# Re-source config
exec $SHELL
```

---

## 📊 Common Aliases

Add to your shell config (`~/.zshrc` or `~/.bashrc`):

```bash
alias oi='omg install'
alias os='omg search'
alias ou='omg use'
alias or='omg run'
alias oe='omg env'
alias oa='omg audit'
alias od='omg dash'
```

---

## 🔗 Quick Links

- **📚 Full Docs:** [pyro1121.com/docs](https://pyro1121.com/docs)
- **🚀 Quick Start:** [docs/quickstart.md](./quickstart.md)
- **💻 CLI Reference:** [docs/cli.md](./cli.md)
- **🔧 Configuration:** [docs/configuration.md](./configuration.md)
- **🐛 Troubleshooting:** [docs/troubleshooting.md](./troubleshooting.md)
- **❓ FAQ:** [docs/faq.md](./faq.md)

---

## 🎓 Learning Path

**Beginner (First 5 minutes):**
1. Install OMG → `curl -fsSL https://pyro1121.com/install.sh | bash`
2. Search packages → `omg search neovim`
3. Install package → `omg install ripgrep`
4. Install runtime → `omg use node 20`

**Intermediate (Day 1):**
1. Set up shell hook → `eval "$(omg hook zsh)"`
2. Create version files → `.nvmrc`, `.python-version`
3. Lock environment → `omg env capture`
4. Run tasks → `omg run dev`

**Advanced (Week 1):**
1. Team sync → `omg env share`
2. Security scanning → `omg audit`
3. Container integration → `omg container shell`
4. Custom configuration → Edit `~/.config/omg/config.toml`

**Expert (Ongoing):**
1. Fleet management → `omg fleet status`
2. Policy enforcement → Edit `~/.config/omg/policy.toml`
3. Compliance reports → `omg fleet report --standard SOC2`
4. Custom integrations → See [integrations.md](./integrations.md)

---

## 📏 Comparison with Traditional Tools

| Task | Traditional | OMG |
|------|-------------|-----|
| **Search** | `pacman -Ss firefox` (133ms) | `omg search firefox` (6ms) |
| **Install** | `pacman -S firefox` | `omg install firefox` |
| **Use Node 20** | `nvm install 20 && nvm use 20` | `omg use node 20` |
| **Use Python 3.12** | `pyenv install 3.12 && pyenv global 3.12` | `omg use python 3.12` |
| **Lock env** | Manual docs, multiple files | `omg env capture` |
| **Sync env** | Manual setup (hours) | `omg env sync` (minutes) |

**Speed improvement:** 22x faster package searches, 10x faster runtime switching

---

## 💡 Pro Tips

1. **Use daemon for speed** - Start `omgd` in background for 6ms searches
2. **Version files = auto-switch** - Create `.nvmrc`, `.python-version` for automatic version switching
3. **Lock early, lock often** - Capture environment after every major change
4. **Parallel installs** - Install multiple packages at once: `omg install pkg1 pkg2 pkg3`
5. **Interactive mode** - Use `omg search <query> -i` for fuzzy selection
6. **Shell aliases** - Create aliases for common commands (see above)
7. **Team sync** - Use `omg env share` to share environment via GitHub Gist
8. **Security first** - Enable `scan_on_install = true` in config

---

**Print this page or save as PDF for offline reference!**

For detailed explanations, see the [Full Documentation](./index.md).
