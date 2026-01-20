//! Shell hook system for PATH modification
//!
//! Implements the fast shell hook approach (like mise) for version switching.
//! This is the default and fastest method - shims are optional fallback.

pub mod completions;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use semver::{Version, VersionReq};
use serde::Deserialize;
use toml::Value;

use crate::config::Settings;
use crate::core::{RuntimeBackend, paths};
use crate::runtimes::rust::RustToolchainSpec;

/// Known version files and their corresponding runtime
const VERSION_FILES: &[(&str, &str)] = &[
    // Node.js
    (".node-version", "node"),
    (".nvmrc", "node"),
    // Python
    (".python-version", "python"),
    // Ruby
    (".ruby-version", "ruby"),
    // Go
    (".go-version", "go"),
    ("go.mod", "go"),
    // Java
    (".java-version", "java"),
    // Bun
    (".bun-version", "bun"),
    // Rust
    ("rust-toolchain", "rust"),
    ("rust-toolchain.toml", "rust"),
    // Universal
    (".tool-versions", "multi"),
    (".mise.toml", "multi"),
    (".mise.local.toml", "multi"),
    ("mise.toml", "multi"),
    ("package.json", "multi"),
];

/// Normalize runtime name aliases to canonical names
fn normalize_runtime_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "nodejs" | "node" => "node".to_string(),
        "bun" | "bunjs" => "bun".to_string(),
        "python3" | "python" => "python".to_string(),
        "golang" | "go" => "go".to_string(),
        "rustlang" | "rust" => "rust".to_string(),
        other => other.to_string(),
    }
}

#[derive(Deserialize)]
struct PackageJsonVersions {
    engines: Option<PackageEngines>,
    volta: Option<VoltaToolchain>,
}

#[derive(Deserialize)]
struct PackageEngines {
    node: Option<String>,
    bun: Option<String>,
}

#[derive(Deserialize)]
struct VoltaToolchain {
    node: Option<String>,
    bun: Option<String>,
}

#[derive(Deserialize)]
struct MiseConfig {
    tools: Option<HashMap<String, Value>>,
}

fn read_package_json_versions(dir: &Path) -> Option<HashMap<String, String>> {
    let file = std::fs::File::open(dir.join("package.json")).ok()?;
    let pkg: PackageJsonVersions = serde_json::from_reader(file).ok()?;
    let mut versions = HashMap::new();

    // Process volta first (lower priority)
    if let Some(volta) = pkg.volta {
        if let Some(node) = volta.node {
            versions.insert("node".to_string(), node);
        }
        if let Some(bun) = volta.bun {
            versions.insert("bun".to_string(), bun);
        }
    }

    // Process engines second (higher priority - overwrites volta)
    if let Some(engines) = pkg.engines {
        if let Some(node) = engines.node {
            versions.insert("node".to_string(), node);
        }
        if let Some(bun) = engines.bun {
            versions.insert("bun".to_string(), bun);
        }
    }

    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}

fn read_mise_versions(path: &Path) -> Option<HashMap<String, String>> {
    let content = fs::read_to_string(path).ok()?;
    let config: MiseConfig = toml::from_str(&content).ok()?;
    let tools = config.tools?;
    let mut versions = HashMap::new();

    for (tool, value) in tools {
        if let Some(version) = mise_tool_version(&value) {
            let normalized = normalize_runtime_name(&tool);
            versions.insert(normalized, version);
        }
    }

    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}

fn mise_tool_version(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => items
            .iter()
            .find_map(|entry| entry.as_str().map(std::string::ToString::to_string)),
        Value::Table(table) => table
            .get("version")
            .and_then(|entry| entry.as_str().map(std::string::ToString::to_string)),
        _ => None,
    }
}

/// Print the shell hook script to be added to shell rc file
///
/// Usage: eval "$(omg hook zsh)"
pub fn print_hook(shell: &str) -> Result<()> {
    let script = match shell.to_lowercase().as_str() {
        "zsh" => ZSH_HOOK,
        "bash" => BASH_HOOK,
        "fish" => FISH_HOOK,
        _ => {
            anyhow::bail!("Unsupported shell: {shell}. Supported: zsh, bash, fish");
        }
    };

    println!("{script}");
    Ok(())
}

