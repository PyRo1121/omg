---
title: Migrating from nvm
sidebar_label: From nvm
sidebar_position: 2
description: Command mapping and migration guide from nvm to OMG
---

# Migrating from nvm

This guide helps nvm users transition to OMG's blazing-fast Node.js version management.

## Why Migrate?

| Feature | nvm | OMG |
|---------|-----|-----|
| Version Switch | 100-200ms | **1-2ms (100x faster)** |
| Shell Startup | 100-500ms | **&lt;10ms** |
| Implementation | Bash script | Pure Rust binary |
| Other Runtimes | ❌ | ✅ Python, Go, Rust, Ruby, Java, Bun |
| Package Management | ❌ | ✅ pacman, AUR, apt |

## Command Mapping

| nvm | OMG | Notes |
|-----|-----|-------|
| `nvm install 20` | `omg use node 20` | Install + activate |
| `nvm use 20` | `omg use node 20` | Activate (installs if needed) |
| `nvm ls` | `omg list node` | List installed versions |
| `nvm ls-remote` | `omg list node --available` | List available versions |
| `nvm current` | `omg which node` | Show active version |
| `nvm uninstall 18` | `omg use node --uninstall 18` | Remove version |
| `nvm alias default 20` | `omg use node 20` | Set default |

## Shell Configuration

### Remove nvm

Remove from `~/.zshrc` or `~/.bashrc`:
```bash
# Remove these lines:
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"
```

### Add OMG

Add to `~/.zshrc`:
```bash
# OMG shell integration (adds &lt;10ms to startup)
eval "$(omg hook zsh)"
```

Or for bash (`~/.bashrc`):
```bash
eval "$(omg hook bash)"
```

## Version File Support

OMG automatically detects and respects your existing version files:

| File | Supported |
|------|-----------|
| `.nvmrc` | ✅ |
| `.node-version` | ✅ |
| `.tool-versions` | ✅ |
| `package.json` engines | ✅ |

### Automatic Switching

When you `cd` into a directory with `.nvmrc`:

```bash
cd my-project/
# OMG automatically switches to the version in .nvmrc
node --version  # v20.10.0
```

## Data Migration

### Existing Node versions

Your nvm-installed versions are in `~/.nvm/versions/node/`. OMG uses `~/.local/share/omg/runtimes/node/`.

You can either:
1. **Re-install** (recommended): `omg use node 20` — fast parallel download
2. **Symlink**: Link existing versions (advanced)

### Global packages

Global npm packages are per-version. After switching:
```bash
npm install -g <your-packages>
```

## Quick Start

```bash
# 1. Install OMG hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc

# 2. Install Node.js
omg use node 20

# 3. Verify
node --version
npm --version

# 4. (Optional) Remove nvm
rm -rf ~/.nvm
```

## Performance Comparison

```bash
# nvm version switch
time nvm use 20
# real    0m0.150s

# OMG version switch
time omg use node 20
# real    0m0.002s
```

**75x faster version switching!**

## Bonus: Multi-Runtime

With OMG, you also get:

```bash
# Python
omg use python 3.12

# Go
omg use go 1.22

# Rust
omg use rust stable

# All managed the same way!
```

## Troubleshooting

### Node not found after switching
Ensure the hook is loaded:
```bash
eval "$(omg hook zsh)"
```

### Wrong version active
Check for conflicting version files:
```bash
omg which node
```

### .nvmrc not detected
OMG scans parent directories. Ensure the file exists:
```bash
cat .nvmrc
```

## Next Steps

- [Runtime Management](/runtimes) — Full runtime documentation
- [Shell Integration](/cli#shell-integration) — Hook configuration
- [Workflows](/workflows) — Team environment sync
