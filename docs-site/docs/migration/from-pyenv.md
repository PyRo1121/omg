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
| Shell Startup | 50-200ms | **<10ms** |
| Implementation | Bash + shims | Pure Rust binary |
| Other Runtimes | ❌ | ✅ Node, Go, Rust, Ruby, Java, Bun |
| Package Management | ❌ | ✅ pacman, AUR, apt |

## Command Mapping

| pyenv | OMG | Notes |
|-------|-----|-------|
| `pyenv install 3.12` | `omg use python 3.12` | Install + activate |
| `pyenv global 3.12` | `omg use python 3.12` | Set global version |
| `pyenv local 3.12` | Creates `.python-version` | OMG respects this file |
| `pyenv versions` | `omg list python` | List installed |
| `pyenv install --list` | `omg list python --available` | List available |
| `pyenv which python` | `omg which python` | Show active path |
| `pyenv uninstall 3.11` | `omg use python --uninstall 3.11` | Remove version |

## Shell Configuration

### Remove pyenv

Remove from `~/.zshrc` or `~/.bashrc`:
```bash
# Remove these lines:
export PYENV_ROOT="$HOME/.pyenv"
command -v pyenv >/dev/null || export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"
```

### Add OMG

Add to `~/.zshrc`:
```bash
# OMG shell integration
eval "$(omg hook zsh)"
```

## Version File Support

OMG respects pyenv's version files:

| File | Supported |
|------|-----------|
| `.python-version` | ✅ |
| `.tool-versions` | ✅ |

### Automatic Switching

```bash
cd my-project/
# OMG reads .python-version automatically
python --version  # Python 3.12.0
```

## Quick Start

```bash
# 1. Install OMG hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc

# 2. Install Python
omg use python 3.12

# 3. Verify
python --version
pip --version

# 4. (Optional) Remove pyenv
rm -rf ~/.pyenv
```

## Virtual Environments

OMG manages Python versions. For virtual environments, continue using:

```bash
# Create venv with OMG-managed Python
omg use python 3.12
python -m venv .venv
source .venv/bin/activate
```

Or use your preferred tool (poetry, pipenv, uv, etc.).

## Data Migration

### Existing Python versions

pyenv versions are in `~/.pyenv/versions/`. OMG uses `~/.local/share/omg/runtimes/python/`.

Recommended: Re-install with OMG:
```bash
omg use python 3.12
omg use python 3.11
```

### pip packages

Global pip packages are per-version. After installing:
```bash
pip install <your-packages>
```

## pyenv-virtualenv Users

If using pyenv-virtualenv:

```bash
# Before (pyenv-virtualenv)
pyenv virtualenv 3.12 myproject
pyenv activate myproject

# After (OMG + standard venv)
omg use python 3.12
python -m venv ~/.virtualenvs/myproject
source ~/.virtualenvs/myproject/bin/activate
```

## Performance Comparison

```bash
# pyenv version switch
time pyenv global 3.12
# real    0m0.080s

# OMG version switch
time omg use python 3.12
# real    0m0.002s
```

**40x faster version switching!**

## Bonus: Unified Runtime Management

With OMG, manage all runtimes the same way:

```bash
# Python
omg use python 3.12

# Node.js
omg use node 20

# Go
omg use go 1.22

# Team sync
omg env capture  # Captures all runtime versions
```

## Troubleshooting

### Python not found
Ensure hook is loaded:
```bash
eval "$(omg hook zsh)"
which python
```

### Wrong version active
Check version files:
```bash
omg which python
cat .python-version
```

### Build errors
Some Python versions require build dependencies:
```bash
# Arch
omg install base-devel openssl zlib

# Debian/Ubuntu
sudo apt install build-essential libssl-dev zlib1g-dev
```

## Next Steps

- [Runtime Management](/runtimes) — Full runtime documentation
- [Configuration](/configuration) — Customize paths and behavior
- [Team Sync](/workflows) — Share environments with your team
