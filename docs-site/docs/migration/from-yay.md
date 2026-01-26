---
title: Migrate from yay to OMG - Arch Linux AUR Helper Alternative
sidebar_label: From yay
sidebar_position: 1
description: Switch from yay to OMG for 22x faster AUR searches. Complete command mapping, migration steps, and enhanced features including runtime management and security scanning.
keywords:
  - yay alternative
  - yay replacement
  - aur helper
  - arch linux package manager
  - faster yay
  - migrate from yay
  - pacman wrapper
  - arch user repository
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

## New Capabilities

After migrating, you gain access to:

### Runtime Management
```bash
omg use node 20
omg use python 3.12
omg list node --available
```

### Security Scanning
```bash
omg audit
omg audit sbom --format cyclonedx
```

### Team Sync
```bash
omg env capture
omg env share
```

## Next Steps

- [CLI Reference](/cli) — Full command documentation
- [Configuration](/configuration) — All config options
- [Security](/security) — Vulnerability scanning setup
