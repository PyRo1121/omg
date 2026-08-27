//! Shell hook system for PATH modification
//!
//! Implements fast shell-hook PATH switching for native runtimes.
//! This is the default and fastest method - shims are optional fallback.

pub mod completions;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::paths;
use crate::runtimes::rust::RustToolchainSpec;
use anyhow::{Context, Result};
use semver::{Version, VersionReq};
use serde::Deserialize;

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

fn read_pin_file(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to read version pin file {}", path.display())),
    }
}

fn read_package_json_versions(dir: &Path) -> Result<Option<HashMap<String, String>>> {
    let path = dir.join("package.json");
    let Some(content) = read_pin_file(&path)? else {
        return Ok(None);
    };
    let pkg: PackageJsonVersions = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
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

    Ok((!versions.is_empty()).then_some(versions))
}

/// Print the shell hook script to be added to shell rc file
///
/// Usage: eval "$(omg hook zsh)"
pub fn print_hook(shell: &str) -> Result<()> {
    // SECURITY: Validate shell
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
    // SECURITY: Validate shell
    if !matches!(shell.to_lowercase().as_str(), "zsh" | "bash" | "fish") {
        anyhow::bail!("Unsupported shell: {shell}");
    }

    let cwd = std::env::current_dir()?;

    // Detect version files in current directory and parents
    let versions = detect_versions(&cwd)?;

    if versions.is_empty() {
        return Ok(());
    }

    // Build PATH modifications
    let path_additions = build_path_additions(&versions)?;

    if path_additions.is_empty() {
        return Ok(());
    }

    // Output shell-specific PATH modification
    //
    // SECURITY: each addition is emitted as a POSIX single-quoted word so no
    // component can break out of the assignment via `"`, `$(`, or backticks;
    // `$PATH` stays outside the quoting so it still expands in the shell.
    match shell.to_lowercase().as_str() {
        "zsh" | "bash" => {
            let additions = path_additions
                .iter()
                .map(|path| posix_single_quoted(path))
                .collect::<Vec<_>>()
                .join(":");
            println!("export PATH={additions}:\"$PATH\"");
        }
        "fish" => {
            for path in &path_additions {
                println!("fish_add_path -g {}", fish_single_quoted(path));
            }
        }
        _ => {}
    }

    Ok(())
}

fn parse_tool_versions_file(
    file_path: &Path,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(content) = read_pin_file(file_path)? else {
        return Ok(());
    };
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let (Some(rt_part), Some(ver_part)) = (parts.first(), parts.get(1)) else {
            continue;
        };
        let rt = normalize_runtime_name(rt_part);
        let ver = (*ver_part).to_string();
        versions.entry(rt).or_insert(ver);
    }
    Ok(())
}

fn parse_rust_toolchain_file(
    file_path: &Path,
    runtime: &str,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(content) = read_pin_file(file_path)? else {
        return Ok(());
    };
    for line in content.lines() {
        if line.contains("channel")
            && let Some(version) = line.split('=').nth(1)
        {
            let v = version.trim().trim_matches('"').trim_matches('\'');
            versions.insert(runtime.to_string(), v.to_string());
        }
    }
    Ok(())
}

fn parse_go_mod_file(
    file_path: &Path,
    runtime: &str,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(content) = read_pin_file(file_path)? else {
        return Ok(());
    };
    for line in content.lines() {
        let line = line.trim();
        if let Some(version) = line.strip_prefix("go ")
            && !version.trim().is_empty()
        {
            versions.insert(runtime.to_string(), version.trim().to_string());
            break;
        }
    }
    Ok(())
}

fn parse_simple_version_file(
    file_path: &Path,
    runtime: &str,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(content) = read_pin_file(file_path)? else {
        return Ok(());
    };
    let version = content.trim().trim_start_matches('v').to_string();
    if !version.is_empty() {
        versions.insert(runtime.to_string(), version);
    }
    Ok(())
}