/// Called by shell hook on directory change to update PATH
///
/// This is the fast path - only outputs changes when version changes.
/// Target: <10ms execution time
pub fn hook_env(shell: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Detect version files in current directory and parents
    let versions = detect_versions(&cwd);

    if versions.is_empty() {
        return Ok(());
    }

    // Build PATH modifications
    let settings = Settings::load()?;
    let path_additions = build_path_additions_with_backend(&versions, settings.runtime_backend);

    if path_additions.is_empty() {
        return Ok(());
    }

    // Output shell-specific PATH modification
    match shell.to_lowercase().as_str() {
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

/// Detect version files in directory and parents
#[must_use]
pub fn detect_versions(start: &Path) -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let mut current = Some(start.to_path_buf());

    // Walk up directory tree
    while let Some(dir) = current {
        for (filename, runtime) in VERSION_FILES {
            if versions.contains_key(*runtime) {
                continue; // Already found closer version
            }

            let file_path = dir.join(filename);
            if file_path.exists() {
                if *filename == ".tool-versions" {
                    // Parse asdf-style .tool-versions
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        for line in content.lines() {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if let (Some(rt_part), Some(ver_part)) = (parts.first(), parts.get(1)) {
                                let rt = normalize_runtime_name(rt_part);
                                let ver = (*ver_part).to_string();
                                versions.entry(rt).or_insert(ver);
                            }
                        }
                    }
                } else if *filename == "rust-toolchain.toml" {
                    // Parse TOML format
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        for line in content.lines() {
                            if line.contains("channel")
                                && let Some(version) = line.split('=').nth(1)
                            {
                                let v = version.trim().trim_matches('"').trim_matches('\'');
                                versions.insert((*runtime).to_string(), v.to_string());
                            }
                        }
                    }
                } else if *filename == "package.json" {
                    if let Some(extra) = read_package_json_versions(&dir) {
                        for (runtime, version) in extra {
                            versions
                                .entry(runtime)
                                .or_insert_with(|| version.trim().to_string());
                        }
                    }
                } else if *filename == "go.mod" {
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        for line in content.lines() {
                            let line = line.trim();
                            if let Some(version) = line.strip_prefix("go ") {
                                let version = version.trim();
                                if !version.is_empty() {
                                    versions.insert((*runtime).to_string(), version.to_string());
                                    break;
                                }
                            }
                        }
                    }
                } else if *filename == ".mise.toml"
                    || *filename == ".mise.local.toml"
                    || *filename == "mise.toml"
                {
                    if let Some(extra) = read_mise_versions(&file_path) {
                        for (runtime, version) in extra {
                            versions
                                .entry(runtime)
                                .or_insert_with(|| version.trim().to_string());
                        }
                    }
                } else {
                    // Simple version file
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        let version = content.trim().trim_start_matches('v').to_string();
                        if !version.is_empty() {
                            versions.insert((*runtime).to_string(), version);
                        }
                    }
                }
            }
        }

        current = dir.parent().map(std::path::Path::to_path_buf);
    }

    versions
}

/// Build PATH additions for detected versions
#[must_use]
pub fn build_path_additions<S: std::hash::BuildHasher>(
    versions: &HashMap<String, String, S>,
) -> Vec<String> {
    let mut paths = Vec::new();

    let data_dir = paths::data_dir();

    for (runtime, version) in versions {
        let bin_path = match runtime.as_str() {
            "node" => match resolve_node_bin_path(&data_dir, version) {
                Some(path) => path,
                None => continue,
            },
            "python" => data_dir.join("versions/python").join(version).join("bin"),
            "go" => data_dir.join("versions/go").join(version).join("bin"),
            "ruby" => data_dir.join("versions/ruby").join(version).join("bin"),
            "java" => data_dir.join("versions/java").join(version).join("bin"),
            "bun" => match resolve_bun_bin_path(&data_dir, version) {
                Some(path) => path,
                None => continue,
            },
            "rust" => {
                // Skip if rustup is installed - let rustup manage Rust
                // Check for both rustc and cargo to be thorough
                let home = dirs::home_dir();
                let has_rustup = home.as_ref()
                    .map(|h| h.join(".cargo/bin/rustc").exists() || h.join(".rustup").exists())
                    .unwrap_or(false);
                if has_rustup {
                    // Rustup is installed, don't add OMG-managed Rust to PATH
                    continue;
                }
                let toolchain = RustToolchainSpec::parse(version)
                    .ok()
                    .map_or_else(|| version.clone(), |spec| spec.name());
                data_dir.join("versions/rust").join(toolchain).join("bin")
            }
            _ => continue,
        };

        if bin_path.exists() {
            paths.push(bin_path.display().to_string());
        }
    }

    paths
}

