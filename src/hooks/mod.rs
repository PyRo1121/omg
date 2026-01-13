//! Shell hook system for PATH modification
//!
//! Implements the fast shell hook approach (like mise) for version switching.
//! This is the default and fastest method - shims are optional fallback.

pub mod completions;

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Known version files and their corresponding runtime
const VERSION_FILES: &[(&str, &str)] = &[
    // Node.js
    (".nvmrc", "node"),
    (".node-version", "node"),
    // Python
    (".python-version", "python"),
    // Ruby
    (".ruby-version", "ruby"),
    // Go
    (".go-version", "go"),
    // Java
    (".java-version", "java"),
    // Bun
    (".bun-version", "bun"),
    // Rust
    ("rust-toolchain", "rust"),
    ("rust-toolchain.toml", "rust"),
    // Universal
    (".tool-versions", "multi"),
];

/// Print the shell hook script to be added to shell rc file
///
/// Usage: eval "$(omg hook zsh)"
pub fn print_hook(shell: &str) -> Result<()> {
    let script = match shell.to_lowercase().as_str() {
        "zsh" => ZSH_HOOK,
        "bash" => BASH_HOOK,
        "fish" => FISH_HOOK,
        _ => {
            anyhow::bail!("Unsupported shell: {}. Supported: zsh, bash, fish", shell);
        }
    };

    println!("{}", script);
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
    let path_additions = build_path_additions(&versions);

    if path_additions.is_empty() {
        return Ok(());
    }

    // Output shell-specific PATH modification
    match shell.to_lowercase().as_str() {
        "zsh" | "bash" => {
            let additions = path_additions.join(":");
            println!("export PATH=\"{}:$PATH\"", additions);
        }
        "fish" => {
            for path in &path_additions {
                println!("fish_add_path -g {}", path);
            }
        }
        _ => {}
    }

    Ok(())
}

/// Detect version files in directory and parents
fn detect_versions(start: &Path) -> HashMap<String, String> {
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
                            if parts.len() >= 2 {
                                let rt = parts[0].to_lowercase();
                                let ver = parts[1].to_string();
                                if !versions.contains_key(&rt) {
                                    versions.insert(rt, ver);
                                }
                            }
                        }
                    }
                } else if *filename == "rust-toolchain.toml" {
                    // Parse TOML format
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
                } else {
                    // Simple version file
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        let version = content.trim().trim_start_matches('v').to_string();
                        if !version.is_empty() {
                            versions.insert(runtime.to_string(), version);
                        }
                    }
                }
            }
        }

        current = dir.parent().map(|p| p.to_path_buf());
    }

    versions
}

/// Build PATH additions for detected versions
fn build_path_additions(versions: &HashMap<String, String>) -> Vec<String> {
    let mut paths = Vec::new();

    let data_dir = directories::ProjectDirs::from("com", "omg", "omg")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            home::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".omg")
        });

    for (runtime, version) in versions {
        let bin_path = match runtime.as_str() {
            "node" => data_dir.join("versions/node").join(version).join("bin"),
            "python" => data_dir.join("versions/python").join(version).join("bin"),
            "go" => data_dir.join("versions/go").join(version).join("bin"),
            "ruby" => data_dir.join("versions/ruby").join(version).join("bin"),
            "java" => data_dir.join("versions/java").join(version).join("bin"),
            "bun" => data_dir.join("versions/bun").join(version),
            "rust" => {
                // Rust uses rustup's PATH
                home::home_dir().unwrap_or_default().join(".cargo/bin")
            }
            _ => continue,
        };

        if bin_path.exists() {
            paths.push(bin_path.display().to_string());
        }
    }

    paths
}

/// Get active versions for display
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
}
