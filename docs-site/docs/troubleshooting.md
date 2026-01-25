---
title: Troubleshooting
sidebar_position: 50
description: Common issues and solutions
---

# Troubleshooting Guide

**Common Issues and Solutions for OMG**

This guide covers common problems, their causes, and step-by-step solutions.

---

## 🔍 Quick Diagnostics

Before diving into specific issues, run the built-in diagnostics:

```bash
# Run full health check
omg doctor

# Check system status
omg status

# View daemon status
omg-fast status
```

---

## 🛠️ Daemon Issues

### Daemon Not Running

**Symptoms:**
- Slow searches (>50ms instead of ~6ms)
- `omg status` shows "Daemon: Not running"
- Commands work but feel sluggish

**Solutions:**

```bash
# 1. Start the daemon
omg daemon

# 2. If that fails, check for stale socket
ls -la $XDG_RUNTIME_DIR/omg.sock

# 3. Remove stale socket if present
rm $XDG_RUNTIME_DIR/omg.sock

# 4. Start daemon in foreground to see errors
omgd --foreground

# 5. Check if port/socket is in use
lsof -U | grep omg
```

---

### Daemon Crashes on Startup

**Symptoms:**
- Daemon starts then immediately exits
- No error message visible

**Solutions:**

```bash
# 1. Run in foreground to see errors
omgd --foreground

# 2. Check for corrupted cache
rm ~/.local/share/omg/cache.redb

# 3. Verify permissions
ls -la ~/.local/share/omg/

# 4. Check system logs
journalctl --user -u omgd -n 50
```

---

### Daemon Socket Permission Denied

**Symptoms:**
- "Permission denied" errors when running commands
- Works with sudo but not as regular user

**Solutions:**

```bash
# 1. Check socket ownership
ls -la $XDG_RUNTIME_DIR/omg.sock

# 2. Remove and recreate socket
rm $XDG_RUNTIME_DIR/omg.sock
omg daemon

# 3. Verify XDG_RUNTIME_DIR
echo $XDG_RUNTIME_DIR
# Should be /run/user/$UID

# 4. Check directory permissions
ls -la $XDG_RUNTIME_DIR
```

---

## 🐚 Shell Integration Issues

### PATH Not Updated After Directory Change

**Symptoms:**
- Runtime versions don't change automatically
- Have to run `omg use` manually each time
- Version files (`.nvmrc`, etc.) not detected

**Solutions:**

```bash
# 1. Verify hook is installed
grep "omg hook" ~/.zshrc  # or ~/.bashrc

# 2. Check hook output
omg hook zsh

# 3. Reinstall hook
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc

# 4. Restart shell (not just source)
exec zsh

# 5. Test hook manually
cd /path/to/project/with/.nvmrc
omg which node
```

---

### Completions Not Working

**Symptoms:**
- Tab completion doesn't show OMG commands
- Partial completion or errors

**Solutions:**

```bash
# For Zsh:
# 1. Regenerate completions
omg completions zsh > ~/.zsh/completions/_omg

# 2. Ensure completions directory is in fpath
echo $fpath | grep completions

# 3. Add to .zshrc if needed
fpath=(~/.zsh/completions $fpath)
autoload -Uz compinit && compinit

# 4. Rebuild completions cache
rm ~/.zcompdump
compinit

# For Bash:
omg completions bash > /etc/bash_completion.d/omg
source /etc/bash_completion.d/omg
```

---

### Shell Hook Slowing Down Prompt

**Symptoms:**
- Noticeable delay when pressing Enter
- Slow directory changes

**Solutions:**

```bash
# 1. Ensure daemon is running (bypasses slow fallback)
omg daemon

# 2. Use cached functions in prompt instead
# Replace: $(omg explicit --count)
# With: $(omg-ec)

# 3. Check for blocking operations
time omg hook-env -s zsh
# Should be &lt;10ms

# 4. Minimize version file checks
# Only place version files in project roots
```

---

## 📦 Package Management Issues

### Search Returns No Results

**Symptoms:**
- `omg search <query>` returns nothing
- Known packages not found

**Solutions:**

