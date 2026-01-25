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

## Command Mapping

| nvm | OMG | Notes |
|-----|-----|-------|
| `nvm install 20` | `omg use node 20` | Install + activate |
| `nvm use 20` | `omg use node 20` | Activate (installs if needed) |
| `nvm ls` | `omg list node` | List installed versions |
| `nvm ls-remote` | `omg list node --available` | List available versions |
| `nvm current` | `omg which node` | Show active version |
| `nvm uninstall 18` | `omg use node --uninstall 18` | Remove version |

## Shell Configuration

### Remove nvm

Remove from `~/.zshrc` or `~/.bashrc`:
```bash
# Remove these lines:
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
```

### Add OMG

```bash
# Add to ~/.zshrc
eval "$(omg hook zsh)"
```

## Version File Support

OMG automatically detects:
- `.nvmrc`
- `.node-version`
- `.tool-versions`
- `package.json` engines

## Next Steps

- [Runtime Management](/runtimes) — Full runtime documentation
- [CLI Reference](/cli) — Full command documentation
