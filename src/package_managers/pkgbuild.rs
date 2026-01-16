//! PKGBUILD metadata parser
//!
//! Extracts package information from PKGBUILD files without a Bash interpreter.
//! Handles multi-line arrays properly for accurate dependency extraction.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
    pub checkdepends: Vec<String>,
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

    /// Parse PKGBUILD content - handles multi-line arrays
    pub fn parse_content(content: &str) -> Result<Self> {
        let mut pkg = Self::default();
        let mut vars: HashMap<String, String> = HashMap::new();

        // First pass: Extract all variables including multi-line arrays
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                i += 1;
                continue;
            }

            // Look for variable assignment
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();

                // Validate key is a valid bash variable name
                if !key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                {
                    i += 1;
                    continue;
                }

                // Check if this is a multi-line array
                if val.starts_with('(') && !val.ends_with(')') {
                    // Multi-line array - collect until closing paren
                    let mut array_content = val.to_string();
                    i += 1;
                    while i < lines.len() {
                        let next_line = lines[i];
                        array_content.push(' ');
                        array_content.push_str(next_line);
                        if next_line.contains(')') {
                            break;
                        }
                        i += 1;
                    }
                    vars.insert(key.to_string(), array_content);
                } else {
                    // Single-line value
                    let val = val.trim_matches('"').trim_matches('\'').to_string();
                    vars.insert(key.to_string(), val);
                }
            }
            i += 1;
        }

        // Second pass: Perform variable substitution
        let substitute = |val: &str, vars: &HashMap<String, String>| -> String {
            let mut result = val.to_string();
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
        if let Some(v) = vars.get("checkdepends") {
            pkg.checkdepends = parse_array(&substitute(v, &vars));
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
    // Remove comments and join lines
    let cleaned = val
        .lines()
        .map(|line| {
            // Remove inline comments
            line.split('#').next().unwrap_or("")
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Remove parentheses
    let trimmed = cleaned.trim();
    let trimmed = trimmed.strip_prefix('(').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix(')').unwrap_or(trimmed);

    // Parse items - handle both quoted and unquoted
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