/// Build PATH additions for detected versions with backend preference
#[must_use]
pub fn build_path_additions_with_backend<S: std::hash::BuildHasher>(
    versions: &HashMap<String, String, S>,
    backend: RuntimeBackend,
) -> Vec<String> {
    let mut paths = match backend {
        RuntimeBackend::Mise => Vec::new(),
        _ => build_path_additions(versions),
    };

    if matches!(
        backend,
        RuntimeBackend::Mise | RuntimeBackend::NativeThenMise
    ) {
        let prefer_native = backend == RuntimeBackend::NativeThenMise;
        add_mise_path_fallbacks(versions, &mut paths, prefer_native);
    }

    paths
}

fn resolve_node_bin_path(data_dir: &Path, version: &str) -> Option<PathBuf> {
    let normalized = version.trim_start_matches('v');
    let versions_dir = data_dir.join("versions/node");
    if let Some(path) = node_version_bin_path(&versions_dir, normalized) {
        return Some(path);
    }

    if let Some(resolved) = resolve_installed_version_req(&versions_dir, normalized)
        && let Some(path) = node_version_bin_path(&versions_dir, &resolved)
    {
        return Some(path);
    }

    nvm_node_bin(normalized)
}

fn resolve_bun_bin_path(data_dir: &Path, version: &str) -> Option<PathBuf> {
    let normalized = version.trim_start_matches('v');
    let versions_dir = data_dir.join("versions/bun");
    if let Some(path) = bun_version_bin_path(&versions_dir, normalized) {
        return Some(path);
    }

    if let Some(resolved) = resolve_installed_version_req(&versions_dir, normalized)
        && let Some(path) = bun_version_bin_path(&versions_dir, &resolved)
    {
        return Some(path);
    }

    None
}

fn node_version_bin_path(versions_dir: &Path, version: &str) -> Option<PathBuf> {
    let path = versions_dir.join(version).join("bin");
    if path.exists() { Some(path) } else { None }
}

fn bun_version_bin_path(versions_dir: &Path, version: &str) -> Option<PathBuf> {
    let path = versions_dir.join(version);
    if path.exists() { Some(path) } else { None }
}

fn resolve_installed_version_req(versions_dir: &Path, req: &str) -> Option<String> {
    let req = normalize_version_req(req)?;
    let mut candidates = Vec::new();

    let entries = fs::read_dir(versions_dir).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "current" {
            continue;
        }
        let ver_str = name.trim_start_matches('v');
        let Ok(version) = Version::parse(ver_str) else {
            continue;
        };
        if req.matches(&version) {
            candidates.push((version, ver_str.to_string()));
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.first().map(|(_, name)| name.clone())
}

fn normalize_version_req(value: &str) -> Option<VersionReq> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);

    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return VersionReq::parse(&format!("^{trimmed}.0.0")).ok();
    }

    if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let normalized = normalize_version_number(trimmed);
        return VersionReq::parse(&format!("={normalized}")).ok();
    }

    VersionReq::parse(&trimmed.replace(' ', ",")).ok()
}

fn normalize_version_number(value: &str) -> String {
    let mut parts: Vec<&str> = value.split('.').filter(|p| !p.is_empty()).collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    parts.truncate(3);
    parts.join(".")
}

fn nvm_node_bin(version: &str) -> Option<PathBuf> {
    let nvm_dir = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| home::home_dir().map(|dir| dir.join(".nvm")))?;

    let resolved = resolve_nvm_alias(&nvm_dir, version).unwrap_or_else(|| version.to_string());
    let normalized = resolved.trim_start_matches('v');
    let bin_path = nvm_dir
        .join("versions/node")
        .join(format!("v{normalized}"))
        .join("bin");

    if bin_path.exists() {
        Some(bin_path)
    } else {
        None
    }
}

fn resolve_nvm_alias(nvm_dir: &Path, alias: &str) -> Option<String> {
    let alias_path = nvm_dir.join("alias").join(alias);
    if !alias_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(alias_path).ok()?;
    let resolved = content.trim();
    if resolved.is_empty() {
        None
    } else {
        Some(resolved.to_string())
    }
}