fn try_parse_version_file(
    filename: &str,
    file_path: &Path,
    runtime: &str,
    dir: &Path,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    match filename {
        ".tool-versions" => parse_tool_versions_file(file_path, versions)?,
        "rust-toolchain.toml" => parse_rust_toolchain_file(file_path, runtime, versions)?,
        "package.json" => {
            if let Some(extra) = read_package_json_versions(dir)? {
                for (runtime, version) in extra {
                    versions
                        .entry(runtime)
                        .or_insert_with(|| version.trim().to_string());
                }
            }
        }
        "go.mod" => parse_go_mod_file(file_path, runtime, versions)?,
        _ => parse_simple_version_file(file_path, runtime, versions)?,
    }
    Ok(())
}

/// Detect version files in directory and parents
pub fn detect_versions(start: &Path) -> Result<HashMap<String, String>> {
    let mut versions = HashMap::new();
    let mut current = Some(start.to_path_buf());

    while let Some(dir) = current {
        for (filename, runtime) in VERSION_FILES {
            if versions.contains_key(*runtime) {
                continue;
            }

            let file_path = dir.join(filename);
            if file_path.exists() {
                try_parse_version_file(filename, &file_path, runtime, &dir, &mut versions)?;
            }
        }

        current = dir.parent().map(std::path::Path::to_path_buf);
    }

    Ok(versions)
}

/// Build PATH additions for detected versions
pub fn build_path_additions<S: std::hash::BuildHasher>(
    versions: &HashMap<String, String, S>,
) -> Result<Vec<String>> {
    let mut paths = Vec::new();

    let data_dir = paths::data_dir();

    for (runtime, version) in versions {
        let bin_path = match runtime.as_str() {
            "node" => {
                let Some(path) = resolve_node_bin_path(&data_dir, version)? else {
                    continue;
                };
                path
            }
            // SECURITY: repo-supplied version files (.python-version, .go-version,
            // .ruby-version, .java-version, .tool-versions) are untrusted input.
            // Validate before using as a path component so a hostile pin like
            // `../../evil/bin` can never traverse out of the versions tree and
            // place an attacker-created directory on the shell's PATH.
            "python" | "go" | "ruby" | "java" | "pi" => {
                let Some(path) = validated_runtime_bin_dir(&data_dir, runtime, version) else {
                    continue;
                };
                path
            }
            "bun" => {
                let Some(path) = resolve_bun_bin_path(&data_dir, version)? else {
                    continue;
                };
                path
            }
            "rust" => {
                let Some(toolchain) = RustToolchainSpec::parse(version).ok() else {
                    continue;
                };
                data_dir
                    .join("versions/rust")
                    .join(toolchain.name())
                    .join("bin")
            }
            _ => continue,
        };

        if crate::runtimes::common::is_trusted_runtime_bin_dir(&bin_path) {
            paths.push(bin_path.display().to_string());
        }
    }

    Ok(paths)
}

fn resolve_node_bin_path(data_dir: &Path, version: &str) -> Result<Option<PathBuf>> {
    let normalized = version.trim_start_matches('v');
    let versions_dir = data_dir.join("versions/node");
    if let Some(path) = node_version_bin_path(&versions_dir, normalized) {
        return Ok(Some(path));
    }

    if let Some(resolved) = resolve_installed_version_req(&versions_dir, normalized)?
        && let Some(path) = node_version_bin_path(&versions_dir, &resolved)
    {
        return Ok(Some(path));
    }

    nvm_node_bin(normalized)
}

fn resolve_bun_bin_path(data_dir: &Path, version: &str) -> Result<Option<PathBuf>> {
    let normalized = version.trim_start_matches('v');
    let versions_dir = data_dir.join("versions/bun");
    if let Some(path) = bun_version_bin_path(&versions_dir, normalized) {
        return Ok(Some(path));
    }

    if let Some(resolved) = resolve_installed_version_req(&versions_dir, normalized)?
        && let Some(path) = bun_version_bin_path(&versions_dir, &resolved)
    {
        return Ok(Some(path));
    }

    Ok(None)
}

