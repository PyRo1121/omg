//! Runtime executable resolution.

use std::path::PathBuf;

/// Find an executable in the system `PATH`.
pub fn find_in_path(binary: &str) -> Option<PathBuf> {
    which::which(binary).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_in_path_finds_known_binaries() {
        // sh should exist on all Unix systems
        #[cfg(unix)]
        assert!(find_in_path("sh").is_some());
    }

    #[test]
    fn test_find_in_path_returns_none_for_nonexistent() {
        assert!(find_in_path("this-binary-definitely-does-not-exist-12345").is_none());
    }
}
