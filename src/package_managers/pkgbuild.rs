//! PKGBUILD metadata parser
//!
//! Extracts package information from PKGBUILD files without a Bash interpreter.
//! Handles multi-line arrays properly for accurate dependency extraction.

use alpm_types::Version;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Read a file safely, preventing symlink attacks on Unix.
///
/// # Security
/// Uses `O_NOFOLLOW` to reject symlinks, preventing attacks where a malicious
/// symlink could redirect file reads to arbitrary locations.
#[cfg(unix)]
fn safe_read_file(path: &Path) -> std::io::Result<String> {
    use std::fs::OpenOptions;

    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;

    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

/// Read a file (non-Unix fallback).
#[cfg(not(unix))]
fn safe_read_file(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

#[derive(Debug, Clone)]
pub struct PkgBuild {
    pub name: String,
    pub version: Version,
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

impl Default for PkgBuild {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: super::types::zero_version(),
            release: String::new(),
            description: String::new(),
            url: String::new(),
            license: Vec::new(),
            depends: Vec::new(),
            makedepends: Vec::new(),
            checkdepends: Vec::new(),
            sources: Vec::new(),
            sha256sums: Vec::new(),
            validpgpkeys: Vec::new(),
        }
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }

        match quote {
            Some(delimiter) if character == delimiter => quote = None,
            None if character == '\'' || character == '"' => quote = Some(character),
            None if character == '#' => return &line[..index],
            Some(_) | None => {}
        }
    }

    line
}

impl PkgBuild {
    /// Parse a PKGBUILD file
    ///
    /// # Security
    /// Uses `O_NOFOLLOW` on Unix to prevent symlink attacks where a malicious
    /// PKGBUILD symlink could point to sensitive files like /etc/passwd.
    pub fn parse(path: &Path) -> Result<Self> {
        let content = safe_read_file(path)
            .with_context(|| format!("Failed to read PKGBUILD at {}", path.display()))?;

        Self::parse_content(&content)
    }

    /// Parse PKGBUILD content - handles multi-line arrays
    pub fn parse_content(content: &str) -> Result<Self> {
        let mut vars: HashMap<String, String> = HashMap::new();

        // First pass: Extract all variables including multi-line arrays.
        let mut lines = content.lines();
        while let Some(line) = lines.next() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = strip_inline_comment(val).trim();
            if !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                continue;
            }

            if val.starts_with('(') && !val.ends_with(')') {
                let mut array_content = val.to_string();
                for next_line in lines.by_ref() {
                    let next_line = strip_inline_comment(next_line);
                    array_content.push(' ');
                    array_content.push_str(next_line);
                    if next_line.contains(')') {
                        break;
                    }
                }
                vars.insert(key.to_string(), array_content);
            } else {
                vars.insert(
                    key.to_string(),
                    val.trim_matches('"').trim_matches('\'').to_string(),
                );
            }
        }

        // Sort substitution sources once, longest first, so shorter variable
        // names cannot partially replace longer names.
        let mut substitutions: Vec<_> = vars.iter().collect();
        substitutions.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
        let substitute = |val: &str| -> String {
            let mut result = val.to_string();
            for (key, value) in &substitutions {
                result = result.replace(&format!("${key}"), value);
                result = result.replace(&format!("${{{key}}}"), value);
            }
            result
        };
        let scalar = |key: &str| vars.get(key).map_or_else(String::new, |v| substitute(v));
        let array = |key: &str| {
            vars.get(key)
                .map_or_else(Vec::new, |v| parse_array(&substitute(v)))
        };

        Ok(Self {
            name: scalar("pkgname"),
            version: vars
                .get("pkgver")
                .map_or_else(super::types::zero_version, |v| {
                    super::types::parse_version_or_zero(&substitute(v))
                }),
            release: scalar("pkgrel"),
            description: scalar("pkgdesc"),
            url: scalar("url"),
            license: array("license"),
            depends: array("depends"),
            makedepends: array("makedepends"),
            checkdepends: array("checkdepends"),
            sources: array("source"),
            sha256sums: array("sha256sums"),
            validpgpkeys: array("validpgpkeys"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_comments_do_not_absorb_following_assignments() {
        let package = PkgBuild::parse_content(
            r#"
                pkgname = "demo" # package name
                pkgver = "1.2.3" # release version
                pkgrel = "1" # package release
                depends = ("openssl" "zlib") # dependency list
                source = ("https://example.test/archive#fragment") # source URL
            "#,
        )
        .expect("valid PKGBUILD metadata");

        assert_eq!(package.name, "demo");
        assert_eq!(package.version.to_string(), "1.2.3");
        assert_eq!(package.release, "1");
        assert_eq!(package.depends, ["openssl", "zlib"]);
        assert_eq!(package.sources, ["https://example.test/archive#fragment"]);
    }
}

fn parse_array(val: &str) -> Vec<String> {
    // Remove comments and join lines
    let cleaned = val
        .lines()
        .map(strip_inline_comment)
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
