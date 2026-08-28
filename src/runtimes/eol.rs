//! Shared runtime end-of-life data (audit typ06 C-5).
//!
//! Previously duplicated between `cli/doctor.rs` (prefix-string matching)
//! and `cli/security.rs` (component-vector matching) with DIFFERENT entries.
//! This is the single canonical table.

/// One EOL entry: match on runtime name + version component prefix.
pub(crate) struct EolEntry {
    pub runtime: &'static str,
    pub version_prefix: &'static [u64],
    pub eol_date: &'static str,
}

/// Canonical EOL table sourced from endoflife.date (last reviewed 2026-08-28).
pub(crate) const EOL_TABLE: &[EolEntry] = &[
    EolEntry {
        runtime: "node",
        version_prefix: &[16],
        eol_date: "2023-09-11",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[18],
        eol_date: "2025-04-30",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[19],
        eol_date: "2023-06-01",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[20],
        eol_date: "2026-04-30",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[21],
        eol_date: "2024-06-01",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[22],
        eol_date: "2027-04-30",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[23],
        eol_date: "2025-06-01",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[24],
        eol_date: "2028-04-30",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[25],
        eol_date: "2026-06-01",
    },
    EolEntry {
        runtime: "node",
        version_prefix: &[26],
        eol_date: "2029-04-30",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 7],
        eol_date: "2023-06-27",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 8],
        eol_date: "2024-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 9],
        eol_date: "2025-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 10],
        eol_date: "2026-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 11],
        eol_date: "2027-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 12],
        eol_date: "2028-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 13],
        eol_date: "2029-10-31",
    },
    EolEntry {
        runtime: "python",
        version_prefix: &[3, 14],
        eol_date: "2030-10-31",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 19],
        eol_date: "2023-09-06",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 20],
        eol_date: "2024-02-06",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 21],
        eol_date: "2024-08-13",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 22],
        eol_date: "2025-02-11",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 23],
        eol_date: "2025-08-12",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 24],
        eol_date: "2026-02-10",
    },
    EolEntry {
        runtime: "go",
        version_prefix: &[1, 25],
        eol_date: "2026-08-19",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[2, 7],
        eol_date: "2023-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[3, 0],
        eol_date: "2024-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[3, 1],
        eol_date: "2025-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[3, 2],
        eol_date: "2026-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[3, 3],
        eol_date: "2027-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[3, 4],
        eol_date: "2028-03-31",
    },
    EolEntry {
        runtime: "ruby",
        version_prefix: &[4, 0],
        eol_date: "2029-03-31",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[8],
        eol_date: "2030-12-31",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[11],
        eol_date: "2026-09-30",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[17],
        eol_date: "2029-09-30",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[18],
        eol_date: "2022-09-20",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[19],
        eol_date: "2023-03-21",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[20],
        eol_date: "2023-09-19",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[21],
        eol_date: "2031-09-30",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[22],
        eol_date: "2024-09-17",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[23],
        eol_date: "2025-03-18",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[24],
        eol_date: "2025-09-16",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[25],
        eol_date: "2033-09-30",
    },
    EolEntry {
        runtime: "java",
        version_prefix: &[26],
        eol_date: "2026-09-18",
    },
];

/// Numeric components of a decorated runtime version.
///
/// Parsing stops at the first non-numeric suffix, so distro revisions do not
/// affect runtime lifecycle matching.
#[must_use]
pub(crate) fn version_components(version: &str) -> Vec<u64> {
    let numeric_prefix = version
        .strip_prefix(['v', 'V'])
        .unwrap_or(version)
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .next()
        .unwrap_or("");
    numeric_prefix
        .split('.')
        .map_while(|part| part.parse::<u64>().ok())
        .collect()
}

/// Find the EOL entry matching a runtime + installed version components.
/// Component-prefix match prevents Python `3.13` from ever matching `3.1`.
pub(crate) fn find_eol_entry(
    runtime: &str,
    version_components: &[u64],
) -> Option<&'static EolEntry> {
    EOL_TABLE
        .iter()
        .find(|e| e.runtime == runtime && version_components.starts_with(e.version_prefix))
}

/// Return the UTC timestamp six calendar months after `now`.
///
/// Calendar arithmetic must run on [`jiff::Zoned`]. `jiff::Timestamp` only
/// supports fixed-duration units and rejects spans containing months.
pub(crate) fn eol_warning_cutoff(now: jiff::Timestamp) -> Result<jiff::Timestamp, jiff::Error> {
    now.to_zoned(jiff::tz::TimeZone::UTC)
        .checked_add(jiff::Span::new().months(6))
        .map(|zoned| zoned.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_3_13_does_not_match_3_1() {
        let e = find_eol_entry("python", &[3, 13]);
        assert!(e.is_some());
        assert_eq!(e.unwrap().version_prefix, &[3, 13]);
    }

    #[test]
    fn node_major_only_matches_full_prefix() {
        // node 16 must match [16] but NOT [160]
        assert!(find_eol_entry("node", &[16]).is_some());
        assert!(find_eol_entry("node", &[160]).is_none());
    }

    #[test]
    fn unknown_runtime_has_no_eol_entry() {
        assert!(find_eol_entry("rust", &[1, 75]).is_none());
    }

    #[test]
    fn lifecycle_table_contains_recent_eol_cycles() {
        let expected = [
            ("node", &[23][..], "2025-06-01"),
            ("node", &[25][..], "2026-06-01"),
            ("python", &[3, 14][..], "2030-10-31"),
            ("go", &[1, 23][..], "2025-08-12"),
            ("go", &[1, 24][..], "2026-02-10"),
            ("go", &[1, 25][..], "2026-08-19"),
            ("ruby", &[3, 4][..], "2028-03-31"),
            ("java", &[24][..], "2025-09-16"),
        ];

        for (runtime, version, eol_date) in expected {
            assert_eq!(
                find_eol_entry(runtime, version).map(|entry| entry.eol_date),
                Some(eol_date),
                "missing or stale lifecycle row for {runtime} {version:?}"
            );
        }
    }

    #[test]
    fn warning_cutoff_uses_calendar_months_in_utc() {
        let now = "2026-01-15T12:00:00Z"
            .parse::<jiff::Timestamp>()
            .expect("valid timestamp");
        let expected = "2026-07-15T12:00:00Z"
            .parse::<jiff::Timestamp>()
            .expect("valid timestamp");

        assert_eq!(
            eol_warning_cutoff(now).expect("calendar arithmetic"),
            expected
        );
    }
}
