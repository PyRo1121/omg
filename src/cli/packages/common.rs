//! Common utilities for package operations

#[cfg(feature = "debian")]
use crate::core::env::distro::is_debian_like;

/// Check if we should use Debian backend
pub fn use_debian_backend() -> bool {
    #[cfg(feature = "debian")]
    {
        return is_debian_like();
    }

    #[cfg(not(feature = "debian"))]
    {
        false
    }
}

/// Truncate string to max length with ellipsis
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Find a valid char boundary
        let mut end = max.saturating_sub(3);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Fuzzy match candidate for "Did you mean?"
#[cfg(feature = "arch")]
pub fn fuzzy_suggest(query: &str) -> Option<String> {
    use crate::core::completion::CompletionEngine;
    use crate::core::Database;

    // 1. Get all names (Fast from local package DB)
    let names = if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            crate::package_managers::apt_list_all_package_names().ok()?
        }
        #[cfg(not(feature = "debian"))]
        {
            return None;
        }
    } else {
        #[cfg(feature = "arch")]
        {
            crate::package_managers::alpm_direct::list_all_package_names().ok()?
        }
        #[cfg(not(feature = "arch"))]
        {
            return None;
        }
    };

    // 2. Open DB for engine (Dummy open just to satisfy constructor)
    let db_path = Database::default_path().ok()?;
    let db = Database::open(&db_path).ok()?;
    let engine = CompletionEngine::new(db);

    // 3. Fuzzy Match
    let matches = engine.fuzzy_match(query, names);

    matches.first().cloned()
}

/// Helper to log package transactions to history
#[cfg(feature = "arch")]
pub fn log_transaction(
    ty: crate::core::history::TransactionType,
    changes: Vec<crate::core::history::PackageChange>,
    success: bool,
) {
    if !changes.is_empty() {
        if let Ok(history) = crate::core::history::HistoryManager::new() {
            let _ = history.add_transaction(ty, changes, success);
        }
    }
}