fn node_version_bin_path(versions_dir: &Path, version: &str) -> Option<PathBuf> {
    crate::core::security::validate_runtime_version(version).ok()?;
    let path = versions_dir.join(version).join("bin");
    crate::runtimes::common::is_trusted_runtime_bin_dir(&path).then_some(path)
}

/// Resolve `<data_dir>/versions/<runtime>/<version>/bin` for the runtimes whose
/// version pins come straight from repo-supplied files. The version string is
/// untrusted input and must pass [`validate_runtime_version`] before it may
/// become a path component; only an existing real directory is returned.
fn validated_runtime_bin_dir(data_dir: &Path, runtime: &str, version: &str) -> Option<PathBuf> {
    crate::core::security::validate_runtime_version(version).ok()?;
    let path = data_dir
        .join("versions")
        .join(runtime)
        .join(version)
        .join("bin");
    crate::runtimes::common::is_trusted_runtime_bin_dir(&path).then_some(path)
}

/// Render `value` as a POSIX single-quoted shell word (`'` becomes `'\''`),
/// so no `$`, backtick, or double-quote inside can alter the emitted command.
fn posix_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Render `value` as a fish single-quoted word (`'` becomes `\'`).
fn fish_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"\'"))
}

fn bun_version_bin_path(versions_dir: &Path, version: &str) -> Option<PathBuf> {
    crate::core::security::validate_runtime_version(version).ok()?;
    let path = versions_dir.join(version);
    crate::runtimes::common::is_trusted_runtime_bin_dir(&path).then_some(path)
}

fn resolve_installed_version_req(versions_dir: &Path, req: &str) -> Result<Option<String>> {
    let Some(req) = normalize_version_req(req) else {
        return Ok(None);
    };
    let mut candidates = Vec::new();

    let entries = match fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to read runtime versions directory {}",
                    versions_dir.display()
                )
            });
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "Failed to read runtime versions directory {}",
                versions_dir.display()
            )
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "current" || !crate::runtimes::common::is_valid_version_dir(&entry.path()) {
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
    Ok(candidates.first().map(|(_, name)| name.clone()))
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

fn nvm_node_bin(version: &str) -> Result<Option<PathBuf>> {
    let Some(nvm_dir) = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| home::home_dir().map(|dir| dir.join(".nvm")))
    else {
        return Ok(None);
    };

    let resolved = match resolve_nvm_alias(&nvm_dir, version)? {
        Some(alias) => alias,
        None => version.to_string(),
    };
    let normalized = resolved.trim_start_matches('v');

    // SECURITY (audit sec14 F1): `version` originates from repo-supplied pin
    // files. Without validation a hostile pin like `../../evil/bin` escapes
    // the nvm versions tree and puts an attacker-controlled directory on the
    // spawned command's PATH. Same contract as validated_runtime_bin_dir.
    if crate::core::security::validate_runtime_version(normalized).is_err() {
        return Ok(None); // hostile pin: never place it on PATH
    }

    let bin_path = nvm_dir
        .join("versions/node")
        .join(format!("v{normalized}"))
        .join("bin");

    // Canonicalize and require the result to stay inside the nvm versions
    // tree even if symlinks redirect it.
    let canonical_base = nvm_dir
        .join("versions/node")
        .canonicalize()
        .unwrap_or_else(|_| nvm_dir.join("versions/node"));
    let Ok(canonical) = bin_path.canonicalize() else {
        return Ok(None);
    };
    if !canonical.starts_with(&canonical_base) {
        return Ok(None);
    }

    Ok(crate::runtimes::common::is_trusted_runtime_bin_dir(&bin_path).then_some(bin_path))
}

fn resolve_nvm_alias(nvm_dir: &Path, alias: &str) -> Result<Option<String>> {
    let relative = Path::new(alias);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Ok(None);
    }

    let alias_root = nvm_dir.join("alias");
    let canonical_root = match alias_root.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("Failed to resolve nvm alias directory"),
    };
    let candidate = alias_root.join(relative);
    let canonical = match candidate.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to resolve nvm alias {}", candidate.display()));
        }
    };
    if !canonical.starts_with(&canonical_root) {
        return Ok(None);
    }
    let Some(content) = read_pin_file(&canonical)? else {
        return Ok(None);
    };
    let resolved = content.trim();
    Ok((!resolved.is_empty()).then(|| resolved.to_string()))
}

