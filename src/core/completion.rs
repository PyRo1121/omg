//! Intelligent completions with fuzzy matching and context awareness.

use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

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

    /// Perform fuzzy matching on a list of candidates
    #[must_use]
    pub fn fuzzy_match(&self, pattern: &str, candidates: Vec<String>) -> Vec<String> {
        if pattern.is_empty() {
            return candidates;
        }

        let pattern_lower = pattern.to_lowercase();
        let pattern_len = pattern_lower.len() as i64;
        let matcher = SkimMatcherV2::default();
        let (gap_weight, start_weight, len_weight) = if pattern_len <= 3 {
            (50, 12, 6)
        } else if pattern_len <= 6 {
            (35, 8, 3)
        } else {
            (25, 5, 2)
        };

        let mut matches: Vec<(String, i64)> = candidates
            .into_iter()
            .filter_map(|cand| {
                let candidate_lower = cand.to_lowercase();
                let (score, indices) = matcher.fuzzy_indices(&candidate_lower, &pattern_lower)?;
                let (start, end) = match (indices.first(), indices.last()) {
                    (Some(start), Some(end)) => (*start as i64, *end as i64),
                    _ => return None,
                };
                let span = end - start + 1;
                let gap = (span - pattern_len).max(0);
                let prefix_bonus = if candidate_lower.starts_with(&pattern_lower) {
                    700
                } else if candidate_lower.contains(&pattern_lower) {
                    250
                } else {
                    0
                };
                let exact_bonus = if candidate_lower == pattern_lower { 1200 } else { 0 };
                let boundary_bonus = if is_boundary_match(&candidate_lower, start as usize) {
                    120
                } else {
                    0
                };
                let candidate_len = candidate_lower.len() as i64;
                let total_score = score as i64
                    + prefix_bonus
                    + exact_bonus
                    + boundary_bonus
                    - (gap * gap_weight)
                    - (start * start_weight)
                    - (candidate_len * len_weight);
                Some((cand, total_score))
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
                if let Ok(last) = DateTime::parse_from_rfc3339(last_refresh) {
                    if Utc::now().signed_duration_since(last).num_hours() < 24 {
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

        Ok(s.lines().map(std::string::ToString::to_string).collect())
    }
}

fn is_boundary_match(candidate: &str, start: usize) -> bool {
    if start == 0 {
        return true;
    }

    if !candidate.is_char_boundary(start) {
        return false;
    }

    candidate
        .get(..start)
        .and_then(|prefix| prefix.chars().last())
        .map_or(false, |prev| !prev.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fuzzy_match_prefers_compact_subsequence() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();
        let engine = CompletionEngine::new(db);

        let candidates = vec![
            "sigrok-firmware-fx2lafw".to_string(),
            "firefox".to_string(),
        ];

        let results = engine.fuzzy_match("frfx", candidates);
        assert_eq!(results.first().map(String::as_str), Some("firefox"));
    }
}
