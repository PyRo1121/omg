//! Error suggestion helpers for user-facing CLI failures.
//!
//! The former typed `OmgError` contract was deleted (wave-9): production code
//! uniformly uses `anyhow::Error`, so pattern-matching on message text at the
//! single CLI exit boundary is the only consumer of error context.

/// Common error suggestions for anyhow errors
#[cold]
#[must_use]
pub fn suggest_for_anyhow(err: &anyhow::Error) -> Option<&'static str> {
    let msg = err.to_string().to_lowercase();

    if msg.contains("package not found") || msg.contains("no such package") {
        return Some("Try: omg search <query> to find available packages");
    }
    if msg.contains("version not found") || msg.contains("no matching version") {
        return Some("Try: omg list <runtime> --available to see available versions");
    }
    if msg.contains("permission denied") || msg.contains("access denied") {
        return Some("Try running with sudo, or check file/directory permissions");
    }
    if msg.contains("connection") || msg.contains("network") || msg.contains("timeout") {
        return Some("Check your internet connection and try again");
    }
    if msg.contains("not found") && msg.contains("command") {
        return Some("The required tool is not installed. Try: omg tool install <name>");
    }
    if msg.contains("daemon") {
        return Some("Start the daemon with: omg daemon");
    }
    if msg.contains("rate limit") || msg.contains("too many requests") {
        return Some("Wait for the cooldown period, then retry your request");
    }
    if msg.contains("no such file") || msg.contains("file not found") {
        return Some("Check that the file path is correct and the file exists");
    }
    if msg.contains("lock") && (msg.contains("exists") || msg.contains("conflict")) {
        return Some(
            "A lock file exists. Another process might be running, or remove the lock file manually.",
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_for_anyhow_permission() {
        let err = anyhow::anyhow!("permission denied: /etc/foo");
        assert!(suggest_for_anyhow(&err).is_some());
    }

    #[test]
    fn test_suggest_for_anyhow_network() {
        let err = anyhow::anyhow!("connection refused");
        assert!(suggest_for_anyhow(&err).is_some());
    }

    #[test]
    fn test_suggest_for_anyhow_none() {
        let err = anyhow::anyhow!("some random error");
        assert!(suggest_for_anyhow(&err).is_none());
    }
}
