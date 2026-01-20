---
title: Shell Integration
sidebar_position: 12
description: Hooks, completions, and PATH management
---

# Shell Integration

**Hooks, Completions, and PATH Management**

This guide covers OMG's shell integration features including the shell hook, completions, and ultra-fast shell functions.

---

## 🎯 Overview

OMG provides deep shell integration that:

1. **Automatically updates PATH** when you change directories
2. **Detects version files** and activates the correct runtime
3. **Provides instant completions** for all commands
4. **Exposes ultra-fast functions** for shell prompts

---

## 🔧 Shell Hook Installation

### What the Hook Does

The shell hook:
- Runs on every directory change (`cd`)
- Runs on every prompt (to catch `pushd`, `popd`, etc.)
- Detects version files (`.nvmrc`, `.python-version`, etc.)
- Prepends correct runtime `bin` directories to PATH
- Provides fast package count functions

### Installation by Shell

#### Zsh

Add to `~/.zshrc`:

```bash
eval "$(omg hook zsh)"
```

Reload:
```bash
source ~/.zshrc
# or
exec zsh
```

#### Bash

Add to `~/.bashrc`:

```bash
eval "$(omg hook bash)"
```

Reload:
```bash
source ~/.bashrc
# or
exec bash
```

#### Fish

Add to `~/.config/fish/config.fish`:

```fish
omg hook fish | source
```

Reload:
```fish
source ~/.config/fish/config.fish
# or
exec fish
```

### Verifying Installation

```bash
# Test hook output
omg hook zsh

# Check if hook is active
type _omg_hook

# Test version detection
cd /path/to/project/with/.nvmrc
echo $PATH | grep omg
```

---

## 📝 How Version Detection Works

### Detection Order

When you enter a directory, OMG checks for version files in this order:

1. **Current directory**
2. **Parent directories** (walking up to filesystem root)
3. **Global default** (`~/.local/share/omg/versions/<runtime>/current`)

### Supported Version Files

| File | Runtime | Priority |
|------|---------|----------|
| `.node-version` | Node.js | 1 (highest) |
| `.nvmrc` | Node.js | 2 |
| `.bun-version` | Bun | 1 |
| `.python-version` | Python | 1 |
| `.ruby-version` | Ruby | 1 |
| `.go-version` | Go | 1 |
| `go.mod` | Go | 2 |
| `.java-version` | Java | 1 |
| `rust-toolchain.toml` | Rust | 1 |
| `rust-toolchain` | Rust | 2 |
| `.tool-versions` | Multiple | 3 |
| `.mise.toml` | Multiple | 3 |
| `.mise.local.toml` | Multiple | 2 |
| `mise.toml` | Multiple | 4 |
| `package.json` | Node/Bun | 4 (engines/volta) |

### Version File Formats

#### Simple Version Files

`.nvmrc`, `.python-version`, `.go-version`, etc.:

```
20.10.0
```

Or with `v` prefix:
```
v20.10.0
```

Or aliases (Node.js):
```
lts/*
lts/hydrogen
```

#### rust-toolchain.toml

```toml
[toolchain]
channel = "stable"  # or "nightly" or "1.75.0"
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown"]
profile = "minimal"
```

Or simple `rust-toolchain` file:
```
stable
```

#### .tool-versions (asdf format)

```
node 20.10.0
python 3.12.0
rust stable
go 1.21.0
```

#### .mise.toml

```toml
[tools]
node = "20.10.0"
python = "3.12.0"
rust = "stable"
deno = "1.40.0"

[tools.node]
version = "20"

[tools.python]
version = "3.12"
```

#### package.json

```json
{
  "engines": {
    "node": ">=18 <21",
    "bun": ">=1.0"
  },
  "volta": {
    "node": "20.10.0",
    "bun": "1.0.25"
  }
}
```

Priority: `engines` > `volta`

---

## 🚀 Ultra-Fast Shell Functions

The hook provides cached package count functions for shell prompts.

### Cached Functions (Sub-Microsecond)

These read from a cached status file updated by the daemon:

| Function | Returns |
|----------|---------|
| `omg-ec` | Explicit package count |
| `omg-tc` | Total package count |
| `omg-oc` | Orphan count |
| `omg-uc` | Updates count |

