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

## 🔄 Daily Development Workflow

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

## 👥 Team Onboarding

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
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash

# 2. Set up shell integration
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc

# 3. Clone project
git clone git@github.com:company/project.git
cd project

# 4. Sync environment from lockfile
omg env sync

# 5. Verify everything matches
omg env check

# 6. You're ready!
omg run dev
```

### Keeping Team in Sync

```bash
# Check for drift daily
omg env check

# If drift detected:
omg env sync  # Get latest from team

# Or update and share your changes:
omg env capture
omg team push
```

---

## 🏗️ Project Setup

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

# 2. Or use .mise.toml for more options
cat > .mise.toml << 'EOF'
[tools]
node = "20.10.0"
python = "3.12.0"
rust = "stable"
deno = "1.40.0"
EOF

# 3. Install all runtimes
omg use node 20.10.0
omg use python 3.12.0
omg use rust stable
omg use deno 1.40.0

# 4. Capture complete environment
omg env capture
```

---

## 🔄 CI/CD Integration

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
          curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      # Sync environment from lockfile
      - name: Sync Environment
        run: |
          omg env sync

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
  - curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
  - export PATH="$HOME/.local/bin:$PATH"
  - omg env sync

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
RUN curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash

# Copy project
WORKDIR /app
COPY . .

# Sync environment
RUN omg env sync

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

## 🔐 Security Compliance

### Daily Security Audit

```bash
# 1. Run vulnerability scan
omg audit

# 2. Check for high-severity issues
omg audit scan | grep -i critical

# 3. Update vulnerable packages
omg update
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
# 1. Create strict policy
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
mkdir compliance-$(date +%Y%m%d)
cd compliance-$(date +%Y%m%d)

# SBOM
omg audit sbom -o sbom.json

# Vulnerability report
omg audit scan > vulnerabilities.txt

# Audit log
omg audit log --limit 10000 > audit-log.jsonl

# Log integrity
omg audit verify > integrity-check.txt

# Package list
omg explicit > explicit-packages.txt

# Create archive
cd ..
tar -czf compliance-$(date +%Y%m%d).tar.gz compliance-$(date +%Y%m%d)/
```

---

## 🔧 System Maintenance

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

# Remove unused versions
omg uninstall node 18.17.0
omg uninstall python 3.10.0

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

## 🚚 Migration from Other Tools

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
omg search -a package-name
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

### From mise

```bash
# 1. OMG includes mise internally
# No migration needed!

# 2. OMG reads .mise.toml natively

# 3. Just add OMG hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc

# 4. Your mise config files work automatically
```

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

```bash
# Add to crontab -e
0 9 * * * ~/.local/bin/omg audit scan | grep -q "high_severity" && notify-send "OMG: Security Alert"
```

---

## 📚 See Also

- [CLI Reference](./cli.md) — Complete command documentation
- [Configuration](./configuration.md) — Detailed configuration options
- [Security & Compliance](./security.md) — Enterprise security features
- [Team Collaboration](./team.md) — Advanced team features
