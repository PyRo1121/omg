---
title: Team Collaboration
sidebar_position: 21
description: Environment lockfiles, drift detection, and team sync
---

# Team Collaboration

**Environment Sharing, Drift Detection, and Team Sync**

OMG provides collaborative features that ensure all team members work with identical development environments.

---

## 🎯 Overview

Team collaboration features:

- **Environment Lockfiles** — Capture exact runtime versions and packages
- **Drift Detection** — Alert when environments diverge
- **Gist Sharing** — Share environments via GitHub Gists
- **Team Sync** — Centralized environment management

---

## 📦 Environment Lockfiles

### What's Captured

The `omg.lock` file captures:

- **Runtime versions**: Node, Python, Go, Rust, Ruby, Java, Bun
- **Explicit packages**: Packages you explicitly installed
- **Environment fingerprint**: SHA256 hash for quick comparison

### Capture Environment

```bash
# Create omg.lock in current directory
omg env capture
```

Example output (`omg.lock`):

```json
{
  "version": "1.0",
  "timestamp": "2026-01-18T10:30:00Z",
  "fingerprint": "a1b2c3d4e5f6...",
  "runtimes": {
    "node": "20.10.0",
    "python": "3.12.0",
    "rust": "1.75.0",
    "go": "1.21.5"
  },
  "packages": {
    "explicit": [
      "firefox",
      "neovim",
      "visual-studio-code-bin"
    ]
  }
}
```

### Check for Drift

```bash
omg env check
```

Output:
```
✓ Environment matches omg.lock

# Or if there's drift:
⚠ Environment drift detected:
  - node: expected 20.10.0, found 20.11.0
  - Missing package: visual-studio-code-bin
```

---

## 🔗 Sharing Environments

### Share via GitHub Gist

```bash
# Set GitHub token
export GITHUB_TOKEN=ghp_xxxxxxxxxxxx

# Share current environment
omg env share
```

Output:
```
✓ Environment shared!
  URL: https://gist.github.com/username/abc123def456
  
Share this URL with your team.
```

### Sync from Shared Environment

```bash
# Sync from a teammate's environment
omg env sync https://gist.github.com/username/abc123def456
```

This will:
1. Download the lockfile
2. Compare against local environment
3. Prompt to install missing runtimes/packages
4. Update local `omg.lock`

---

## 👥 Team Workspaces

### Initialize Team

```bash
# Create team workspace
omg team init mycompany/frontend

# With custom name
omg team init mycompany/frontend --name "Frontend Team"
```

This creates:
- Team configuration file
- Git hooks for environment checking
- Initial environment lock

### Join Existing Team

```bash
# Join from remote URL
omg team join https://github.com/mycompany/env-config
```

### Team Commands

```bash
# Check team sync status
omg team status

# Push your environment to team lock
omg team push

# Pull team lock and check for drift
omg team pull

# List team members and their sync status
omg team members
```

---

## 🔄 Workflow Patterns

### New Project Setup

```bash
# 1. Create project
mkdir my-project && cd my-project
git init

# 2. Set up version files
echo "20.10.0" > .nvmrc
echo "3.12.0" > .python-version

# 3. Install runtimes
omg use node 20.10.0
omg use python 3.12.0

# 4. Capture environment
omg env capture

# 5. Commit
git add omg.lock .nvmrc .python-version
git commit -m "chore: add environment configuration"
```

### Daily Sync Workflow

```bash
# Start of day
git pull
omg env check  # Check for drift

# If drift detected
omg env sync   # Sync to project environment

# Work...
# ...

# End of day (if you changed environment)
omg env capture
git add omg.lock
git commit -m "chore: update environment lock"
```

### Onboarding New Team Members

**For the new member:**

```bash
# 1. Install OMG
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash

# 2. Setup shell
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc

# 3. Clone project
git clone git@github.com:company/project.git
cd project

# 4. Sync environment
omg env sync

# 5. Verify
omg env check
omg which node
omg which python
```

---

## 🔍 Fingerprinting

### How Fingerprints Work

The environment fingerprint is a SHA256 hash of:
- All active runtime versions
- Explicit package list (sorted)

This allows quick comparison without sending full package lists.

### Check Fingerprint

```bash
omg env fingerprint
# a1b2c3d4e5f6789...
```

### Compare Fingerprints

```bash
# Quickly check if teammate's environment matches
omg env fingerprint
# Compare with teammate's output
```

---

## 🛡️ Best Practices

### 1. Version Control Your Lockfile

```gitignore
# .gitignore - DO NOT ignore omg.lock
# Include it in version control
```

```bash
git add omg.lock
git commit -m "chore: update environment lock"
```

### 2. Use Version Files

Create explicit version files for each runtime:

```bash
# Node.js
echo "20.10.0" > .nvmrc

# Python
echo "3.12.0" > .python-version

# Rust
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
EOF
```

### 3. Check Before Push

Add to your workflow:

```bash
# Pre-push check
omg env check
```

Or add a Git hook:

```bash
# .git/hooks/pre-push
#!/bin/bash
omg env check || exit 1
```

### 4. Automate in CI

```yaml
# .github/workflows/ci.yml
- name: Check Environment
  run: omg env check
```

---

## 🔧 Configuration

### GitHub Token

For sharing via Gist:

```bash
# Option 1: Environment variable
export GITHUB_TOKEN=ghp_xxxxxxxxxxxx

# Option 2: In shell config
echo 'export GITHUB_TOKEN=ghp_xxxx' >> ~/.zshrc
```

Required scopes:
- `gist` (for creating Gists)

### Team Settings

Team configuration in `.omg/team.toml`:

```toml
[team]
id = "mycompany/frontend"
name = "Frontend Team"
remote = "https://github.com/mycompany/env-config"

[sync]
auto_check = true  # Check on directory enter
auto_prompt = true # Prompt to sync on drift
```

---

## 🔍 Troubleshooting

### "GITHUB_TOKEN not set"

```bash
# Create a token at https://github.com/settings/tokens
# With 'gist' scope

export GITHUB_TOKEN=ghp_xxxxxxxxxxxx
```

### Drift Detected but Nothing Changed

```bash
# Re-capture environment
omg env capture

# Check what's different
diff <(cat omg.lock | jq '.runtimes') <(omg env fingerprint --json | jq '.runtimes')
```

### Sync Fails

```bash
# Check URL is accessible
curl -I <gist-url>

# Try manual sync
curl -s <gist-raw-url> > omg.lock
omg env check
```

---

## 📚 See Also

- [Workflows](./workflows.md) — Team onboarding workflow
- [Configuration](./configuration.md) — Environment settings
- [Quick Start](./quickstart.md) — Initial setup