```bash
# 1. Sync package databases
omg sync

# 2. Clear and rebuild cache
omg daemon &
sleep 2
omg search linux

# 3. Check daemon cache
omgd --foreground
# Watch for index building messages

# 4. Try direct search (bypasses cache)
pacman -Ss <query>
```

---

### AUR Build Failures

**Symptoms:**
- AUR packages fail to build
- Dependency errors during build

**Solutions:**

```bash
# 1. Check base-devel is installed
pacman -Q base-devel

# 2. Install if missing
sudo pacman -S base-devel

# 3. Check build dependencies
omg info <package-name>

# 4. Clear AUR cache and retry
omg clean --aur
omg install <package>

# 5. Check build logs
cat ~/.cache/omg/logs/<package>.log

# 6. Try manual build
cd ~/.cache/omg/srcdest/<package>
makepkg -si
```

---

### Package Installation Blocked by Policy

**Symptoms:**
- "Package grade X below minimum Y" error
- "AUR packages not allowed" error

**Solutions:**

```bash
# 1. Check current policy
cat ~/.config/omg/policy.toml

# 2. View package security grade
omg info <package>

# 3. Temporarily lower policy
# Edit ~/.config/omg/policy.toml:
# minimum_grade = "Community"
# allow_aur = true

# 4. Install package
omg install <package>

# 5. Restore policy
```

---

## 🔧 Runtime Management Issues

### Runtime Version Not Switching

**Symptoms:**
- `omg use` completes but wrong version active
- `node --version` shows different version than expected

**Solutions:**

```bash
# 1. Check PATH order
echo $PATH | tr ':' '\n' | head -10

# 2. OMG paths should come first
# ~/.local/share/omg/versions/node/current/bin

# 3. Check for conflicting version managers
which -a node
# Should show OMG path first

# 4. Verify symlink
ls -la ~/.local/share/omg/versions/node/current

# 5. Force update symlink
omg use node 20.10.0 --force

# 6. Restart shell
exec zsh
```

---

### Runtime Download Fails

**Symptoms:**
- "Download failed" or network errors
- Timeout during installation

**Solutions:**

```bash
# 1. Check network connectivity
curl -I https://nodejs.org/dist/

# 2. Check for proxy issues
echo $http_proxy $https_proxy

# 3. Increase timeout
# In config.toml:
# [network]
# timeout = 60

# 4. Try manual download
omg list node --available
# Note the URL and download manually

# 5. Check disk space
df -h ~/.local/share/omg/
```

---

### mise Not Found for Extended Runtimes

**Symptoms:**
- Error when installing deno, elixir, etc.
- "mise not available" message

**Solutions:**

```bash
# 1. Check mise installation
ls ~/.local/share/omg/mise/

# 2. Force mise reinstall
rm -rf ~/.local/share/omg/mise/
omg use deno 1.40.0
# Will auto-download mise

# 3. Check mise works
~/.local/share/omg/mise/mise --version

# 4. Verify network access
curl -L https://github.com/jdx/mise/releases/
```

---

## 💾 Cache and Database Issues

### Cache Corruption

**Symptoms:**
- Bizarre search results
- Inconsistent package info
- Daemon errors mentioning "cache" or "redb"

**Solutions:**

```bash
# 1. Stop daemon
pkill omgd

# 2. Remove cache
rm ~/.local/share/omg/cache.redb

# 3. Start daemon (rebuilds cache)
omg daemon

# 4. Wait for cache to populate
sleep 5
omg status
```

---

### History File Corrupted

**Symptoms:**
- `omg history` returns empty or errors
- Rollback doesn't work

**Solutions:**

```bash
# 1. Check history file
cat ~/.local/share/omg/history.json | head

# 2. Validate JSON
python -m json.tool ~/.local/share/omg/history.json

# 3. If corrupted, back up and reset
mv ~/.local/share/omg/history.json ~/.local/share/omg/history.json.bak
echo "[]" > ~/.local/share/omg/history.json
```

---

### Audit Log Issues

**Symptoms:**
- `omg audit log` shows nothing
- `omg audit verify` fails

**Solutions:**

```bash
# 1. Check log file
ls -la ~/.local/share/omg/audit/

# 2. Verify log format
head ~/.local/share/omg/audit/audit.jsonl

# 3. Reset audit log if corrupted
mv ~/.local/share/omg/audit/audit.jsonl ~/.local/share/omg/audit/audit.jsonl.bak
```

