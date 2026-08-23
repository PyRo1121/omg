//! Intelligent completions with fuzzy matching and context awareness.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use jiff::Timestamp;
use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{CaseMatching, Normalization, Pattern},
};

use crate::core::paths;

/// File-backed completion cache (single atomically-replaced JSON document).
///
/// Replaces the former redb table: the cache holds two keys, which does not
/// justify an embedded transactional database.
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedCompletionCache {
    format_version: u32,
    entries: std::collections::HashMap<String, String>,
}

impl PersistedCompletionCache {
    const FORMAT_VERSION: u32 = 1;

    fn path() -> PathBuf {
        paths::data_dir().join("completion-cache.json")
    }

    fn load() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|error| {
                tracing::debug!("Discarding malformed completion cache: {error}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    fn save(&self) -> Result<()> {
        let content = serde_json::to_vec(self).context("Failed to serialize completion cache")?;
        crate::core::safe_ops::atomic_write_file_sync(Self::path(), content)
            .with_context(|| format!("Failed to write {}", Self::path().display()))
    }
}

impl Default for PersistedCompletionCache {
    fn default() -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            entries: std::collections::HashMap::new(),
        }
    }
}

/// Intelligent completion engine
#[derive(Default)]
pub struct CompletionEngine;

impl CompletionEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        Self::probe_context_from(&current_dir, runtime)
    }

    fn probe_context_from(start: &Path, runtime: &str) -> Result<Vec<String>> {
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
        let cache = PersistedCompletionCache::load();
        if let Some(last_refresh) = cache
            .entries
            .get("aur_last_refresh")
            .and_then(|value| value.parse::<Timestamp>().ok())
        {
            let hours_since = Timestamp::now().as_second() - last_refresh.as_second();
            if hours_since < 24 * 3600
                && let Some(data) = cache.entries.get("aur_packages")
            {
                return Ok(data.split(',').map(String::from).collect());
            }
        }

        // Refresh cache
        let names = self.fetch_aur_names().await?;
        let mut cache = PersistedCompletionCache::load();
        cache
            .entries
            .insert("aur_packages".to_string(), names.join(","));
        cache
            .entries
            .insert("aur_last_refresh".to_string(), Timestamp::now().to_string());
        cache.save()?;

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

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fuzzy_match_returns_matches() {
        let engine = CompletionEngine::new();

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
        let engine = CompletionEngine::new();

        let candidates = vec!["a".to_string(), "b".to_string()];
        let results = engine.fuzzy_match("", candidates.clone());
        assert_eq!(results, candidates);
    }

    #[test]
    fn probe_context_reads_python_version() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join(".python-version"), "3.12.0\n").unwrap();
        let suggestions = CompletionEngine::probe_context_from(temp_dir.path(), "python").unwrap();
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
        let result = CompletionEngine::probe_context_from(temp_dir.path(), "python");
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
        let error = CompletionEngine::probe_context_from(temp_dir.path(), "node").unwrap_err();
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
