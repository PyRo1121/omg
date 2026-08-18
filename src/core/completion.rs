//! Intelligent completions with fuzzy matching and context awareness.

use std::path::Path;

use anyhow::{Context, Result};
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
    pub fn probe_context(&self, runtime: &str) -> Result<Vec<String>> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        self.probe_context_from(&current_dir, runtime)
    }

    fn probe_context_from(&self, start: &Path, runtime: &str) -> Result<Vec<String>> {
        let mut suggestions = Vec::new();
        let mut dir = Some(start);

        while let Some(path) = dir {
            match runtime {
                "node" => {
                    let pkg_json = path.join("package.json");
                    if let Some(content) = read_optional_file(&pkg_json)? {
                        let value: serde_json::Value = serde_json::from_str(&content)
                            .with_context(|| format!("Failed to parse {}", pkg_json.display()))?;
                        if let Some(s) = value
                            .get("engines")
                            .and_then(|engines| engines.get("node"))
                            .and_then(serde_json::Value::as_str)
                        {
                            suggestions.push(s.to_string());
                        }
                    }
                    let nvmrc = path.join(".nvmrc");
                    if let Some(content) = read_optional_file(&nvmrc)? {
                        suggestions.push(content.trim().to_string());
                    }
                }
                "python" => {
                    let py_version = path.join(".python-version");
                    if let Some(content) = read_optional_file(&py_version)? {
                        suggestions.push(content.trim().to_string());
                    }
                }
                "rust" => {
                    let toolchain = path.join("rust-toolchain");
                    if let Some(content) = read_optional_file(&toolchain)? {
                        suggestions.push(content.trim().to_string());
                    } else {
                        let toolchain_toml = path.join("rust-toolchain.toml");
                        if let Some(content) = read_optional_file(&toolchain_toml)?
                            && content.contains("channel = \"")
                            && let Some(v) = content
                                .split("channel = \"")
                                .nth(1)
                                .and_then(|s| s.split('"').next())
                        {
                            suggestions.push(v.to_string());
                        }
                    }
                }
                _ => {}
            }
            if !suggestions.is_empty() {
                break;
            }
            dir = path.parent();
        }

        Ok(suggestions)
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

fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}", path.display())),
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
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
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

    #[test]
    fn probe_context_reads_python_version() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join(".python-version"), "3.12.0\n").unwrap();
        let db = Database::open(temp_dir.path().join("test.redb")).unwrap();
        let engine = CompletionEngine::new(db);
        let suggestions = engine
            .probe_context_from(temp_dir.path(), "python")
            .unwrap();
        assert_eq!(suggestions.first().map(String::as_str), Some("3.12.0"));
    }

    #[test]
    fn probe_context_unreadable_pin_fails_closed() {
        let temp_dir = TempDir::new().unwrap();
        let pin = temp_dir.path().join(".python-version");
        std::fs::write(&pin, "3.12.0\n").unwrap();
        let original = std::fs::metadata(&pin).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&pin, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = std::fs::read_to_string(&pin).is_err();
        let db = Database::open(temp_dir.path().join("test.redb")).unwrap();
        let engine = CompletionEngine::new(db);
        let result = engine.probe_context_from(temp_dir.path(), "python");
        let _ = std::fs::set_permissions(&pin, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable pin must fail closed, got {result:?}"
        );
    }

    #[test]
    fn probe_context_invalid_package_json_fails_closed() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("package.json"), "not json").unwrap();
        let db = Database::open(temp_dir.path().join("test.redb")).unwrap();
        let engine = CompletionEngine::new(db);
        let error = engine
            .probe_context_from(temp_dir.path(), "node")
            .unwrap_err();
        assert!(
            error.to_string().contains("Failed to parse"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn read_optional_file_missing_is_none() {
        let missing = TempDir::new().unwrap().path().join("does-not-exist");
        assert!(read_optional_file(&missing).unwrap().is_none());
    }
}
