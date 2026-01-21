---
title: Shell Hook System Deep Dive
sidebar_position: 36
description: How automatic runtime version switching actually works
---

# Shell Hook System: How It Works

**Understanding OMG's Automatic Runtime Version Detection and PATH Management**

This guide explains the technical implementation behind OMG's "magic" automatic runtime switching when you `cd` between projects.

---

## 🎯 The User Experience

When you install the shell hook:

```bash
eval "$(omg hook zsh)"
```

OMG automatically switches runtime versions based on your project's version files:

```bash
$ cd ~/projects/legacy-app
# (OMG detects .nvmrc with "14.17.0")
# PATH now includes ~/.local/share/omg/versions/node/14.17.0/bin

$ node --version
v14.17.0

$ cd ~/projects/modern-app
# (OMG detects .nvmrc with "20.10.0")
# PATH now includes ~/.local/share/omg/versions/node/20.10.0/bin

$ node --version
v20.10.0
```

**No manual `omg use` commands required**. This guide explains how this works under the hood.

---

## 🏗️ Architecture Overview

The hook system has three components:

1. **Hook Script** — Shell-specific code injected into your `.zshrc`/`.bashrc`
2. **Detection Engine** — Rust code that finds and parses version files
3. **PATH Builder** — Logic that constructs the correct PATH modifications

```
┌─────────────┐
│ User: cd    │
│ ~/project   │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────┐
│ Shell Hook (_omg_hook)              │
│ - Triggered by precmd/chpwd (Zsh)   │
│ - Triggered by PROMPT_COMMAND (Bash)│
└──────┬──────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│ omg hook-env -s zsh                 │
│ (Rust binary)                        │
└──────┬──────────────────────────────┘
       │
       ├─► detect_versions()
       │   (Walk up tree, find .nvmrc, etc.)
       │
       ├─► build_path_additions()
       │   (Map version → bin path)
       │
       └─► Output shell code
           (export PATH="...")

       ▼
┌─────────────────────────────────────┐
│ Shell: eval output                  │
│ (Updates PATH for current session)  │
└─────────────────────────────────────┘
```

---

## 📝 Step 1: Hook Installation

When you run `omg hook zsh`, OMG outputs a shell script:

```bash
# OMG Shell Hook for Zsh
_omg_hook() {
  trap -- '' SIGINT  # Ignore Ctrl+C during hook
  eval "$(\\command omg hook-env -s zsh)"
  trap - SIGINT      # Restore Ctrl+C handler
}

# Register hook to run on every directory change
typeset -ag chpwd_functions
if [[ -z "${chpwd_functions[(r)_omg_hook]+1}" ]]; then
  chpwd_functions=(_omg_hook ${chpwd_functions[@]})
fi

# Register hook to run before every prompt
typeset -ag precmd_functions
if [[ -z "${precmd_functions[(r)_omg_hook]+1}" ]]; then
  precmd_functions=(_omg_hook ${precmd_functions[@]})
fi
```

### Why Both `chpwd` and `precmd`?

- **`chpwd`**: Fires when directory changes (e.g., `cd`, `pushd`)
- **`precmd`**: Fires before every prompt (catches `git checkout`, `ln -s`, etc.)

This ensures version detection works for **all** navigation methods.

### Bash Alternative

Bash uses `PROMPT_COMMAND` instead:

