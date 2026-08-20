//! Shared human-readable formatting helpers.
//!
//! Single home for display formatting used by the CLI, TUI, and standalone
//! binaries so truncation and size rules stay consistent everywhere.

/// Truncate a string to a maximum byte length, respecting UTF-8 char
/// boundaries. Appends `"..."` when truncation occurs.
///
/// The result never exceeds `max` bytes.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Reserve room for the ellipsis, then back off to a char boundary so the
    // slice stays valid UTF-8 even for multibyte input.
    let mut end = max.saturating_sub(3);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

/// Format a byte count as a human-readable size (e.g., `"1.5 MB"`, `"342 B"`).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    let unit = UNITS[unit_index];
    if unit_index == 0 {
        format!("{bytes} {unit}")
    } else {
        format!("{size:.1} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_returns_short_strings_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_appends_ellipsis_and_stays_within_limit() {
        assert_eq!(truncate("hello world", 5), "he...");
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_respects_utf8_boundaries() {
        // Regression test: an earlier copy in the tea module sliced at raw
        // byte offsets and panicked when the cut landed inside a multibyte
        // character.
        assert_eq!(truncate("日本語テキスト", 8), "日...");
    }

    #[test]
    fn truncate_with_tiny_max_yields_ellipsis_only() {
        assert_eq!(truncate("hello", 2), "...");
    }

    #[test]
    fn format_bytes_uses_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(1_099_511_627_776), "1.0 TB");
    }
}
