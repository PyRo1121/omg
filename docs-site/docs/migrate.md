---
title: Migration Tools
sidebar_position: 48
description: Move environments between machines and distros
---

# Migration Tools

**Cross-Distro Environment Portability**

OMG makes it easy to move your development environment from one machine to another, even if they are running different Linux distributions (e.g., Arch to Ubuntu).

## 📦 Exporting Your Environment

Create a portable manifest of your current setup.

```bash
omg migrate export my-env.json
```

The manifest includes:
- **Runtimes**: Exact versions of Node, Python, Rust, etc.
- **Packages**: A normalized list of installed tools.
- **Mapping Data**: Logic to translate package names between distros (e.g., `base-devel` on Arch becomes `build-essential` on Debian).

## 📥 Importing an Environment

Restore an environment on a new machine.

```bash
# Preview changes (Dry Run)
omg migrate import my-env.json --dry-run

# Apply migration
omg migrate import my-env.json
```

### Intelligent Mapping

OMG automatically handles package name differences:

| Original (Arch) | Target (Ubuntu) | Status |
|-----------------|-----------------|--------|
| `base-devel`    | `build-essential` | ✅ Mapped |
| `python`        | `python3`       | ✅ Mapped |
| `vim`           | `vim`           | ✅ Match |

If a package cannot be automatically mapped, OMG will list it in the summary so you can install it manually.

## Use Cases

- **Distro Hopping**: Switch from Ubuntu to Arch without losing your dev tools.
- **Onboarding**: Give new hires a single file to set up their machine.
- **Backup**: Keep a JSON record of your setup alongside your dotfiles.
