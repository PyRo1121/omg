---
title: Workflows
sidebar_position: 40
description: Common workflows and recipes
---

# Workflows & Recipes

**Common Patterns and Best Practices for OMG**

This guide provides step-by-step workflows for common tasks, from daily usage to team collaboration and CI/CD integration.

---

## 📋 Table of Contents

1. [Daily Development Workflow](#daily-development-workflow)
2. [Team Onboarding](#team-onboarding)
3. [Project Setup](#project-setup)
4. [CI/CD Integration](#cicd-integration)
5. [Security Compliance](#security-compliance)
6. [System Maintenance](#system-maintenance)
7. [Migration from Other Tools](#migration-from-other-tools)

---

## Daily development workflow

### Morning Startup

Start your day with a quick system check:

```bash
# 1. Start daemon (if not running via systemd)
omg daemon

# 2. Check for updates
omg status

# 3. Update if desired
omg update --check  # See what's available
omg update          # Apply updates
```

### Working on a Project

```bash
# 1. Navigate to project
cd ~/projects/my-app

# 2. OMG automatically detects version files
# (.nvmrc, .python-version, rust-toolchain.toml, etc.)
# and updates PATH via shell hook

# 3. Verify runtime version
omg which node  # Shows version from .nvmrc

# 4. Run project tasks
omg run dev
omg run test

# 5. Install a tool you need
omg tool install jq
```

### End of Day

```bash
# 1. Capture environment state
omg env capture

# 2. Review what changed today
omg history --limit 5

# 3. (Optional) Share with team
omg env share
```

---

## Team onboarding

### For the Team Lead: Setting Up

```bash
# 1. Initialize team workspace
omg team init mycompany/frontend

# 2. Configure shared environment
cd /path/to/project

# 3. Create version files
echo "20.10.0" > .nvmrc
echo "3.12.0" > .python-version

# 4. Create rust-toolchain.toml if needed
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
EOF

# 5. Capture environment
omg env capture

# 6. Commit omg.lock
git add omg.lock .nvmrc .python-version rust-toolchain.toml
git commit -m "chore: add environment lockfile"
git push
```

### For New Team Members: Joining

```bash
# 1. Install OMG
curl -fsSL https://omg.latham.cloud/install.sh | bash

# 2. Set up shell integration
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc

# 3. Clone project
git clone git@github.com:company/project.git
cd project

# 4. Verify environment matches the lockfile
omg env check

# 5. Verify everything matches
omg env check

# 6. You're ready!
omg run dev
```

### Keeping Team in Sync

```bash
# Check for drift daily
omg env check

# If drift detected, pull the latest team lock:
omg team pull  # Pull team lock and check drift

# Or update and share your changes:
omg env capture
omg team push
```

---

## Project setup

### New Rust Project

```bash
# 1. Create project
omg new rust my-cli

# 2. Navigate to project
cd my-cli

# 3. Set Rust version
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
EOF

# 4. Capture environment
omg env capture

# 5. Verify toolchain
omg which rust

# 6. Run tasks
omg run build
omg run test
```

### New React/Node Project

```bash
# 1. Create project
omg new react my-app

# 2. Navigate to project
cd my-app

# 3. Set Node version
echo "20.10.0" > .nvmrc

# 4. Install dependencies
omg run install

# 5. Start development
omg run dev
```

### Multi-Runtime Project

For projects needing multiple runtimes:

```bash
# 1. Create .tool-versions (asdf format)
cat > .tool-versions << 'EOF'
node 20.10.0
python 3.12.0
rust stable
go 1.21.0
EOF

# 2. Install all runtimes
omg use node 20.10.0
omg use python 3.12.0
omg use rust stable

# 3. Capture complete environment
omg env capture
```

---

## CI/CD integration

### GitHub Actions

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Install OMG
      - name: Install OMG
        run: |
          curl -fsSL https://omg.latham.cloud/install.sh | bash
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      # Verify environment against omg.lock
      - name: Check Environment
        run: |
          omg env check

      # Verify environment
      - name: Verify Environment
        run: |
          omg env check
          omg which node
          omg which python

      # Run tasks
      - name: Install Dependencies
        run: omg run install

      - name: Run Tests
        run: omg run test

      - name: Run Build
        run: omg run build
```

### GitLab CI

```yaml
# .gitlab-ci.yml
stages:
  - setup
  - test
  - build

variables:
  OMG_SOCKET_PATH: "/tmp/omg.sock"

before_script:
  - curl -fsSL https://omg.latham.cloud/install.sh | bash
  - export PATH="$HOME/.local/bin:$PATH"
  - omg env check

test:
  stage: test
  script:
    - omg run test

build:
  stage: build
  script:
    - omg run build
  artifacts:
    paths:
      - dist/
```

### Docker Integration

```dockerfile
# Dockerfile
FROM archlinux:latest

# Install OMG
RUN curl -fsSL https://omg.latham.cloud/install.sh | bash

# Copy project
WORKDIR /app
COPY . .

# Verify environment matches the committed omg.lock
RUN omg env check

# Build
RUN omg run build

# Run
CMD ["omg", "run", "start"]
```

With OMG container commands:

```bash
# Initialize containerized environment
omg container init

# Build container
omg container build -t myapp

# Run in container
omg container run myapp -- npm start

# Development shell
omg container shell
```

---

## Security compliance

These recipes collect review inputs on Arch with a running daemon. They do not certify compliance or implement HIPAA controls. Scans do not fail solely because vulnerabilities exist; local log verification checks consistency, not authenticity. Exports and tar archives are plaintext. Restrict destinations and encrypt externally when required. See [security limits](./security.md).

### Daily Security Audit

```bash
# 1. Run vulnerability scan
omg audit

# 2. Review the report, then preview available updates
omg audit fix --dry-run
```

### Weekly Compliance Check

```bash
# 1. Generate SBOM for compliance
omg audit sbom -o sbom-$(date +%Y%m%d).json

# 2. Scan for secrets
omg audit secrets -p .

# 3. Verify audit log integrity
omg audit verify

# 4. Export audit log for review
omg audit log --limit 1000 > audit-$(date +%Y%m%d).log
```

### Setting Up Security Policy

```bash
# Review and back up any existing policy before replacing it.
# Explicit install/upgrade policy enforcement is supported on ALPM, not native APT/DNF/Homebrew.
cat > ~/.config/omg/policy.toml << 'EOF'
# Only allow verified packages
minimum_grade = "Verified"

# Disable AUR for security
allow_aur = false

# Require signatures
require_pgp = true

# Allowed licenses
allowed_licenses = [
    "Apache-2.0",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
]

# Block known problematic packages
banned_packages = []
EOF

# 2. Test policy
omg install some-package
# Will be rejected if doesn't meet policy
```

### Compliance Reporting

```bash
# Generate compliance package
mkdir -m 700 compliance-$(date +%Y%m%d)
cd compliance-$(date +%Y%m%d)

# SBOM
omg audit sbom -o sbom.json

# Vulnerability report
omg audit scan > vulnerabilities.txt

# Audit log
omg audit log --limit 10000 --export audit-log.json

# Log integrity
omg audit verify > integrity-check.txt

# Package list
omg explicit > explicit-packages.txt

# Create archive
cd ..
tar -czf compliance-$(date +%Y%m%d).tar.gz compliance-$(date +%Y%m%d)/
```

---

## System maintenance

### Weekly Maintenance

```bash
# 1. Update everything
omg update

# 2. Clean up orphans
omg clean --orphans

# 3. Clear old caches
omg clean --cache

# 4. Check for issues
omg doctor
```

### Monthly Deep Clean

```bash
# 1. Full cleanup
omg clean --all

# 2. Remove old runtime versions
# List what's installed
omg list node
omg list python

# Remove only versions you no longer need. Switch away from active versions first.
omg use node 18.17.0 --uninstall
omg use python 3.10.0 --uninstall
# Then verify what remains:
omg list node
omg list python

# 3. Verify system health
omg doctor

# 4. Backup history
cp ~/.local/share/omg/history.json ~/.local/share/omg/history.json.bak
```

### Before Major Upgrades

```bash
# 1. Create restoration point
omg env capture
cp omg.lock omg.lock.backup

# 2. Export package list
omg explicit > packages.txt

# 3. Check current status
omg status

# 4. Review history
omg history --limit 10

# 5. Proceed with upgrade
omg update

# 6. If issues, rollback
omg rollback
```

---

## Migration from other tools

### From nvm (Node Version Manager)

```bash
# 1. List current nvm versions
nvm ls

# 2. Install same versions with OMG
omg use node 20.10.0
omg use node 18.17.0
# etc.

# 3. Set default
omg use node 20.10.0

# 4. Remove nvm hook from shell config
# Edit ~/.zshrc, remove nvm lines

# 5. Add OMG hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc

# 6. (Optional) Remove nvm
rm -rf ~/.nvm
```

### From pyenv (Python Version Manager)

```bash
# 1. List current pyenv versions
pyenv versions

# 2. Install same versions with OMG
omg use python 3.12.0
omg use python 3.11.0
# etc.

# 3. Update shell config
# Remove pyenv init from ~/.zshrc
# Add OMG hook

# 4. (Optional) Remove pyenv
rm -rf ~/.pyenv
```

### From rustup

```bash
# 1. List current toolchains
rustup show

# 2. OMG manages toolchains directly
omg use rust stable
omg use rust nightly

# 3. rustup can coexist if needed
# Or uninstall:
rustup self uninstall
```

### From yay/paru (AUR Helpers)

```bash
# 1. OMG handles AUR natively
omg search package-name        # Search includes AUR automatically
omg install aur-package

# 2. No migration needed - OMG reads same databases
# yay/paru can coexist

# 3. (Optional) List packages installed by yay
yay -Qm

# 4. These will appear in
omg explicit
```

### From asdf/.tool-versions

```bash
# 1. OMG reads .tool-versions natively
# No migration needed!

# 2. Just add OMG hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc

# 3. OMG will respect existing .tool-versions files

# 4. (Optional) Remove asdf
rm -rf ~/.asdf
# Remove asdf lines from shell config
```

### From other runtime managers

Convert supported runtime pins to `.tool-versions`, then install each exact version with `omg use`. Unsupported runtimes must remain with their existing manager.

---

## 📊 Monitoring and Metrics

### Shell Prompt Integration

Add package counts to your prompt:

```bash
# For Zsh, in ~/.zshrc:
# Fast cached counts
PROMPT='[📦 $(omg-ec)] %~$ '

# Or with styling
PROMPT='%F{cyan}[$(omg-ec) pkgs]%f %~$ '
```

### Status Dashboard

Keep a terminal open with the dashboard:

```bash
# Start dashboard
omg dash

# Keys:
# q - quit
# r - refresh
# Tab - switch views
```

### Automated Alerts

Create a cron job for security monitoring:

Run `omg audit scan` and review the report. It requires the daemon. Its display is not a documented machine-readable alert interface, and findings alone do not cause a nonzero exit status. Do not use a `high_severity` grep as a security gate.

---

## 📚 See Also

- [CLI Reference](./cli.md) — Complete command documentation
- [Configuration](./configuration.md) — Detailed configuration options
- [Security & Compliance](./security.md) — Enterprise security features
- [Team Collaboration](./team.md) — Advanced team features