```bash
_omg_hook() {
  local previous_exit_status=$?
  trap -- '' SIGINT
  eval "$(\\command omg hook-env -s bash)"
  trap - SIGINT
  return $previous_exit_status
}

PROMPT_COMMAND="_omg_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

**Important**: Preserves `$?` (exit status) so your prompt theme doesn't break.

---

## 🔍 Step 2: Version File Detection

When the hook runs, it calls `omg hook-env -s zsh`, which executes this Rust function:

```rust
pub fn hook_env(shell: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Detect version files in current directory and parents
    let versions = detect_versions(&cwd);

    if versions.is_empty() {
        return Ok(()); // No version files found
    }

    // Build PATH modifications
    let path_additions = build_path_additions(&versions);

    if path_additions.is_empty() {
        return Ok(()); // No runtimes installed
    }

    // Output shell-specific PATH modification
    match shell {
        "zsh" | "bash" => {
            let additions = path_additions.join(":");
            println!("export PATH=\"{additions}:$PATH\"");
        }
        "fish" => {
            for path in &path_additions {
                println!("fish_add_path -g {path}");
            }
        }
        _ => {}
    }

    Ok(())
}
```

### The `detect_versions()` Algorithm

This function walks up the directory tree looking for version files:

```rust
pub fn detect_versions(start: &Path) -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let mut current = Some(start.to_path_buf());

    // Walk up directory tree
    while let Some(dir) = current {
        for (filename, runtime) in VERSION_FILES {
            // Skip if we already found a version for this runtime
            if versions.contains_key(*runtime) {
                continue;
            }

            let file_path = dir.join(filename);
            if file_path.exists() {
                // Parse the version file
                if let Some(version) = parse_version_file(&file_path, filename) {
                    versions.insert(runtime.to_string(), version);
                }
            }
        }

        // Move to parent directory
        current = dir.parent().map(Path::to_path_buf);
    }

    versions
}
```

**Key insight**: The function stops at the **first** version file for each runtime, implementing a "closest wins" priority.

### Supported Version Files

The `VERSION_FILES` constant defines what to look for:

```rust
const VERSION_FILES: &[(&str, &str)] = &[
    (".node-version", "node"),
    (".nvmrc", "node"),
    (".bun-version", "bun"),
    (".python-version", "python"),
    (".ruby-version", "ruby"),
    (".go-version", "go"),
    ("go.mod", "go"),
    (".java-version", "java"),
    ("rust-toolchain.toml", "rust"),
    ("rust-toolchain", "rust"),
    (".tool-versions", "multi"),
    (".mise.toml", "multi"),
    ("package.json", "multi"),
];
```

### Example: Walking Up the Tree

Given this directory structure:

```
/home/user/
├── .python-version (3.11.0)
└── projects/
    └── my-app/
        ├── .nvmrc (20.10.0)
        └── src/
            └── index.js
```

If you're in `/home/user/projects/my-app/src/`:

1. Check `/home/user/projects/my-app/src/` — No version files
2. Check `/home/user/projects/my-app/` — Found `.nvmrc` → `{"node": "20.10.0"}`
3. Check `/home/user/projects/` — No version files
4. Check `/home/user/` — Found `.python-version` → `{"python": "3.11.0"}`
5. Stop at root

**Result**: `{"node": "20.10.0", "python": "3.11.0"}`

---

## 🗂️ Step 3: Parsing Version Files

Different version files have different formats:

### Simple Version Files (`.nvmrc`, `.python-version`)

```rust
// Simple version file: just a version string
if let Ok(content) = std::fs::read_to_string(&file_path) {
    let version = content.trim().trim_start_matches('v').to_string();
    if !version.is_empty() {
        versions.insert(runtime.to_string(), version);
    }
}
```

**Example `.nvmrc`**:
```
20.10.0
```

### `rust-toolchain.toml`

```rust
if filename == "rust-toolchain.toml" {
    if let Ok(content) = std::fs::read_to_string(&file_path) {
        for line in content.lines() {
            if line.contains("channel") {
                if let Some(version) = line.split('=').nth(1) {
                    let v = version.trim().trim_matches('"').trim_matches('\'');
                    versions.insert(runtime.to_string(), v.to_string());
                }
            }
        }
    }
}
```

**Example `rust-toolchain.toml`**:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

### `.tool-versions` (asdf format)

```rust
if filename == ".tool-versions" {
    if let Ok(content) = std::fs::read_to_string(&file_path) {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let (Some(runtime), Some(version)) = (parts.get(0), parts.get(1)) {
                versions.insert(normalize_runtime_name(runtime), version.to_string());
            }
        }
    }
}
```

**Example `.tool-versions`**:
```
node 20.10.0
python 3.12.0
rust stable
```

### `package.json` (engines/volta)

```rust
fn read_package_json_versions(dir: &Path) -> Option<HashMap<String, String>> {
    let file = std::fs::File::open(dir.join("package.json")).ok()?;
    let pkg: PackageJsonVersions = serde_json::from_reader(file).ok()?;
    let mut versions = HashMap::new();

    // Process volta first (lower priority)
    if let Some(volta) = pkg.volta {
        if let Some(node) = volta.node {
            versions.insert("node".to_string(), node);
        }
    }

    // Process engines second (higher priority — overwrites volta)
    if let Some(engines) = pkg.engines {
        if let Some(node) = engines.node {
            versions.insert("node".to_string(), node);
        }
    }

    Some(versions)
}
```

**Example `package.json`**:
```json
{
  "engines": {
    "node": ">=18 <21"
  },
  "volta": {
    "node": "20.10.0"
  }
}
```

**Priority**: `engines` \u003e `volta` (if both present, engines wins).

---

## 🛤️ Step 4: Building PATH Additions

Once versions are detected, OMG maps them to binary paths:

```rust
pub fn build_path_additions(versions: &HashMap<String, String>) -> Vec<String> {
    let mut paths = Vec::new();
    let data_dir = paths::data_dir();

    for (runtime, version) in versions {
        let bin_path = match runtime.as_str() {
            "node" => resolve_node_bin_path(&data_dir, version)?,
            "python" => data_dir.join("versions/python").join(version).join("bin"),
            "go" => data_dir.join("versions/go").join(version).join("bin"),
            "ruby" => data_dir.join("versions/ruby").join(version).join("bin"),
            "java" => data_dir.join("versions/java").join(version).join("bin"),
            "bun" => resolve_bun_bin_path(&data_dir, version)?,
            "rust" => {
                // Skip if rustup is installed
                if has_rustup() {
                    continue;
                }
                data_dir.join("versions/rust").join(version).join("bin")
            }
            _ => continue,
        };

        if bin_path.exists() {
            paths.push(bin_path.display().to_string());
        }
    }

    paths
}
```

### Special Cases

#### Node.js: Fallback to NVM

If OMG doesn't have the requested Node version, it checks NVM:

```rust
fn resolve_node_bin_path(data_dir: &Path, version: &str) -> Option<PathBuf> {
    // 1. Check OMG's versions
    let omg_path = data_dir.join("versions/node").join(version).join("bin");
    if omg_path.exists() {
        return Some(omg_path);
    }

    // 2. Fall back to NVM
    nvm_node_bin(version)
}

