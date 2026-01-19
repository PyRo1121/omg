---
title: Migrating from pyenv
sidebar_label: From pyenv
sidebar_position: 3
description: Command mapping and migration guide from pyenv to OMG
---

# Migrating from pyenv

This guide helps pyenv users transition to OMG's unified runtime management.

## Why Migrate?

| Feature | pyenv | OMG |
|---------|-------|-----|
| Version Switch | 50-100ms | **1-2ms (50x faster)** |
| Shell Startup | 50-200ms | **&lt;10ms** |
| Implementation | Bash + shims | Pure Rust binary |
| Other Runtimes | ❌ | ✅ Node, Go, Rust, Ruby, Java, Bun |

## Command Mapping

| pyenv | OMG | Notes |
|-------|-----|-------|
| `pyenv install 3.12` | `omg use python 3.12` | Install + activate |
| `pyenv global 3.12` | `omg use python 3.12` | Set global version |
| `pyenv versions` | `omg list python` | List installed |
| `pyenv install --list` | `omg list python --available` | List available |
| `pyenv which python` | `omg which python` | Show active path |

## Shell Configuration

### Remove pyenv

Remove from `~/.zshrc`:
```bash
# Remove these lines:
export PYENV_ROOT="$HOME/.pyenv"
eval "$(pyenv init -)"
```

### Add OMG

```bash
eval "$(omg hook zsh)"
```

## Version File Support

OMG respects:
- `.python-version`
- `.tool-versions`

## Next Steps

- [Runtime Management](/runtimes) — Full runtime documentation
- [Configuration](/configuration) — Customize paths and behavior
