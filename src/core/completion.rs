//! Intelligent completions with fuzzy matching and context awareness.

use anyhow::Result;
use chrono::{DateTime, Utc};
use nucleo_matcher::{Matcher, Utf32Str};
use std::path::PathBuf;

use crate::core::Database;

/// Intelligent completion engine
pub struct CompletionEngine {
    db: Database,
}

impl CompletionEngine {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Perform fuzzy matching on a list of candidates
    pub fn fuzzy_match(&self, pattern: &str, candidates: Vec<String>) -> Vec<String> {
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut pattern_buf = Vec::new();
        let pattern_utf32 = Utf32Str::new(pattern, &mut pattern_buf);

        let mut matches: Vec<(String, u16)> = candidates
            .into_iter()
            .filter_map(|cand| {
                let mut cand_buf = Vec::new();
                let cand_utf32 = Utf32Str::new(&cand, &mut cand_buf);
                matcher
                    .fuzzy_match(cand_utf32, pattern_utf32)
                    .map(|score| (cand, score))
            })
            .collect();

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        matches.into_iter().map(|(s, _)| s).collect()
    }

    /// Probe context (package.json, .nvmrc, etc.) to prioritize versions
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
                if let Ok(last) = DateTime::parse_from_rfc3339(last_refresh) {
                    if Utc::now().signed_duration_since(last).num_hours() < 24 {
                        if let Some(data) = db.get(&rtxn, "aur_packages")? {
                            return Ok(data.split(',').map(|s| s.to_string()).collect());
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
        db.put(&mut wtxn, "aur_last_refresh", &Utc::now().to_rfc3339())?;
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

        Ok(s.lines().map(|l| l.to_string()).collect())
    }
}
