//! PKGBUILD metadata parser
//!
//! Extracts package information from PKGBUILD files without a Bash interpreter.
//! Uses optimized string scanning and regex for high performance.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

static RE_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^([a-z0-9_]+)=(.+)$").unwrap());

#[derive(Debug, Clone, Default)]
pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: String,
    pub description: String,
    pub url: String,
    pub license: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub sources: Vec<String>,
    pub sha256sums: Vec<String>,
    pub validpgpkeys: Vec<String>,
}

impl PkgBuild {
    /// Parse a PKGBUILD file
    pub fn parse(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read PKGBUILD at {}", path.display()))?;

        Self::parse_content(&content)
    }

    /// Parse PKGBUILD content
    pub fn parse_content(content: &str) -> Result<Self> {
        let mut pkg = Self::default();
        let mut vars = HashMap::new();

        for cap in RE_VAR.captures_iter(content) {
            let key = cap[1].to_string();
            let val = cap[2]
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();

            // Handle basic arrays
            if val.starts_with('(') && val.ends_with(')') {
                // Keep as is for now, will process later
            }

            vars.insert(key, val);
        }

        // Second pass: Perform variable substitution
        let substitute = |val: &str, vars: &HashMap<String, String>| -> String {
            let mut result = val.to_string();
            // Sort keys by length descending to avoid partial matches (e.g., $pkgname vs $pkgname_ext)
            let mut keys: Vec<_> = vars.keys().collect();
            keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

            for k in keys {
                let v = vars.get(k).unwrap();
                let pattern1 = format!("${k}");
                let pattern2 = format!("${{{k}}}");
                result = result.replace(&pattern1, v);
                result = result.replace(&pattern2, v);
            }
            result
        };

        if let Some(v) = vars.get("pkgname") {
            pkg.name = substitute(v, &vars);
        }
        if let Some(v) = vars.get("pkgver") {
            pkg.version = substitute(v, &vars);
        }
        if let Some(v) = vars.get("pkgrel") {
            pkg.release = substitute(v, &vars);
        }
        if let Some(v) = vars.get("pkgdesc") {
            pkg.description = substitute(v, &vars);
        }
        if let Some(v) = vars.get("url") {
            pkg.url = substitute(v, &vars);
        }

        // Process arrays with substitution
        if let Some(v) = vars.get("depends") {
            pkg.depends = parse_array(&substitute(v, &vars));
        }
        if let Some(v) = vars.get("makedepends") {
            pkg.makedepends = parse_array(&substitute(v, &vars));
        }
        if let Some(v) = vars.get("source") {
            pkg.sources = parse_array(&substitute(v, &vars));
        }
        if let Some(v) = vars.get("sha256sums") {
            pkg.sha256sums = parse_array(&substitute(v, &vars));
        }
        if let Some(v) = vars.get("license") {
            pkg.license = parse_array(&substitute(v, &vars));
        }
        if let Some(v) = vars.get("validpgpkeys") {
            pkg.validpgpkeys = parse_array(&substitute(v, &vars));
        }

        Ok(pkg)
    }
}

fn parse_array(val: &str) -> Vec<String> {
    let cleaned = val
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = cleaned.trim_matches('(').trim_matches(')');
    trimmed
        .split_whitespace()
        .filter_map(|s| {
            let token = s.trim_matches('"').trim_matches('\'');
            if token.is_empty() {
                None
            } else {
                Some(token.to_string())
            }
        })
        .collect()
}
