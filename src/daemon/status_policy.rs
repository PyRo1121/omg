//! Status-publication policy for the daemon.
//!
//! These rules are domain policy, not wire format: a failed vulnerability
//! scan must never invent a clean zero, and only completed scans may be
//! cached or published. They live apart from [`super::protocol`] so the wire
//! types stay declarative and the invariants stay tested in one place.

use super::protocol::StatusResult;

/// Map a vulnerability scan onto a previously published count.
///
/// A failed scan keeps the previous count and must not invent a clean zero.
pub fn vulnerability_count_from_scan<E>(
    scan: &Result<usize, E>,
    previous: Option<usize>,
) -> Option<usize> {
    match scan {
        Ok(count) => Some(*count),
        Err(_) => previous,
    }
}

/// Build a status snapshot. Only a completed vulnerability scan may be cached.
pub fn status_snapshot(
    total_packages: usize,
    explicit_packages: usize,
    orphan_packages: usize,
    updates_available: usize,
    runtime_versions: Vec<(String, String)>,
    scanned_vulnerabilities: Option<usize>,
) -> (StatusResult, bool) {
    (
        StatusResult {
            total_packages,
            explicit_packages,
            orphan_packages,
            updates_available,
            security_vulnerabilities: scanned_vulnerabilities.unwrap_or(0),
            vulnerabilities_scanned: scanned_vulnerabilities.is_some(),
            runtime_versions,
        },
        scanned_vulnerabilities.is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::{status_snapshot, vulnerability_count_from_scan};

    #[test]
    fn successful_scan_replaces_the_previous_count() {
        assert_eq!(
            vulnerability_count_from_scan::<()>(&Ok(3), Some(5)),
            Some(3)
        );
    }

    #[test]
    fn failed_scan_keeps_the_previous_count() {
        assert_eq!(
            vulnerability_count_from_scan(&Err("alsa unavailable"), Some(5)),
            Some(5)
        );
    }

    #[test]
    fn failed_scan_without_a_prior_count_does_not_invent_zero() {
        assert_eq!(
            vulnerability_count_from_scan(&Err("alsa unavailable"), None),
            None
        );
    }

    #[test]
    fn unscanned_status_is_not_cacheable() {
        let (status, cacheable) = status_snapshot(10, 4, 1, 2, vec![], None);
        assert!(!cacheable);
        assert!(!status.vulnerabilities_scanned);
        assert_eq!(status.scanned_vulnerability_count(), None);
        assert_eq!(status.security_vulnerabilities, 0);
        assert_eq!(status.total_packages, 10);
    }

    #[test]
    fn scanned_status_is_cacheable() {
        let (status, cacheable) = status_snapshot(10, 4, 1, 2, vec![], Some(7));
        assert!(cacheable);
        assert!(status.vulnerabilities_scanned);
        assert_eq!(status.scanned_vulnerability_count(), Some(7));
        assert_eq!(status.security_vulnerabilities, 7);
    }
}