---

## 🖥️ TUI Dashboard Issues

### Dashboard Won't Start

**Symptoms:**
- `omg dash` hangs or crashes immediately
- Terminal garbled after exit

**Solutions:**

```bash
# 1. Check terminal compatibility
echo $TERM
# Should show xterm-256color or similar

# 2. Verify alternate screen support
tput smcup
tput rmcup

# 3. Check daemon is running
omg status

# 4. Reset terminal if garbled
reset
# or
stty sane

# 5. Try different terminal emulator
```

---

### Display Garbled or Wrong Colors

**Symptoms:**
- Characters display incorrectly
- Colors wrong or missing

**Solutions:**

```bash
# 1. Ensure proper TERM
export TERM=xterm-256color

# 2. Use a Unicode-capable font
# Recommended: Nerd Fonts, JetBrains Mono

# 3. Resize terminal
# Sometimes fixes rendering issues

# 4. Check locale
echo $LANG
# Should be UTF-8
```

---

## 🔄 History and Rollback Issues

### Rollback Fails

**Symptoms:**
- "Package not found in cache" error
- Downgrade fails

**Solutions:**

```bash
# 1. Check package cache
ls /var/cache/pacman/pkg/ | grep <package>

# 2. If missing, download old version
# From Arch Archive:
# https://archive.archlinux.org/packages/

# 3. Manual downgrade
sudo pacman -U /var/cache/pacman/pkg/<package>-<version>.pkg.tar.zst

# 4. Keep more versions in cache
# In /etc/pacman.conf:
# CleanMethod = KeepCurrent
```

---

### AUR Rollback Not Supported

**Symptoms:**
- "AUR rollback not supported" message

**Solutions:**

This is a current limitation. Workarounds:

```bash
# 1. Rebuild old version manually
cd ~/.cache/omg/srcdest/<package>
git checkout <old-commit>
makepkg -si

# 2. Use Arch Archive for official version
# (if package was once in official repos)
```

---

## 🔐 Security and Audit Issues

### Security Audit Fails

**Symptoms:**
- `omg audit` returns errors
- Vulnerability data not loading

**Solutions:**

```bash
# 1. Ensure daemon is running
omg daemon

# 2. Check network access
curl https://security.archlinux.org/issues/all.json | head

# 3. Check OSV.dev access
curl https://api.osv.dev/v1/query -X POST -d '{}'

# 4. Clear vulnerability cache
# Restart daemon
pkill omgd
omg daemon
```

---

### SBOM Generation Fails

**Symptoms:**
- `omg audit sbom` errors out
- Empty or incomplete SBOM

**Solutions:**

```bash
# 1. Check write permissions
touch /tmp/test-sbom.json

# 2. Specify output path
omg audit sbom -o ~/sbom.json

# 3. Check installed packages
omg explicit
```

---

## 📋 General Troubleshooting Steps

### Reset Everything

If all else fails, complete reset:

```bash
# 1. Stop daemon
pkill omgd

# 2. Backup then remove data
mv ~/.local/share/omg ~/.local/share/omg.bak
mv ~/.config/omg ~/.config/omg.bak

# 3. Remove socket
rm $XDG_RUNTIME_DIR/omg.sock

# 4. Reinstall if needed
cd /path/to/omg
cargo build --release
cp target/release/{omg,omgd,omg-fast} ~/.local/bin/

# 5. Start fresh
omg daemon
omg status
```

---

### Collect Debug Information

When reporting issues:

```bash
# 1. Get system info
uname -a
cat /etc/os-release

# 2. Get OMG version
omg --version

# 3. Run doctor
omg doctor

# 4. Get daemon logs
omgd --foreground 2>&1 | tee omg-debug.log

# 5. Get command output
omg <failing-command> 2>&1 | tee command-output.log
```

---

## 📚 See Also

- [Quick Start Guide](./quickstart.md) — Initial setup
- [Configuration](./configuration.md) — Configuration options
- [Daemon Internals](./daemon.md) — Daemon troubleshooting
- [FAQ](./faq.md) — Frequently asked questions