fn add_mise_path_fallbacks<S: std::hash::BuildHasher>(
    versions: &HashMap<String, String, S>,
    path_additions: &mut Vec<String>,
    prefer_native: bool,
) {
    if !mise_available() {
        return;
    }

    let mut seen: std::collections::HashSet<String> = path_additions.iter().cloned().collect();
    for (runtime, version) in versions {
        if prefer_native && native_runtime_bin_path(runtime, version).is_some() {
            continue;
        }

        if let Some(bin_dir) = mise_runtime_bin_path(runtime, version) {
            let bin = bin_dir.display().to_string();
            if seen.insert(bin.clone()) {
                path_additions.push(bin);
            }
        }
    }
}

fn native_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let data_dir = paths::data_dir();
    let bin_path = match runtime {
        "node" => resolve_node_bin_path(&data_dir, version)?,
        "python" => data_dir.join("versions/python").join(version).join("bin"),
        "go" => data_dir.join("versions/go").join(version).join("bin"),
        "ruby" => data_dir.join("versions/ruby").join(version).join("bin"),
        "java" => data_dir.join("versions/java").join(version).join("bin"),
        "bun" => resolve_bun_bin_path(&data_dir, version)?,
        "rust" => {
            let toolchain = RustToolchainSpec::parse(version)
                .ok()
                .map_or_else(|| version.to_string(), |spec| spec.name());
            data_dir.join("versions/rust").join(toolchain).join("bin")
        }
        _ => return None,
    };

    if bin_path.exists() {
        Some(bin_path)
    } else {
        None
    }
}

fn mise_available() -> bool {
    find_in_path("mise").is_some()
}

fn mise_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let tool_spec = if version.is_empty() {
        runtime.to_string()
    } else {
        format!("{runtime}@{version}")
    };

    let output = Command::new("mise")
        .args(["where", &tool_spec])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let install_dir = PathBuf::from(stdout.trim());
    if install_dir.as_os_str().is_empty() {
        return None;
    }

    let bin_dir = install_dir.join("bin");
    if bin_dir.exists() {
        Some(bin_dir)
    } else if install_dir.exists() {
        Some(install_dir)
    } else {
        None
    }
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Get active versions for display
#[must_use]
pub fn get_active_versions() -> HashMap<String, String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    detect_versions(&cwd)
}

/// Zsh hook script
const ZSH_HOOK: &str = r#"
# OMG Shell Hook for Zsh
# Add to ~/.zshrc: eval "$(omg hook zsh)"

_omg_hook() {
  trap -- '' SIGINT
  eval "$(\command omg hook-env -s zsh)"
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

# ═══════════════════════════════════════════════════════════════════════════════
# ULTRA-FAST PACKAGE QUERIES (10-50x faster than pacman!)
#
# Two modes:
#   1. CACHED (instant): omg-ec uses $_OMG_EXPLICIT - sub-microsecond
#   2. FRESH (fast): omg-explicit-count reads file - ~1ms
#
# The cache is refreshed every 60 seconds by the prompt hook.
# ═══════════════════════════════════════════════════════════════════════════════

# Cached values (refreshed by _omg_refresh_cache)
typeset -g _OMG_TOTAL=0
typeset -g _OMG_EXPLICIT=0
typeset -g _OMG_ORPHANS=0
typeset -g _OMG_UPDATES=0
typeset -g _OMG_CACHE_TIME=0

# Refresh cache from status file (called by prompt hook)
_omg_refresh_cache() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || return
  local now=$EPOCHSECONDS
  # Only refresh every 60 seconds
  (( now - _OMG_CACHE_TIME < 60 )) && return
  _OMG_CACHE_TIME=$now
  # Read all values at once
  local data=$(od -An -j8 -N16 -tu4 "$f" 2>/dev/null)
  read _OMG_TOTAL _OMG_EXPLICIT _OMG_ORPHANS _OMG_UPDATES <<< "$data"
}

# INSTANT access (sub-microsecond) - uses cached values
omg-ec() { echo ${_OMG_EXPLICIT:-0}; }
omg-tc() { echo ${_OMG_TOTAL:-0}; }
omg-oc() { echo ${_OMG_ORPHANS:-0}; }
omg-uc() { echo ${_OMG_UPDATES:-0}; }

# Fresh read (~1ms) - reads file directly, 10x faster than pacman
omg-explicit-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { command omg explicit --count; return; }
  od -An -j12 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-total-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j8 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-orphan-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j16 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-updates-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j20 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}