fn nvm_node_bin(version: &str) -> Option<PathBuf> {
    let nvm_dir = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| home::home_dir().map(|d| d.join(".nvm")))?;

    let bin_path = nvm_dir
        .join("versions/node")
        .join(format!("v{}", version))
        .join("bin");

    if bin_path.exists() {
        Some(bin_path)
    } else {
        None
    }
}
```

**Benefit**: OMG works alongside NVM — you don't have to reinstall all your Node versions.

#### Rust: Defer to rustup

If `rustup` is installed, OMG doesn't add Rust to PATH:

```rust
let has_rustup = home_dir().is_some_and(|h| {
    h.join(".cargo/bin/rustc").exists() || h.join(".rustup").exists()
});

if has_rustup {
    continue; // Let rustup manage Rust
}
```

**Rationale**: `rustup` handles toolchains better than OMG, so defer to it.

---

## 🔄 Step 5: Outputting Shell Code

Finally, OMG outputs the PATH modification:

```rust
match shell {
    "zsh" | "bash" => {
        let additions = path_additions.join(":");
        println!("export PATH=\"{additions}:$PATH\"");
    }
    "fish" => {
        for path in &path_additions {
            println!("fish_add_path -g {path}");
        }
    }
    _ => {}
}
```

### Example Output

For a project with `.nvmrc` (20.10.0) and `.python-version` (3.12.0):

```bash
export PATH="/home/user/.local/share/omg/versions/node/20.10.0/bin:/home/user/.local/share/omg/versions/python/3.12.0/bin:$PATH"
```

The shell hook then runs:

```bash
eval "$(omg hook-env -s zsh)"
```

which executes the above `export`, prepending the correct paths.

---

## ⚡ Performance Optimizations

### 1. Early Exit on No Changes

```rust
if versions.is_empty() {
    return Ok(()); // Don't output anything
}
```

If no version files are found, the hook exits immediately (sub-millisecond).

### 2. Cached Version Detection

The hook doesn't re-detect if you `cd` within the same project:

```bash
$ cd ~/my-app
# (Hook runs, detects .nvmrc)

$ cd ~/my-app/src
# (Hook runs, detects same .nvmrc — no change)

