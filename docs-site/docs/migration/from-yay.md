---
title: Migrating from yay
sidebar_label: From yay
sidebar_position: 1
description: Command mapping and migration guide from yay to OMG
---

# Migrating from yay

This guide helps yay users transition to OMG with familiar command patterns and enhanced capabilities.

## Why Migrate?

| Feature | yay | OMG |
|---------|-----|-----|
| Search Speed | 200-800ms | **6ms (22x faster)** |
| Runtime Management | ❌ | ✅ Node, Python, Go, Rust, Ruby, Java, Bun |
| Security Scanning | ❌ | ✅ CVE scanning, SBOM generation |
| Team Sync | ❌ | ✅ Environment lockfiles |
| Language | Go | Rust (pure, no subprocess) |

## Command Mapping

### Package Operations

| yay | OMG | Notes |
|-----|-----|-------|
| `yay -Ss <query>` | `omg search <query>` | 22x faster, unified results |
| `yay -S <pkg>` | `omg install <pkg>` | Security grading included |
| `yay -R <pkg>` | `omg remove <pkg>` | Same behavior |
| `yay -Syu` | `omg update` | Updates official + AUR |
| `yay -Si <pkg>` | `omg info <pkg>` | Richer metadata |
| `yay -Sc` | `omg clean` | Clears caches |
| `yay -Qe` | `omg explicit` | List explicitly installed |
| `yay -Sy` | `omg sync` | Sync databases |

### Interactive Mode

```bash
# yay interactive search
yay <query>

# OMG equivalent
omg search <query> -i
```

### AUR Operations

OMG handles AUR transparently:

```bash
# Search includes AUR automatically
omg search spotify

# Install from AUR (auto-detected)
omg install spotify

# Update AUR packages
omg update
```

## Configuration Migration

### yay config location
```
~/.config/yay/config.json
```

### OMG config location
```
~/.config/omg/config.toml
```

### Common Settings

```toml
# ~/.config/omg/config.toml

[aur]
enabled = true
build_dir = "~/.cache/omg/aur"

[search]
limit = 50
include_aur = true
```

## New Capabilities

After migrating, you gain access to:

### Runtime Management
```bash
# Install and use Node.js
omg use node 20

# Install Python
omg use python 3.12

# List available versions
omg list node --available
```

### Security Scanning
```bash
# Scan for vulnerabilities
omg audit

# Generate SBOM
omg audit sbom --format cyclonedx
```

### Team Sync
```bash
# Capture environment
omg env capture

# Share with team
omg env share
```

### Shell Integration
```bash
# Add to ~/.zshrc
eval "$(omg hook zsh)"

# Enable completions
omg completions zsh > ~/.zsh/completions/_omg
```

## Common Workflows

### Daily Update
```bash
# yay
yay -Syu

# OMG (same result, faster)
omg update
```

### Search and Install
```bash
# yay
yay firefox

# OMG
omg search firefox -i
# or
omg install firefox
```

### Clean System
```bash
# yay
yay -Sc && yay -Yc

# OMG
omg clean
```

## Troubleshooting

### "Package not found in AUR"
OMG searches official repos first. Use `omg search <pkg> --aur` to force AUR search.

### Build failures
Check `~/.cache/omg/aur/<pkg>/` for build logs. Same PKGBUILD format as yay.

### Missing dependencies
```bash
omg doctor
```

## Next Steps

- [CLI Reference](/cli) — Full command documentation
- [Configuration](/configuration) — All config options
- [Security](/security) — Vulnerability scanning setup