# Initialize cache on shell startup
_omg_refresh_cache
"#;

/// Bash hook script
const BASH_HOOK: &str = r#"
# OMG Shell Hook for Bash
# Add to ~/.bashrc: eval "$(omg hook bash)"

_omg_hook() {
  local previous_exit_status=$?
  trap -- '' SIGINT
  eval "$(\command omg hook-env -s bash)"
  trap - SIGINT
  return $previous_exit_status
}

if [[ ! "${PROMPT_COMMAND:-}" =~ _omg_hook ]]; then
  PROMPT_COMMAND="_omg_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# ULTRA-FAST PACKAGE QUERIES (10x+ faster than pacman!)
#
# Functions:
#   omg-ec / omg-explicit-count  - explicit package count
#   omg-tc / omg-total-count     - total package count
#   omg-oc / omg-orphan-count    - orphan package count
#   omg-uc / omg-updates-count   - available updates count
# ═══════════════════════════════════════════════════════════════════════════════

omg-explicit-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { command omg explicit --count; return; }
  od -An -j12 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-total-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j8 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-orphan-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j16 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-updates-count() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || { echo 0; return; }
  od -An -j20 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}

alias omg-ec='omg-explicit-count'
alias omg-tc='omg-total-count'
alias omg-oc='omg-orphan-count'
alias omg-uc='omg-updates-count'
"#;

/// Fish hook script
const FISH_HOOK: &str = r"
# OMG Shell Hook for Fish
# Add to ~/.config/fish/config.fish: omg hook fish | source

function _omg_hook --on-variable PWD --on-event fish_prompt
  omg hook-env -s fish | source
end
";

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_nvmrc() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".nvmrc"), "20.10.0").unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("node"), Some(&"20.10.0".to_string()));
    }

    #[test]
    fn test_detect_tool_versions() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".tool-versions"),
            "node 20.10.0\npython 3.12.0\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("node"), Some(&"20.10.0".to_string()));
        assert_eq!(versions.get("python"), Some(&"3.12.0".to_string()));
    }

    #[test]
    fn test_node_version_priority() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".nvmrc"), "18.19.0").unwrap();
        fs::write(dir.path().join(".node-version"), "20.11.1").unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("node"), Some(&"20.11.1".to_string()));
    }

    #[test]
    fn test_package_json_engines_and_volta() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "engines": { "node": ">=18 <21", "bun": "1.1.0" },
  "volta": { "node": "20.12.0" }
}"#,
        )
        .unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("node"), Some(&">=18 <21".to_string()));
        assert_eq!(versions.get("bun"), Some(&"1.1.0".to_string()));
    }

    #[test]
    fn test_mise_toml_tools() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".mise.toml"),
            r#"[tools]
node = "20.10.0"
bun = "1.0.25"
python = "3.12.1"
"#,
        )
        .unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("node"), Some(&"20.10.0".to_string()));
        assert_eq!(versions.get("bun"), Some(&"1.0.25".to_string()));
        assert_eq!(versions.get("python"), Some(&"3.12.1".to_string()));
    }

    #[test]
    fn test_mise_toml_non_native_runtimes() {
        // Test that mise.toml detects runtimes we don't have native support for
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".mise.toml"),
            r#"[tools]
deno = "1.40.0"
elixir = "1.16.0"
zig = "0.11.0"
swift = "5.9"
erlang = "26.2"
"#,
        )
        .unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("deno"), Some(&"1.40.0".to_string()));
        assert_eq!(versions.get("elixir"), Some(&"1.16.0".to_string()));
        assert_eq!(versions.get("zig"), Some(&"0.11.0".to_string()));
        assert_eq!(versions.get("swift"), Some(&"5.9".to_string()));
        assert_eq!(versions.get("erlang"), Some(&"26.2".to_string()));
    }

    #[test]
    fn test_tool_versions_non_native_runtimes() {
        // Test that .tool-versions detects runtimes we don't have native support for
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".tool-versions"),
            "deno 1.40.0\nelixir 1.16.0\nzig 0.11.0\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path());
        assert_eq!(versions.get("deno"), Some(&"1.40.0".to_string()));
        assert_eq!(versions.get("elixir"), Some(&"1.16.0".to_string()));
        assert_eq!(versions.get("zig"), Some(&"0.11.0".to_string()));
    }
}
