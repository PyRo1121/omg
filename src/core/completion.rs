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
                    if pkg_json.exists() {
                        if let Ok(content) = std::fs::read_to_string(pkg_json) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(engines) = v.get("engines") {
                                    if let Some(node_v) = engines.get("node") {
                                        if let Some(s) = node_v.as_str() {
                                            suggestions.push(s.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Check .nvmrc
                    let nvmrc = path.join(".nvmrc");
                    if nvmrc.exists() {
                        if let Ok(content) = std::fs::read_to_string(nvmrc) {
                            suggestions.push(content.trim().to_string());
                        }
                    }
                }
                "python" => {
                    let py_version = path.join(".python-version");
                    if py_version.exists() {
                        if let Ok(content) = std::fs::read_to_string(py_version) {
                            suggestions.push(content.trim().to_string());
                        }
                    }
                }
                "rust" => {
                    let toolchain = path.join("rust-toolchain");
                    let toolchain_toml = path.join("rust-toolchain.toml");
                    if toolchain.exists() {
                        if let Ok(content) = std::fs::read_to_string(toolchain) {
                            suggestions.push(content.trim().to_string());
                        }
                    } else if toolchain_toml.exists() {
                        if let Ok(content) = std::fs::read_to_string(toolchain_toml) {
                            // Basic parsing, could be improved
                            if content.contains("channel = \"") {
                                if let Some(v) = content
                                    .split("channel = \"")
                                    .nth(1)
                                    .and_then(|s| s.split('"').next())
                                {
                                    suggestions.push(v.to_string());
                                }
                            }
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

        suggestions
    }

    /// Get AUR package names from cache or refresh if needed
    pub async fn get_aur_package_names(&self) -> Result<Vec<String>> {
        let db = self.db.get_completion_db()?;
        let env = self.db.env();

        // Check last refresh
        {
            let rtxn = env.read_txn()?;
            if let Some(last_refresh) = db.get(&rtxn, "aur_last_refresh")? {
                if let Ok(last) = last_refresh.parse::<Timestamp>() {
                    let now = Timestamp::now();
                    let hours_since = now.as_second() - last.as_second();
                    if hours_since < 24 * 3600 {
                        if let Some(data) = db.get(&rtxn, "aur_packages")? {
                            return Ok(data
                                .split(',')
                                .map(std::string::ToString::to_string)
                                .collect());
                        }
                    }
                }
            }
        }

        // Refresh cache
        let names = self.fetch_aur_names().await?;
        let data = names.join(",");

        let mut wtxn = env.write_txn()?;
        db.put(&mut wtxn, "aur_packages", &data)?;
        db.put(&mut wtxn, "aur_last_refresh", &Timestamp::now().to_string())?;
        wtxn.commit()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fuzzy_match_returns_matches() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();
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
        let db = Database::open(temp_dir.path()).unwrap();
        let engine = CompletionEngine::new(db);

        let candidates = vec!["a".to_string(), "b".to_string()];
        let results = engine.fuzzy_match("", candidates.clone());
        assert_eq!(results, candidates);
    }
}
