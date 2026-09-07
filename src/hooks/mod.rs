//! Shell hook system for PATH modification
//!
//! Implements fast shell-hook PATH switching for native runtimes.
//! This is the default and fastest method - shims are optional fallback.

pub mod completions;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::paths;
use crate::runtimes::rust::{RustManager, RustToolchainSpec};
use anyhow::{Context, Result};
use semver::{Version, VersionReq};
use serde::Deserialize;

/// Known version files and their corresponding runtime
const VERSION_FILES: &[(&str, &str)] = &[
    // Node.js
    (".node-version", "node"),
    (".nvmrc", "node"),
    // Python
    // Same-directory priority: `.python-version` is listed before
    // `pyproject.toml`, so `detect_versions` sees it first and the explicit
    // pin file wins over the weaker `[project] requires-python` specifier.
    (".python-version", "python"),
    ("pyproject.toml", "python"),
    // Ruby
    (".ruby-version", "ruby"),
    // Go
    (".go-version", "go"),
    ("go.mod", "go"),
    // Java
    (".java-version", "java"),
    // Bun
    (".bun-version", "bun"),
    // Deno
    (".deno-version", "deno"),
    (".dvmrc", "deno"),
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

#[derive(Deserialize)]
struct PyProjectToml {
    project: Option<PyProjectSection>,
}

#[derive(Deserialize)]
struct PyProjectSection {
    #[serde(rename = "requires-python")]
    requires_python: Option<String>,
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
    let status_path = paths::fast_status_path().to_string_lossy().into_owned();
    let quoted_status_path = match shell.to_lowercase().as_str() {
        "fish" => fish_single_quoted(&status_path),
        _ => posix_single_quoted(&status_path),
    };

    println!(
        "{}",
        script.replace("__OMG_STATUS_FILE__", &quoted_status_path)
    );
    Ok(())
}

/// Lines the shell integration owns inside rc files. `remove_hook` deletes
/// exactly these (plus the `# OMG Package Manager` marker) and nothing else.
fn hook_lines(shell: &str) -> Vec<String> {
    match shell.to_lowercase().as_str() {
        "fish" => vec!["omg hook fish | source".to_string()],
        other => vec![format!("eval \"$(omg hook {other})\"")],
    }
}

fn rc_file_for_shell(shell: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().context("No home directory for shell hook removal")?;
    let relative = match shell.to_lowercase().as_str() {
        "bash" => ".bashrc",
        "zsh" => ".zshrc",
        "fish" => ".config/fish/config.fish",
        _ => anyhow::bail!("Unsupported shell: {shell}. Supported: zsh, bash, fish"),
    };
    Ok(home.join(relative))
}

/// Remove OMG shell integration lines from the shell's rc file, keeping a
/// `.omg-backup` copy. Returns whether anything was removed.
pub fn remove_hook(shell: &str) -> Result<bool> {
    let rc = rc_file_for_shell(shell)?;
    let Ok(content) = fs::read_to_string(&rc) else {
        return Ok(false);
    };
    let owned = hook_lines(shell);
    let kept: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != "# OMG Package Manager" && !owned.iter().any(|own| own == trimmed)
        })
        .collect();
    if kept.len() == content.lines().count() {
        return Ok(false);
    }
    let backup = rc.with_file_name(format!(
        "{}.omg-backup",
        rc.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    fs::copy(&rc, &backup)
        .with_context(|| format!("Failed to back up {} to {}", rc.display(), backup.display()))?;
    let mut rewritten = kept.join("\n");
    if content.ends_with('\n') {
        rewritten.push('\n');
    }
    crate::core::safe_ops::atomic_write_file_sync(&rc, rewritten)
        .with_context(|| format!("Failed to rewrite {}", rc.display()))?;
    Ok(true)
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
    let versions = detect_versions_for_hook(&cwd);

    // Build PATH modifications. An empty result is meaningful to the
    // generated hooks, which reset PATH to the user's base PATH first.
    let path_additions = build_path_additions(&versions)?;

    if path_additions.is_empty() {
        return Ok(());
    }

    // Output shell-specific PATH modification
    //
    // SECURITY: each addition is emitted as a POSIX single-quoted word so no
    // component can break out of the assignment via `"`, `$(`, or backticks;
    // The generated hooks reset PATH before evaluating this output. The
    // fallback keeps direct `eval "$(omg hook-env ...)"` calls safe too.
    match shell.to_lowercase().as_str() {
        "zsh" | "bash" => {
            let additions = path_additions
                .iter()
                .map(|path| posix_single_quoted(path))
                .collect::<Vec<_>>()
                .join(":");
            println!("export PATH={additions}:\"${{_OMG_PATH_BASE:-$PATH}}\"");
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

/// Detect version files for the shell hook, degrading gracefully when the
/// current directory contains a malformed pin.
///
/// Ancestor directories are already isolated inside `detect_versions`, but a
/// pin in the start directory itself is a hard error there. Running this on
/// every shell prompt would make one bad `.nvmrc`/`package.json` turn every
/// prompt into a failing `omg hook-env` while PATH silently resets to the
/// user's base PATH. Instead: skip the bad pin, warn once per process (same
/// pattern as the deprecation notice in `config/settings.rs`), and keep the
/// rest of the environment working.
fn detect_versions_for_hook(cwd: &Path) -> HashMap<String, String> {
    match detect_versions(cwd) {
        Ok(versions) => versions,
        Err(error) => {
            use std::sync::atomic::{AtomicBool, Ordering};
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!("Ignoring invalid runtime pin in current directory: {error:#}");
            }
            HashMap::new()
        }
    }
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
    let request = RustManager::parse_toolchain_content(file_path, &content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;
    anyhow::ensure!(
        !request.channel.trim().is_empty(),
        "Rust toolchain channel must not be empty in {}",
        file_path.display()
    );
    versions.insert(runtime.to_string(), request.channel);
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

fn parse_pyproject_requires_python(
    file_path: &Path,
    versions: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(content) = read_pin_file(file_path)? else {
        return Ok(());
    };
    let pyproject: PyProjectToml = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;
    // Keep the specifier raw (`>=3.12`, `~=3.12.1`, …): it is a version
    // request, not an exact pin, and the PATH resolver maps it against the
    // installed version tree.
    if let Some(requires_python) = pyproject
        .project
        .and_then(|project| project.requires_python)
        && !requires_python.trim().is_empty()
    {
        versions.insert("python".to_string(), requires_python);
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
        "pyproject.toml" => parse_pyproject_requires_python(file_path, versions)?,
        _ => parse_simple_version_file(file_path, runtime, versions)?,
    }
    Ok(())
}

/// Detect version files in directory and parents
pub fn detect_versions(start: &Path) -> Result<HashMap<String, String>> {
    let mut versions = HashMap::new();
    let mut current = Some(start.to_path_buf());
    let mut is_start_directory = true;

    while let Some(dir) = current {
        for (filename, runtime) in VERSION_FILES {
            if versions.contains_key(*runtime) {
                continue;
            }

            let file_path = dir.join(filename);
            if file_path.exists() {
                let previous_versions = versions.clone();
                if let Err(error) =
                    try_parse_version_file(filename, &file_path, runtime, &dir, &mut versions)
                {
                    versions = previous_versions;
                    if is_start_directory {
                        return Err(error);
                    }
                    tracing::warn!(
                        "Ignoring invalid ancestor runtime pin {}: {error:#}",
                        file_path.display()
                    );
                }
            }
        }

        current = dir.parent().map(std::path::Path::to_path_buf);
        is_start_directory = false;
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
            "python" | "go" | "ruby" | "java" | "pi" | "deno" => {
                let Some(path) = resolve_runtime_bin_dir(&data_dir, runtime, version)? else {
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

/// Resolve a canonical runtime's raw request to an installed version `bin`
/// directory.
///
/// One generic resolver for the runtimes whose pins come straight from
/// repo-supplied files: the request is mapped onto
/// `<data_dir>/versions/<runtime>/<version>/bin`, first by a safe exact
/// lookup and then by the newest installed semver match. Nothing that fails
/// validation or does not exist is ever returned.
fn resolve_runtime_bin_dir(
    data_dir: &Path,
    runtime: &str,
    raw_request: &str,
) -> Result<Option<PathBuf>> {
    // Java installs by feature number: a request like `21.0` names the same
    // JDK `21` directory, while non-feature requests (for example `21.0.5`)
    // name nothing this runtime tree can contain. Fail soft in PATH
    // building: such a pin skips the runtime instead of failing the hook.
    let request = if runtime == "java" {
        match crate::runtimes::java::java_feature_number(raw_request) {
            Ok(feature) => feature,
            Err(_) => return Ok(None),
        }
    } else {
        raw_request.to_string()
    };

    // Safe exact lookup: the request itself must be a validated, existing
    // version directory before it may reach PATH.
    if let Some(path) = validated_runtime_bin_dir(data_dir, runtime, &request) {
        return Ok(Some(path));
    }

    let versions_dir = data_dir.join("versions").join(runtime);
    let Some(resolved) = resolve_installed_version_req(&versions_dir, &request)? else {
        return Ok(None);
    };
    Ok(validated_runtime_bin_dir(data_dir, runtime, &resolved))
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
        // A bare two-component request (`3.12`) is a compatible patch-range
        // pin (`~3.12.0`), not the exact patch-level `=3.12.0`; a full
        // three-component request stays exact.
        let op = if trimmed.split('.').filter(|part| !part.is_empty()).count() == 2 {
            '~'
        } else {
            '='
        };
        return VersionReq::parse(&format!("{op}{normalized}")).ok();
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

zmodload zsh/datetime

_omg_hook() {
  trap -- '' SIGINT
  if [[ -z "${_OMG_PATH_BASE+x}" ]]; then _OMG_PATH_BASE=$PATH; fi
  export PATH="$_OMG_PATH_BASE"
  eval "$(\command omg hook-env -s zsh)"
  _omg_refresh_cache
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
_omg_status_file_valid() {
  local f=__OMG_STATUS_FILE__
  [[ -f "$f" && ! -L "$f" && -O "$f" ]] || return 1
  [[ "$(wc -c < "$f" 2>/dev/null)" -eq 32 ]] || return 1
  [[ "$(od -An -N4 -tu4 "$f" 2>/dev/null)" -eq 1330464595 ]] || return 1
  [[ "$(od -An -j4 -N1 -tu1 "$f" 2>/dev/null)" -eq 1 ]] || return 1
  local timestamp=$(od -An -j24 -N8 -tu8 "$f" 2>/dev/null)
  local now=$EPOCHSECONDS
  (( now < timestamp || now - timestamp <= 300 ))
}

_omg_refresh_cache() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || return
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
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { command omg explicit --count; return; }
  od -An -j12 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-total-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
  od -An -j8 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-orphan-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
  od -An -j16 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-updates-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
  od -An -j20 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}

# Initialize cache on shell startup
_omg_refresh_cache

# ~/.zfunc is not on a default omz fpath; prepend both so tab-complete finds _omg.
if (( $+functions[compdef] )); then
  fpath=("$HOME/.oh-my-zsh/completions" "$HOME/.zfunc" $fpath)
  autoload -Uz _omg
  compdef _omg omg
fi
"#;

/// Bash hook script
const BASH_HOOK: &str = r#"
# OMG Shell Hook for Bash
# Add to ~/.bashrc: eval "$(omg hook bash)"

_omg_hook() {
  local previous_exit_status=$?
  trap -- '' SIGINT
  if [[ -z "${_OMG_PATH_BASE+x}" ]]; then _OMG_PATH_BASE=$PATH; fi
  export PATH="$_OMG_PATH_BASE"
  eval "$(\command omg hook-env -s bash)"
  trap - SIGINT
  return $previous_exit_status
}

case "$(declare -p PROMPT_COMMAND 2>/dev/null)" in
  "declare -a"*)
    _omg_prompt_hook_present=false
    for _omg_prompt_command in "${PROMPT_COMMAND[@]}"; do
      if [[ "$_omg_prompt_command" == *"_omg_hook"* ]]; then
        _omg_prompt_hook_present=true
        break
      fi
    done
    if [[ "$_omg_prompt_hook_present" == false ]]; then
      PROMPT_COMMAND=("_omg_hook" "${PROMPT_COMMAND[@]}")
    fi
    unset _omg_prompt_command _omg_prompt_hook_present
    ;;
  *)
    if [[ ! "${PROMPT_COMMAND:-}" =~ _omg_hook ]]; then
      PROMPT_COMMAND="_omg_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
    fi
    ;;
esac

# ═══════════════════════════════════════════════════════════════════════════════
# ULTRA-FAST PACKAGE QUERIES (10x+ faster than pacman!)
#
# Functions:
#   omg-ec / omg-explicit-count  - explicit package count
#   omg-tc / omg-total-count     - total package count
#   omg-oc / omg-orphan-count    - orphan package count
#   omg-uc / omg-updates-count   - available updates count
# ═══════════════════════════════════════════════════════════════════════════════

_omg_status_file_valid() {
  local f=__OMG_STATUS_FILE__
  [[ -f "$f" && ! -L "$f" && -O "$f" ]] || return 1
  [[ "$(wc -c < "$f" 2>/dev/null)" -eq 32 ]] || return 1
  [[ "$(od -An -N4 -tu4 "$f" 2>/dev/null)" -eq 1330464595 ]] || return 1
  [[ "$(od -An -j4 -N1 -tu1 "$f" 2>/dev/null)" -eq 1 ]] || return 1
  local timestamp=$(od -An -j24 -N8 -tu8 "$f" 2>/dev/null)
  local now=$(date +%s)
  (( now < timestamp || now - timestamp <= 300 ))
}

omg-explicit-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { command omg explicit --count; return; }
  od -An -j12 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-total-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
  od -An -j8 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-orphan-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
  od -An -j16 -N4 -tu4 "$f" 2>/dev/null | tr -d ' '
}
omg-updates-count() {
  local f=__OMG_STATUS_FILE__
  _omg_status_file_valid || { echo 0; return; }
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
  if not set -q _OMG_PATH_BASE
    set -g _OMG_PATH_BASE $PATH
  end
  set -gx PATH $_OMG_PATH_BASE
  # `command` bypasses fish functions and aliases, so a user-defined `omg`
  # function can neither shadow nor recursively invoke the real binary
  # (mirrors the `\command omg` guard in the zsh and bash hooks).
  command omg hook-env -s fish | source
end
";

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    /// Shell integration removal deletes exactly the lines OMG owns,
    /// backs the rc file up first, and is a no-op the second time.
    /// Serial: HOME is process-global environment.
    #[serial_test::serial]
    #[test]
    fn hook_uninstall_removes_only_owned_lines() {
        let home = tempdir().unwrap();
        let home_str = home.path().to_string_lossy().into_owned();
        let vars: Vec<(&str, Option<&str>)> = vec![("HOME", Some(home_str.as_str()))];
        temp_env::with_vars(&vars, || {
            let rc = home.path().join(".bashrc");
            fs::write(
                &rc,
                "alias ll=\"ls -l\"\n# OMG Package Manager\neval \"$(omg hook bash)\"\n",
            )
            .unwrap();
            assert!(remove_hook("bash").unwrap());
            let kept = fs::read_to_string(&rc).unwrap();
            assert!(kept.contains("alias ll"), "{kept}");
            assert!(!kept.contains("omg hook"), "{kept}");
            assert!(!kept.contains("OMG Package Manager"), "{kept}");
            assert!(home.path().join(".bashrc.omg-backup").exists());
            assert!(!remove_hook("bash").unwrap());
        });
    }

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
    fn python_version_file_wins_over_same_directory_pyproject() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".python-version"), "3.11.9").unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\nrequires-python = \">=3.12\"\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path()).unwrap();

        assert_eq!(
            versions.get("python"),
            Some(&"3.11.9".to_string()),
            "same-directory .python-version must win over pyproject.toml"
        );
    }

    #[test]
    fn pyproject_requires_python_is_detected_with_raw_specifier() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\nrequires-python = \">=3.12\"\n\n[tool.poetry]\nname = \"y\"\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path()).unwrap();

        assert_eq!(
            versions.get("python"),
            Some(&">=3.12".to_string()),
            "requires-python specifier must be preserved raw"
        );
    }

    #[test]
    fn pyproject_requires_python_resolves_to_newest_installed_match() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nrequires-python = \">=3.12\"\n",
        )
        .unwrap();
        let versions = detect_versions(dir.path()).unwrap();
        assert_eq!(versions.get("python"), Some(&">=3.12".to_string()));

        let data = tempdir().unwrap();
        for version in ["3.12.4", "3.13.1"] {
            fs::create_dir_all(
                data.path()
                    .join("versions/python")
                    .join(version)
                    .join("bin"),
            )
            .unwrap();
        }
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let additions = build_path_additions(&versions).unwrap();
            assert_eq!(
                additions.len(),
                1,
                "raw specifier must resolve through the generic resolver: {additions:?}"
            );
            assert!(
                additions[0].ends_with("versions/python/3.13.1/bin"),
                "newest installed match must win: {additions:?}"
            );
        });
    }

    #[test]
    fn pyproject_without_project_section_pins_nothing() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.poetry]\nname = \"x\"\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path()).unwrap();

        assert!(
            !versions.contains_key("python"),
            "poetry-only pyproject must not pin python, got {versions:?}"
        );
    }

    #[test]
    fn malformed_pyproject_fails_closed() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "project = [").unwrap();

        let error =
            detect_versions(dir.path()).expect_err("malformed pyproject.toml must not be ignored");
        assert!(
            error.to_string().contains("Failed to parse"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn deno_pin_files_are_detected() {
        for (filename, version) in [(".deno-version", "2.1.4"), (".dvmrc", "2.0.0")] {
            let dir = tempdir().unwrap();
            fs::write(dir.path().join(filename), version).unwrap();

            let versions = detect_versions(dir.path()).unwrap();

            assert_eq!(
                versions.get("deno"),
                Some(&version.to_string()),
                "{filename} must pin deno"
            );
        }
    }

    #[test]
    fn deno_version_file_wins_over_dvmrc() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".deno-version"), "2.1.4").unwrap();
        fs::write(dir.path().join(".dvmrc"), "2.0.0").unwrap();

        let versions = detect_versions(dir.path()).unwrap();

        assert_eq!(
            versions.get("deno"),
            Some(&"2.1.4".to_string()),
            ".deno-version must win over .dvmrc"
        );
    }

    #[test]
    fn deno_two_component_request_selects_newest_installed_patch() {
        let data = tempdir().unwrap();
        for version in ["1.40.1", "1.40.9", "1.41.0"] {
            fs::create_dir_all(data.path().join("versions/deno").join(version).join("bin"))
                .unwrap();
        }
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let versions = HashMap::from([("deno".to_string(), "1.40".to_string())]);
            let additions = build_path_additions(&versions).unwrap();
            assert_eq!(
                additions,
                vec![
                    data.path()
                        .join("versions/deno/1.40.9/bin")
                        .display()
                        .to_string()
                ],
                "a two-component request is compatible (~1.40.0) and must pick the newest 1.40.x"
            );
        });
    }

    #[test]
    fn version_request_semantics_match_compat_rules() {
        let render = |request: &str| normalize_version_req(request).map(|req| req.to_string());
        assert_eq!(
            render("3.12").as_deref(),
            Some("~3.12.0"),
            "bare two-component request must become compatible"
        );
        assert_eq!(
            render("3.12.0").as_deref(),
            Some("=3.12.0"),
            "full three-component request must stay exact"
        );
        assert_eq!(
            render("3").as_deref(),
            Some("^3.0.0"),
            "major request must keep compatible-major semantics"
        );
    }

    #[test]
    fn java_two_component_request_maps_to_feature_directory() {
        let data = tempdir().unwrap();
        fs::create_dir_all(data.path().join("versions/java/21/bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let versions = HashMap::from([("java".to_string(), "21.0".to_string())]);
            let additions = build_path_additions(&versions).unwrap();
            assert_eq!(
                additions,
                vec![
                    data.path()
                        .join("versions/java/21/bin")
                        .display()
                        .to_string()
                ],
                "java 21.0 must map to the feature-number directory 21"
            );
        });
    }

    #[test]
    fn non_feature_java_request_fails_soft_in_hook_path_building() {
        let data = tempdir().unwrap();
        fs::create_dir_all(data.path().join("versions/java/21/bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            for request in ["21.0.5", "latest", "21-ea"] {
                let versions = HashMap::from([("java".to_string(), request.to_string())]);
                let additions = build_path_additions(&versions)
                    .expect("non-feature java request must not fail the hook");
                assert!(
                    additions.is_empty(),
                    "non-feature request {request:?} must skip java, got {additions:?}"
                );
            }
        });
    }

    #[test]
    fn vendor_companion_commands_share_the_selected_path() {
        let data = tempdir().unwrap();
        for (runtime, version, relative_bin, primary, companion) in [
            ("node", "20.10.0", "bin", "node", "npm"),
            ("python", "3.12.0", "bin", "python3", "pip"),
            ("bun", "1.2.0", "", "bun", "bunx"),
            ("deno", "2.9.6", "bin", "deno", "deno-lsp"),
        ] {
            let selected = if relative_bin.is_empty() {
                data.path().join("versions").join(runtime).join(version)
            } else {
                data.path()
                    .join("versions")
                    .join(runtime)
                    .join(version)
                    .join(relative_bin)
            };
            fs::create_dir_all(&selected).unwrap();
            fs::write(selected.join(primary), b"#!/bin/sh").unwrap();
            fs::write(selected.join(companion), b"#!/bin/sh").unwrap();

            temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
                let versions = HashMap::from([(runtime.to_string(), version.to_string())]);
                let additions = build_path_additions(&versions).unwrap();
                assert_eq!(additions, vec![selected.display().to_string()]);
                assert!(selected.join(primary).is_file());
                assert!(selected.join(companion).is_file());
            });
        }
    }

    #[test]
    fn full_version_pin_does_not_select_a_newer_patch() {
        let data = tempdir().unwrap();
        let exact = data.path().join("versions/python/3.12.0/bin");
        fs::create_dir_all(&exact).unwrap();
        fs::create_dir_all(data.path().join("versions/python/3.12.9/bin")).unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(data.path()), || {
            let versions = HashMap::from([("python".to_string(), "3.12.0".to_string())]);
            assert_eq!(
                build_path_additions(&versions).unwrap(),
                vec![exact.display().to_string()]
            );
        });
    }

    #[test]
    fn bash_hook_preserves_array_prompt_commands() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut child = Command::new("bash")
            .args(["--noprofile", "--norc"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("launch bash");
        let script = format!(
            "PROMPT_COMMAND=(\"first\" \"second\")\n{BASH_HOOK}\ndeclare -p PROMPT_COMMAND\n"
        );
        child
            .stdin
            .as_mut()
            .expect("bash stdin")
            .write_all(script.as_bytes())
            .expect("write bash fixture");
        let output = child.wait_with_output().expect("wait for bash");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 bash output");
        assert!(stdout.contains("declare -a PROMPT_COMMAND="), "{stdout}");
        assert!(stdout.contains("[0]=\"_omg_hook\""), "{stdout}");
        assert!(stdout.contains("[1]=\"first\""), "{stdout}");
        assert!(stdout.contains("[2]=\"second\""), "{stdout}");
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
    fn rust_toolchain_toml_uses_structured_channel_parsing() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("rust-toolchain.toml"),
            "# channel = \"nightly\"\n[toolchain]\nchannel = \"stable\" # rolling\ncomponents = [\"rustfmt\"]\n",
        )
        .unwrap();

        let versions = detect_versions(dir.path()).unwrap();

        assert_eq!(versions.get("rust").map(String::as_str), Some("stable"));
    }

    #[test]
    fn malformed_rust_toolchain_toml_fails_closed() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = [\"nightly\"]\n",
        )
        .unwrap();

        let error = detect_versions(dir.path())
            .expect_err("malformed project-local toolchain must not be ignored");
        assert!(error.to_string().contains("Failed to parse"));
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
    fn malformed_ancestor_pin_does_not_break_child_detection() {
        let root = tempdir().unwrap();
        let child = root.path().join("project");
        fs::create_dir(&child).unwrap();
        fs::write(root.path().join("package.json"), "not json").unwrap();
        fs::write(child.join(".nvmrc"), "20.10.0").unwrap();

        let versions = detect_versions(&child).expect("ancestor parse failure must be isolated");

        assert_eq!(versions.get("node"), Some(&"20.10.0".to_string()));
    }

    #[test]
    fn fish_hook_uses_command_guard() {
        // W2-B-01: a user function named `omg` must not shadow or
        // recursively invoke the hook's call into the real binary.
        assert!(
            FISH_HOOK.contains("command omg hook-env -s fish"),
            "fish hook must invoke the binary via `command omg`, got:\n{FISH_HOOK}"
        );
    }

    #[test]
    fn zsh_and_bash_hooks_keep_command_guard() {
        assert!(ZSH_HOOK.contains("\\command omg hook-env -s zsh"));
        assert!(BASH_HOOK.contains("\\command omg hook-env -s bash"));
    }

    #[test]
    fn zsh_hook_registers_completions_on_load() {
        assert!(
            ZSH_HOOK.contains("fpath=(\"$HOME/.oh-my-zsh/completions\" \"$HOME/.zfunc\" $fpath)")
        );
        assert!(ZSH_HOOK.contains("autoload -Uz _omg"));
        assert!(ZSH_HOOK.contains("compdef _omg omg"));
        let registration = ZSH_HOOK
            .split("# ~/.zfunc is not on a default omz fpath")
            .nth(1)
            .expect("completion registration block");
        assert!(
            !registration.contains("local "),
            "top-level hook registration must not use local"
        );
    }

    #[test]
    fn malformed_cwd_pin_degrades_to_empty_versions() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "not json").unwrap();
        // detect_versions still fails closed for the start directory...
        assert!(detect_versions(dir.path()).is_err());
        // ...but the hook path degrades gracefully.
        let versions = detect_versions_for_hook(dir.path());
        assert!(
            versions.is_empty(),
            "malformed cwd pin must yield no additions, got {versions:?}"
        );
    }

    #[test]
    #[serial_test::serial]
    fn malformed_cwd_pin_hook_env_succeeds_with_no_path_output() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "not json").unwrap();
        let original = std::env::current_dir().unwrap();
        let _restore = scopeguard::guard(original, |dir| {
            std::env::set_current_dir(dir).unwrap();
        });
        std::env::set_current_dir(dir.path()).unwrap();

        // Exit 0: a malformed cwd pin must not fail every shell prompt.
        hook_env("fish").expect("malformed cwd pin must not fail hook-env");
        // No PATH additions are emitted, so the generated hook leaves PATH
        // at the user's base PATH — the environment keeps working.
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