### Fresh Functions (~1ms)

These read the status file directly:

| Function | Returns |
|----------|---------|
| `omg-explicit-count` | Explicit package count |
| `omg-total-count` | Total package count |
| `omg-orphan-count` | Orphan count |
| `omg-updates-count` | Updates count |

### Using in Prompts

#### Zsh Prompt

```bash
# In ~/.zshrc
PROMPT='[📦 $(omg-ec)] %~$ '

# Or with colors
PROMPT='%F{cyan}[$(omg-ec) pkgs]%f %~$ '

# Full example with git
PROMPT='%F{green}%n@%m%f %F{blue}%~%f $(git_prompt_info)[📦 $(omg-ec)]$ '
```

#### Bash Prompt

```bash
# In ~/.bashrc
export PS1='[\w] $(omg-ec) pkgs$ '

# With colors
export PS1='\[\e[36m\][$(omg-ec) pkgs]\[\e[0m\] \w$ '
```

#### Fish Prompt

```fish
# In ~/.config/fish/functions/fish_prompt.fish
function fish_prompt
    echo -n (omg-ec)" pkgs "
    set_color blue
    echo -n (prompt_pwd)
    set_color normal
    echo '$ '
end
```

### Performance Comparison

| Method | Latency | Use Case |
|--------|---------|----------|
| `omg-ec` (cached) | &lt;1μs | Prompts |
| `omg-explicit-count` (fresh) | ~1ms | Scripts |
| `omg explicit --count` | ~1.2ms | Commands |
| `pacman -Qq \| wc -l` | ~14ms | Fallback |

---

## 🔤 Shell Completions

### Installation

#### Zsh

```bash
# Create completions directory
mkdir -p ~/.zsh/completions

# Generate completions
omg completions zsh > ~/.zsh/completions/_omg

# Add to fpath in ~/.zshrc (if not already)
fpath=(~/.zsh/completions $fpath)
autoload -Uz compinit && compinit

# Rebuild completion cache
rm ~/.zcompdump
compinit
```

#### Bash

```bash
# System-wide
sudo omg completions bash > /etc/bash_completion.d/omg

# Or user-only
omg completions bash > ~/.local/share/bash-completion/completions/omg

# Source immediately
source /etc/bash_completion.d/omg
```

#### Fish

```bash
# User completions
omg completions fish > ~/.config/fish/completions/omg.fish

# System-wide
sudo omg completions fish > /usr/share/fish/vendor_completions.d/omg.fish
```

### Completion Features

OMG provides intelligent completions for:

- **Commands**: All subcommands with descriptions
- **Package names**: From daemon cache (80k+ AUR packages)
- **Runtime versions**: Installed and available versions
- **Options**: All flags with descriptions

### Fuzzy Matching

OMG uses Nucleo for ultra-fast fuzzy matching:

```bash
omg i frfx<TAB>
# Completes to: firefox
```

---

## 🔄 Hook Behavior Deep Dive

### Zsh Hook Internals

The Zsh hook uses `precmd` and `chpwd` functions:

```bash
_omg_hook() {
  trap -- '' SIGINT
  eval "$(omg hook-env -s zsh)"
  trap - SIGINT
}

typeset -ag precmd_functions
if [[ -z "${precmd_functions[(r)_omg_hook]+1}" ]]; then
  precmd_functions=(_omg_hook ${precmd_functions[@]})
fi

typeset -ag chpwd_functions  
if [[ -z "${chpwd_functions[(r)_omg_hook]+1}" ]]; then
  chpwd_functions=(_omg_hook ${chpwd_functions[@]})
fi
```

### Bash Hook Internals

Bash uses `PROMPT_COMMAND`:

```bash
_omg_hook() {
  local previous_exit_status=$?
  trap -- '' SIGINT
  eval "$(omg hook-env -s bash)"
  trap - SIGINT
  return $previous_exit_status
}

if [[ ! "${PROMPT_COMMAND:-}" =~ _omg_hook ]]; then
  PROMPT_COMMAND="_omg_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
```

### Fish Hook Internals

Fish uses event handlers:

```fish
function _omg_hook --on-variable PWD --on-event fish_prompt
  omg hook-env -s fish | source
end
```

### Cache Refresh

