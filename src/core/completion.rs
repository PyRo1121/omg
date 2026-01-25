//! Intelligent completions with fuzzy matching and context awareness.

use std::path::PathBuf;

use anyhow::Result;
use jiff::Timestamp;
use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{CaseMatching, Normalization, Pattern},
};

use crate::core::Database;

/// Intelligent completion engine
pub struct CompletionEngine {
    db: Database,
}

impl CompletionEngine {
    #[must_use]
    pub const fn new(db: Database) -> Self {
        Self { db }
    }

    /// Perform fuzzy matching on a list of candidates (10x faster with nucleo)
    #[must_use]
    pub fn fuzzy_match(&self, pattern: &str, candidates: Vec<String>) -> Vec<String> {
        if pattern.is_empty() {
            return candidates;
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pat = Pattern::parse(pattern, CaseMatching::Ignore, Normalization::Smart);

        let mut matches: Vec<(String, u32)> = candidates
            .into_iter()
            .filter_map(|cand| {
                let haystack = Utf32String::from(cand.as_str());
                let score = pat.score(haystack.slice(..), &mut matcher)?;
                Some((cand, score))
            })
            .collect();

        matches.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.len().cmp(&b.0.len()))
                .then_with(|| a.0.cmp(&b.0))
        });

        matches.into_iter().map(|(s, _)| s).collect()
    }

    /// Probe context (package.json, .nvmrc, etc.) to prioritize versions
    #[must_use]
    pub fn probe_context(&self, runtime: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Search up the tree for config files
        let mut dir = Some(current_dir.as_path());
        while let Some(path) = dir {
            match runtime {
                "node" => {
                    // Check package.json
                    let pkg_json = path.join("package.json");
                    if pkg_json.exists()
                        && let Ok(content) = std::fs::read_to_string(pkg_json)
                        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&content)
                        && let Some(engines) = v.get("engines")
                        && let Some(node_v) = engines.get("node")
                        && let Some(s) = node_v.as_str()
                    {
                        suggestions.push(s.to_string());
                    }
                    // Check .nvmrc
                    let nvmrc = path.join(".nvmrc");
                    if nvmrc.exists()
                        && let Ok(content) = std::fs::read_to_string(nvmrc)
                    {
                        suggestions.push(content.trim().to_string());
                    }
                }
                "python" => {
                    let py_version = path.join(".python-version");
                    if py_version.exists()
                        && let Ok(content) = std::fs::read_to_string(py_version)
                    {
                        suggestions.push(content.trim().to_string());
                    }
                }
                "rust" => {
                    let toolchain = path.join("rust-toolchain");
                    let toolchain_toml = path.join("rust-toolchain.toml");
                    if toolchain.exists() {
                        if let Ok(content) = std::fs::read_to_string(toolchain) {
                            suggestions.push(content.trim().to_string());
                        }
                    } else if toolchain_toml.exists()
                        && let Ok(content) = std::fs::read_to_string(toolchain_toml)
                        && content.contains("channel = \"")
                        && let Some(v) = content
                            .split("channel = \"")
                            .nth(1)
                            .and_then(|s| s.split('"').next())
                    {
                        suggestions.push(v.to_string());
                    }
                }
                _ => {}
            }
            if !suggestions.is_empty() {
                break;
            }
            dir = path.parent();
        }

        suggestions
    }

    /// Get AUR package names from cache or refresh if needed
    pub async fn get_aur_package_names(&self) -> Result<Vec<String>> {
        // Check last refresh using redb-based Database
        if let Some(last_refresh) = self.db.get_completion("aur_last_refresh")?
            && let Ok(last) = last_refresh.parse::<Timestamp>()
        {
            let now = Timestamp::now();
            let hours_since = now.as_second() - last.as_second();

            if hours_since < 24 * 3600
                && let Some(data) = self.db.get_completion("aur_packages")?
            {
                return Ok(data
                    .split(',')
                    .map(std::string::ToString::to_string)
                    .collect());
            }
        }

        // Refresh cache
        let names = self.fetch_aur_names().await?;
        let data = names.join(",");

        self.db.set_completion("aur_packages", &data)?;
        self.db
            .set_completion("aur_last_refresh", &Timestamp::now().to_string())?;

        Ok(names)
    }

    async fn fetch_aur_names(&self) -> Result<Vec<String>> {
        // Use the AUR RPC to get all package names
        let url = "https://aur.archlinux.org/packages.gz";
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;

        use std::io::Read;
        let mut gz = flate2::read::GzDecoder::new(&bytes[..]);
        let mut s = String::new();
        gz.read_to_string(&mut s)?;

        Ok(s.lines().map(std::string::ToString::to_string).collect())
    }
}

