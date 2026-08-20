//! Common utilities for package operations

use anyhow::Result;

pub use crate::core::env::distro::use_debian_backend;

/// Get description truncation width based on terminal size.
///
/// Reserves space for package name, version, source label, and formatting
/// chrome (~45 chars), then uses the rest for the description.
/// Falls back to 50 chars if terminal width is unavailable.
pub fn description_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| {
            let cols = cols as usize;
            // Reserve ~45 chars for "  name version (source) - " prefix chrome
            cols.saturating_sub(45).max(20)
        })
        .unwrap_or(50)
}

/// Validate a search query for safety.
///
/// Rejects queries that are too long, contain control characters,
/// path traversal sequences, or shell metacharacters.
pub fn validate_search_query(query: &str) -> Result<()> {
    if query.len() > 100 {
        anyhow::bail!("Search query too long (max 100 characters)");
    }
    if query.chars().any(char::is_control) {
        anyhow::bail!("Search query contains invalid characters");
    }
    if query.contains('/') || query.contains('\\') || query.contains("..") {
        anyhow::bail!("Invalid search query: path traversal detected");
    }
    if query.chars().any(|c| ";|&><$".contains(c)) {
        anyhow::bail!("Invalid search query: shell metacharacters detected");
    }
    Ok(())
}

/// Check if a search query passes validation (returns false on failure).
///
/// Convenience wrapper for sync code paths that return `bool` instead of `Result`.
pub fn is_valid_search_query(query: &str) -> bool {
    validate_search_query(query).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_search_query_valid() {
        assert!(validate_search_query("firefox").is_ok());
        assert!(validate_search_query("lib32-mesa").is_ok());
        assert!(validate_search_query("python-numpy").is_ok());
    }

    #[test]
    fn test_validate_search_query_too_long() {
        let long = "a".repeat(101);
        let err = validate_search_query(&long).unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn test_validate_search_query_control_chars() {
        let err = validate_search_query("test\x00query").unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_validate_search_query_path_traversal() {
        let err = validate_search_query("../etc/passwd").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn test_validate_search_query_shell_metacharacters() {
        let err = validate_search_query("test;rm -rf").unwrap_err();
        assert!(err.to_string().contains("shell metacharacters"));
    }

    #[test]
    fn test_is_valid_search_query() {
        assert!(is_valid_search_query("firefox"));
        assert!(!is_valid_search_query("../passwd"));
        assert!(!is_valid_search_query("test;ls"));
    }

    #[test]
    fn test_description_width_has_minimum() {
        assert!(description_width() >= 20);
    }
}