$ cd ~/my-app/src/components
# (Hook runs, detects same .nvmrc — no change)
```

The PATH is only modified when the detected version **changes**.

### 3. Minimal Syscalls

The detection algorithm uses:
- `std::env::current_dir()` — 1 syscall
- `file_path.exists()` — 1 syscall per version file (max ~10)
- `std::fs::read_to_string()` — 1 syscall per matched file

**Total**: ~15 syscalls max, typically \u003c5.

### 4. No Process Spawning

Unlike `asdf` which spawns a process for each runtime, OMG is a **single binary** that handles all runtimes internally.

**Comparison**:
- asdf: ~50ms (spawns `asdf current`)
- OMG: \u003c10ms (single binary)

---

## 🧪 Testing the Hook

### Manual Trigger

You can manually trigger the hook:

```bash
omg hook-env -s zsh
```

This outputs the PATH modification without running it.

### Debug Mode

Add `-v` for verbose output:

```bash
omg -v hook-env -s zsh
```

This shows:
- Detected version files
- Resolved versions
- Binary paths
- Final PATH modification

### Timing

Measure hook performance:

```bash
time omg hook-env -s zsh
```

**Target**: \u003c10ms (imperceptible).

---

## 🐛 Troubleshooting

### Wrong Version Active

**Symptom**: `node --version` doesn't match `.nvmrc`

**Diagnosis**:

```bash
# 1. Check what OMG detects
omg which node

# 2. Check PATH order
echo $PATH | tr ':' '\\n' | head -10

# 3. Force detection
omg hook-env -s zsh

# 4. Check for conflicting tools
which -a node
```

**Common causes**:
- NVM added to PATH after OMG
- Another runtime manager interfering
- Stale shell session (run `exec zsh`)

### Hook Not Running

**Symptom**: Versions don't change when you `cd`

**Diagnosis**:

```bash
# 1. Check hook is registered
type _omg_hook

# 2. Manually trigger
_omg_hook

# 3. Check precmd/chpwd
echo $precmd_functions
echo $chpwd_functions
```

**Common causes**:
- Hook not added to `.zshrc`
- Another tool overwrote `precmd_functions`
- Shell theme disabling hooks

### Slow Directory Changes

**Symptom**: `cd` takes \u003e100ms

**Diagnosis**:

```bash
# 1. Time the hook
time _omg_hook

# 2. Check for network timeouts
strace -c omg hook-env -s zsh

# 3. Disable hook temporarily
unset precmd_functions
unset chpwd_functions
```

**Common causes**:
- Slow disk (HDD)
- Network mount for home directory
- Too many version files in deep tree

---

## 🔧 Advanced: Custom Hook Logic

You can extend the hook with custom logic:

```bash
_omg_hook() {
  trap -- '' SIGINT
  eval "$(omg hook-env -s zsh)"
  trap - SIGINT

  # Custom: Auto-activate Python virtualenv
  if [[ -f .venv/bin/activate ]]; then
    source .venv/bin/activate
  fi

  # Custom: Load direnv if present
  if command -v direnv &>/dev/null && [[ -f .envrc ]]; then
    eval "$(direnv export zsh)"
  fi
}
```

---

## 📚 Implementation References

### Source Files

- **Hook Script Generation**: `src/hooks/mod.rs` (ZSH_HOOK, BASH_HOOK, FISH_HOOK)
- **Version Detection**: `src/hooks/mod.rs` (`detect_versions()`)
- **PATH Building**: `src/hooks/mod.rs` (`build_path_additions()`)
- **Hook Entry Point**: `src/cli/mod.rs` (`hook_env` command)

### Related Docs

- [Shell Integration](./shell-integration.md) — User guide
- [Runtime Management](./runtimes.md) — Runtime installation
- [Configuration](./configuration.md) — Hook settings

---

## 💡 Key Takeaways

1. **Automatic Detection**: No manual `omg use` needed
2. **Directory-Aware**: Walks up tree to find version files
3. **Closest Wins**: Project-level files override global defaults
4. **Fast**: \u003c10ms execution, imperceptible lag
5. **Compatible**: Works alongside NVM, rustup, etc.

This is how OMG makes runtime switching feel like magic — it's just a well-designed shell hook with smart file detection and PATH manipulation.