/// Get completions for common commands
#[must_use]
pub fn get_command_completions(partial: &str) -> Vec<String> {
    let commands = vec![
        "search",
        "install",
        "remove",
        "update",
        "info",
        "why",
        "outdated",
        "pin",
        "size",
        "blame",
        "diff",
        "snapshot",
        "ci",
        "migrate",
        "clean",
        "explicit",
        "sync",
        "use",
        "list",
        "hook",
        "run",
        "new",
        "tool",
        "env",
        "team",
        "container",
        "license",
        "fleet",
        "enterprise",
        "history",
        "rollback",
        "dash",
        "stats",
        "init",
        "doctor",
        "audit",
    ];

    if partial.is_empty() {
        return commands.into_iter().map(String::from).collect();
    }

    let partial_lower = partial.to_lowercase();
    commands
        .into_iter()
        .filter(|c| c.starts_with(&partial_lower))
        .map(String::from)
        .collect()
}

/// Get completions for runtime names
#[must_use]
pub fn get_runtime_completions(partial: &str) -> Vec<String> {
    let runtimes = vec!["node", "python", "rust", "go", "ruby", "java", "bun"];

    if partial.is_empty() {
        return runtimes.into_iter().map(String::from).collect();
    }

    let partial_lower = partial.to_lowercase();
    runtimes
        .into_iter()
        .filter(|r| r.starts_with(&partial_lower))
        .map(String::from)
        .collect()
}

/// Get completions for tool names from registry
#[must_use]
pub fn get_tool_completions(partial: &str) -> Vec<String> {
    let tools = crate::cli::tool::registry_tool_names();

    if partial.is_empty() {
        return tools;
    }

    let partial_lower = partial.to_lowercase();
    tools
        .into_iter()
        .filter(|t| t.to_lowercase().starts_with(&partial_lower))
        .collect()
}

/// Get completions for container subcommands
#[must_use]
pub fn get_container_completions(partial: &str) -> Vec<String> {
    let subcommands = vec![
        "status", "run", "shell", "build", "list", "images", "pull", "stop", "exec", "init",
    ];

    if partial.is_empty() {
        return subcommands.into_iter().map(String::from).collect();
    }

    let partial_lower = partial.to_lowercase();
    subcommands
        .into_iter()
        .filter(|c| c.starts_with(&partial_lower))
        .map(String::from)
        .collect()
}

/// Get completions for env subcommands
#[must_use]
pub fn get_env_completions(partial: &str) -> Vec<String> {
    let subcommands = vec!["capture", "check", "share", "sync"];

    if partial.is_empty() {
        return subcommands.into_iter().map(String::from).collect();
    }

    let partial_lower = partial.to_lowercase();
    subcommands
        .into_iter()
        .filter(|c| c.starts_with(&partial_lower))
        .map(String::from)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fuzzy_match_returns_matches() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("test.redb")).unwrap();
        let engine = CompletionEngine::new(db);

        let candidates = vec![
            "firefox".to_string(),
            "chromium".to_string(),
            "brave".to_string(),
        ];

        let results = engine.fuzzy_match("fire", candidates);
        assert_eq!(results.first().map(String::as_str), Some("firefox"));
    }

    #[test]
    fn fuzzy_match_empty_pattern_returns_all() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("test.redb")).unwrap();
        let engine = CompletionEngine::new(db);

        let candidates = vec!["a".to_string(), "b".to_string()];
        let results = engine.fuzzy_match("", candidates.clone());
        assert_eq!(results, candidates);
    }

    #[test]
    fn command_completions_work() {
        let results = get_command_completions("ins");
        assert!(results.contains(&"install".to_string()));
    }

    #[test]
    fn runtime_completions_work() {
        let results = get_runtime_completions("no");
        assert!(results.contains(&"node".to_string()));
    }
}