// Runtime resolution helpers
// moved to core::runtime_resolver module

/// Get active versions for display
pub fn get_active_versions() -> Result<HashMap<String, String>> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
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
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn hostile_python_pin_never_reaches_path() {
        let data = tempdir().unwrap();
        // The directory a hostile `../..` pin would resolve to really exists,
        // so only the validator (not the existence check) can reject it.
        fs::create_dir_all(data.path().join("bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let versions = HashMap::from([("python".to_string(), "../..".to_string())]);
            let additions = build_path_additions(&versions).unwrap();
            assert!(
                additions.is_empty(),
                "traversal pin must be rejected, got {additions:?}"
            );
        });
    }

    #[test]
    fn all_pinned_runtimes_reject_traversal_versions() {
        let data = tempdir().unwrap();
        fs::create_dir_all(data.path().join("bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            for runtime in ["python", "go", "ruby", "java"] {
                let versions = HashMap::from([(runtime.to_string(), "../..".to_string())]);
                let additions = build_path_additions(&versions).unwrap();
                assert!(
                    additions.is_empty(),
                    "{runtime}: traversal pin must be rejected, got {additions:?}"
                );
            }
        });
    }

    #[test]
    fn valid_python_pin_resolves_to_existing_bin_dir() {
        let data = tempdir().unwrap();
        fs::create_dir_all(data.path().join("versions/python/3.12.0/bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let versions = HashMap::from([("python".to_string(), "3.12.0".to_string())]);
            let additions = build_path_additions(&versions).unwrap();
            assert_eq!(
                additions.len(),
                1,
                "valid pin should resolve: {additions:?}"
            );
            assert!(additions[0].ends_with("bin"));
        });
    }

    #[test]
    fn posix_quoting_neutralizes_metacharacters() {
        assert_eq!(posix_single_quoted("/opt/bin"), "'/opt/bin'");
        assert_eq!(posix_single_quoted("a'b"), "'a'\\''b'");
        let hostile = "$(rm -rf ~)";
        // Inside single quotes nothing expands, so metacharacters are inert
        // verbatim; only embedded single quotes need escaping.
        assert_eq!(posix_single_quoted(hostile), "'$(rm -rf ~)'");
    }

    #[test]
    fn fish_quoting_escapes_embedded_quotes() {
        assert_eq!(fish_single_quoted("/opt/bin"), "'/opt/bin'");
        assert_eq!(fish_single_quoted("a'b"), "'a\\'b'");
    }

    #[test]
    fn test_detect_nvmrc() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".nvmrc"), "20.10.0").unwrap();

        let versions = detect_versions(dir.path()).unwrap();
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

        let versions = detect_versions(dir.path()).unwrap();
        assert_eq!(versions.get("node"), Some(&"20.10.0".to_string()));
        assert_eq!(versions.get("python"), Some(&"3.12.0".to_string()));
    }

    #[test]
    fn test_node_version_priority() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".nvmrc"), "18.19.0").unwrap();
        fs::write(dir.path().join(".node-version"), "20.11.1").unwrap();

        let versions = detect_versions(dir.path()).unwrap();
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

        let versions = detect_versions(dir.path()).unwrap();
        assert_eq!(versions.get("node"), Some(&">=18 <21".to_string()));
        assert_eq!(versions.get("bun"), Some(&"1.1.0".to_string()));
    }

    #[test]
    fn runtime_path_builders_reject_parent_directory_versions() {
        let dir = tempdir().unwrap();
        let versions_dir = dir.path().join("versions/node");
        fs::create_dir_all(dir.path().join("versions/bin")).unwrap();

        assert!(node_version_bin_path(&versions_dir, "..").is_none());
        assert!(bun_version_bin_path(&versions_dir, "..").is_none());
    }

    #[test]
    fn node_requirements_resolve_to_safe_installed_versions() {
        let dir = tempdir().unwrap();
        let expected = dir.path().join("versions/node/20.11.1/bin");
        fs::create_dir_all(&expected).unwrap();

        assert_eq!(
            resolve_node_bin_path(dir.path(), "^20").unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn resolve_installed_version_req_missing_dir_is_none() {
        let dir = tempdir().unwrap();
        assert!(
            resolve_installed_version_req(&dir.path().join("versions/node"), "^20")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn resolve_installed_version_req_unreadable_dir_fails_closed() {
        let dir = tempdir().unwrap();
        let versions_dir = dir.path().join("versions/node");
        fs::create_dir_all(&versions_dir).unwrap();
        let original = fs::metadata(&versions_dir).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&versions_dir, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = fs::read_dir(&versions_dir).is_err();
        let result = resolve_installed_version_req(&versions_dir, "^20");
        let _ = fs::set_permissions(&versions_dir, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable versions directory must fail closed, got {result:?}"
        );
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

        let versions = detect_versions(dir.path()).unwrap();
        assert_eq!(versions.get("deno"), Some(&"1.40.0".to_string()));
        assert_eq!(versions.get("elixir"), Some(&"1.16.0".to_string()));
        assert_eq!(versions.get("zig"), Some(&"0.11.0".to_string()));
    }

    #[test]
    fn test_read_pin_file_missing_is_none() {
        let missing = tempdir().unwrap().path().join("does-not-exist");
        assert!(read_pin_file(&missing).unwrap().is_none());
    }

    #[test]
    fn test_detect_versions_unreadable_pin_errors() {
        let dir = tempdir().unwrap();
        let pin = dir.path().join(".nvmrc");
        fs::write(&pin, "20.10.0").unwrap();
        let original = fs::metadata(&pin).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&pin, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = fs::read_to_string(&pin).is_err();
        let result = detect_versions(dir.path());
        let _ = fs::set_permissions(&pin, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable pin file must fail closed, got {result:?}"
        );
    }

    #[test]
    fn test_detect_versions_invalid_package_json_errors() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "not json").unwrap();
        let error = detect_versions(dir.path()).unwrap_err();
        assert!(
            error.to_string().contains("Failed to parse"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn resolve_nvm_alias_missing_is_none() {
        let dir = tempdir().unwrap();
        assert!(resolve_nvm_alias(dir.path(), "lts").unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_nvm_alias_symlink_cannot_escape_alias_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let alias_dir = dir.path().join("alias");
        fs::create_dir(&alias_dir).unwrap();
        let outside = dir.path().join("outside");
        fs::write(&outside, "20.11.1\n").unwrap();
        symlink(&outside, alias_dir.join("default")).unwrap();

        assert!(resolve_nvm_alias(dir.path(), "default").unwrap().is_none());
        assert!(
            resolve_nvm_alias(dir.path(), "../outside")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn resolve_nvm_alias_unreadable_fails_closed() {
        let dir = tempdir().unwrap();
        let alias_dir = dir.path().join("alias");
        fs::create_dir(&alias_dir).unwrap();
        let alias = alias_dir.join("lts");
        fs::write(&alias, "20.11.1\n").unwrap();
        let original = fs::metadata(&alias).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&alias, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = fs::read_to_string(&alias).is_err();
        let result = resolve_nvm_alias(dir.path(), "lts");
        let _ = fs::set_permissions(&alias, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable nvm alias must fail closed, got {result:?}"
        );
    }
    #[cfg(test)]
    mod nvm_path_traversal_tests {
        use super::*;

        #[test]
        fn hostile_nvm_pin_cannot_escape_versions_tree() {
            // Audit sec14 F1: a repo-supplied pin traversing out of the nvm
            // versions tree must never reach PATH.
            for hostile in [
                "../../evil/bin",
                "../../../usr/local/bin",
                "v8.0.0/../../../evil",
                "..",
            ] {
                let result = nvm_node_bin(hostile).expect("nvm probe must not error");
                assert!(result.is_none(), "hostile pin {hostile:?} must be refused");
            }
        }
    }
}