The hook includes automatic cache refresh every 60 seconds:

```bash
_omg_refresh_cache() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || return
  local now=$EPOCHSECONDS
  (( now - _OMG_CACHE_TIME < 60 )) && return
  _OMG_CACHE_TIME=$now
  local data=$(od -An -j8 -N16 -tu4 "$f" 2>/dev/null)
  read _OMG_TOTAL _OMG_EXPLICIT _OMG_ORPHANS _OMG_UPDATES <<< "$data"
}
```

---

## 🛠️ Manual PATH Management

If you prefer manual control over PATH:

### Without Hook

```bash
# Manually add runtime paths
export PATH="$HOME/.local/share/omg/versions/node/current/bin:$PATH"
export PATH="$HOME/.local/share/omg/versions/python/current/bin:$PATH"
export PATH="$HOME/.local/share/omg/versions/go/current/bin:$PATH"
export PATH="$HOME/.local/share/omg/versions/rust/current/bin:$PATH"
```

### Project-Specific

```bash
# Add to project's .envrc (if using direnv)
export PATH="$HOME/.local/share/omg/versions/node/20.10.0/bin:$PATH"
```

### Check Active Versions

```bash
# See what's in PATH
echo $PATH | tr ':' '\n' | grep omg

# Check symlinks
ls -la ~/.local/share/omg/versions/node/current
```

---

## ⚙️ Configuration Options

### Shims vs Hooks

OMG offers two PATH management methods:

#### Hooks (Default, Recommended)

- Faster runtime switching
- PATH modified on directory change
- Works with all terminals

#### Shims

- Better IDE compatibility
- Wrapper scripts for each binary
- Slightly slower execution

Enable shims in `~/.config/omg/config.toml`:

```toml
shims_enabled = true
```

Generate shims:
```bash
omg shim generate
```

### Runtime Backend

Control runtime resolution:

```toml
# In ~/.config/omg/config.toml
runtime_backend = "native-then-mise"
```

Options:
- `native` — Only OMG's built-in managers
- `mise` — Only mise
- `native-then-mise` — Native first, mise fallback (default)

---

## 🔍 Troubleshooting

### Hook Not Running

```bash
# 1. Check hook is in shell config
grep "omg hook" ~/.zshrc

# 2. Verify hook works
omg hook zsh | head -20

# 3. Check function exists
type _omg_hook

# 4. Force reload
exec zsh
```

### Wrong Version Active

```bash
# 1. Check version files
ls -la .nvmrc .python-version .tool-versions

# 2. Check what OMG detects
omg which node

# 3. Check PATH order
echo $PATH | tr ':' '\n' | head -10
# OMG paths should be first

# 4. Force switch
omg use node 20.10.0
```

### Slow Directory Changes

```bash
# 1. Ensure daemon is running
omg status

# 2. Check hook-env timing
time omg hook-env -s zsh
# Should be under 10ms

# 3. If slow, the daemon may be down
omg daemon
```

### Completions Not Working

```bash
# Zsh
rm ~/.zcompdump
omg completions zsh > ~/.zsh/completions/_omg
compinit

# Check fpath
echo $fpath | tr ' ' '\n' | grep completions
```

---

## 📊 Performance Tips

### 1. Keep Daemon Running

```bash
# Start daemon on login
echo "omg daemon &" >> ~/.zprofile

# Or use systemd
systemctl --user enable omgd
```

### 2. Use Cached Functions in Prompts

```bash
# Fast (cached)
PROMPT='$(omg-ec) pkgs$ '

# Avoid (hits binary each time)
PROMPT='$(omg explicit --count) pkgs$ '
```

### 3. Minimize Version Files

Only place version files in project roots, not deeply nested directories.

### 4. Combine with Starship/Powerlevel10k

These prompt themes have built-in version display. Combine with OMG:

```bash
# OMG handles PATH, Starship handles display
eval "$(omg hook zsh)"
eval "$(starship init zsh)"
```

---

## 📚 See Also

- [Quick Start](./quickstart.md) — Initial setup
- [Configuration](./configuration.md) — Shell and runtime settings
- [Runtime Management](./runtimes.md) — Version file details
- [Troubleshooting](./troubleshooting.md) — Common issues
