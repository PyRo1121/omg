//! Advanced property-based testing for version comparison
//!
//! Tests algebraic properties of version ordering that must hold
//! for correct dependency resolution and update detection.
//!
//! Run: `cargo test --test property_version_advanced --features arch`

#![cfg(feature = "arch")]
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use std::cmp::Ordering;
use std::str::FromStr;

use alpm_types::Version as AlpmVersion;

// ============================================================================
// Property 1: Reflexivity - v == v
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        max_shrink_iters: 10000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_reflexivity(
        major in 0u32..1000u32,
        minor in 0u32..1000u32,
        patch in 0u32..1000u32,
        pkgrel in 1u32..100u32
    ) {
        let version_str = format!("{}.{}.{}-{}", major, minor, patch, pkgrel);

        if let Ok(v) = AlpmVersion::from_str(&version_str) {
            // Reflexivity: v == v
            prop_assert_eq!(v.cmp(&v), Ordering::Equal);

            // Clone should also be equal
            let v2 = v.clone();
            prop_assert_eq!(&v, &v2);
            prop_assert_eq!(v.cmp(&v2), Ordering::Equal);
        }
    }
}

// ============================================================================
// Property 2: Transitivity - if a > b and b > c, then a > c
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_transitivity(
        v1_major in 0u32..100u32,
        v2_major in 0u32..100u32,
        v3_major in 0u32..100u32,
        minor in 0u32..100u32,
        patch in 0u32..100u32,
        pkgrel in 1u32..10u32
    ) {
        // Sort to ensure v1 <= v2 <= v3
        let mut versions = vec![v1_major, v2_major, v3_major];
        versions.sort_unstable();

        let v1_str = format!("{}.{}.{}-{}", versions[0], minor, patch, pkgrel);
        let v2_str = format!("{}.{}.{}-{}", versions[1], minor, patch, pkgrel);
        let v3_str = format!("{}.{}.{}-{}", versions[2], minor, patch, pkgrel);

        if let (Ok(v1), Ok(v2), Ok(v3)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str),
            AlpmVersion::from_str(&v3_str)
        ) {
            // Transitivity: if v1 <= v2 and v2 <= v3, then v1 <= v3
            if v1 <= v2 && v2 <= v3 {
                prop_assert!(v1 <= v3, "Transitivity violated: {} <= {} <= {} but {} > {}",
                    v1_str, v2_str, v3_str, v1_str, v3_str);
            }

            // Also test strict ordering
            if v1 < v2 && v2 < v3 {
                prop_assert!(v1 < v3, "Strict transitivity violated");
            }
        }
    }
}

// ============================================================================
// Property 3: Antisymmetry - if a > b, then !(b > a)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_antisymmetry(
        major1 in 0u32..100u32,
        major2 in 0u32..100u32,
        minor in 0u32..100u32,
        patch in 0u32..100u32
    ) {
        let v1_str = format!("{}.{}.{}", major1, minor, patch);
        let v2_str = format!("{}.{}.{}", major2, minor, patch);

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            // Antisymmetry: if v1 > v2, then !(v2 > v1)
            let ordering = v1.cmp(&v2);
            match ordering {
                Ordering::Greater => {
                    let reverse = v2.cmp(&v1);
                    prop_assert!(matches!(reverse, Ordering::Less),
                        "Antisymmetry violated: {} > {} but {} >= {}",
                        v1_str, v2_str, v2_str, v1_str);
                }
                Ordering::Less => {
                    let reverse = v2.cmp(&v1);
                    prop_assert!(matches!(reverse, Ordering::Greater),
                        "Antisymmetry violated: {} < {} but {} <= {}",
                        v1_str, v2_str, v2_str, v1_str);
                }
                Ordering::Equal => {
                    prop_assert_eq!(&v1, &v2);
                    prop_assert_eq!(v2.cmp(&v1), Ordering::Equal);
                }
            }
        }
    }
}

// ============================================================================
// Property 4: Totality - any two versions are comparable
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_totality(
        v1_major in 0u32..100u32,
        v1_minor in 0u32..100u32,
        v2_major in 0u32..100u32,
        v2_minor in 0u32..100u32
    ) {
        let v1_str = format!("{}.{}.0", v1_major, v1_minor);
        let v2_str = format!("{}.{}.0", v2_major, v2_minor);

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            // Totality: exactly one of v1 < v2, v1 == v2, or v1 > v2 must be true
            let ordering = v1.cmp(&v2);
            prop_assert!(
                matches!(ordering, Ordering::Less | Ordering::Equal | Ordering::Greater),
                "Totality violated: comparison returned invalid result"
            );

            // Verify consistency
            match ordering {
                Ordering::Less => {
                    prop_assert!(v1 < v2);
                    prop_assert!(!(v1 == v2));
                    prop_assert!(!(v1 > v2));
                }
                Ordering::Equal => {
                    prop_assert!(!(v1 < v2));
                    prop_assert!(v1 == v2);
                    prop_assert!(!(v1 > v2));
                }
                Ordering::Greater => {
                    prop_assert!(!(v1 < v2));
                    prop_assert!(!(v1 == v2));
                    prop_assert!(v1 > v2);
                }
            }
        }
    }
}

// ============================================================================
// Property 5: Epoch dominates all other fields
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 500,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_epoch_dominates(
        epoch1 in 0u32..10u32,
        epoch2 in 0u32..10u32,
        major1 in 0u32..100u32,
        major2 in 0u32..100u32,
        minor1 in 0u32..100u32,
        minor2 in 0u32..100u32,
        patch1 in 0u32..100u32,
        patch2 in 0u32..100u32
    ) {
        let v1_str = format!("{}:{}.{}.{}", epoch1, major1, minor1, patch1);
        let v2_str = format!("{}:{}.{}.{}", epoch2, major2, minor2, patch2);

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            // Epoch comparison dominates all other fields
            match epoch1.cmp(&epoch2) {
                Ordering::Greater => {
                    prop_assert!(v1 > v2,
                        "Epoch dominance violated: epoch {} > {} but {} <= {}",
                        epoch1, epoch2, v1_str, v2_str);
                }
                Ordering::Less => {
                    prop_assert!(v1 < v2,
                        "Epoch dominance violated: epoch {} < {} but {} >= {}",
                        epoch1, epoch2, v1_str, v2_str);
                }
                Ordering::Equal => {
                    // When epochs equal, normal version comparison applies
                    let expected = (major1, minor1, patch1).cmp(&(major2, minor2, patch2));
                    prop_assert_eq!(v1.cmp(&v2), expected,
                        "Version comparison incorrect when epochs equal");
                }
            }
        }
    }
}

// ============================================================================
// Property 6: pkgrel acts as tiebreaker
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 500,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_pkgrel_tiebreaker(
        major in 0u32..100u32,
        minor in 0u32..100u32,
        patch in 0u32..100u32,
        pkgrel1 in 1u32..50u32,
        pkgrel2 in 1u32..50u32
    ) {
        let v1_str = format!("{}.{}.{}-{}", major, minor, patch, pkgrel1);
        let v2_str = format!("{}.{}.{}-{}", major, minor, patch, pkgrel2);

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            // pkgrel acts as tiebreaker when version equal
            match pkgrel1.cmp(&pkgrel2) {
                Ordering::Greater => {
                    prop_assert!(v1 > v2,
                        "pkgrel tiebreaker violated: pkgrel {} > {} but {} <= {}",
                        pkgrel1, pkgrel2, v1_str, v2_str);
                }
                Ordering::Less => {
                    prop_assert!(v1 < v2,
                        "pkgrel tiebreaker violated: pkgrel {} < {} but {} >= {}",
                        pkgrel1, pkgrel2, v1_str, v2_str);
                }
                Ordering::Equal => {
                    prop_assert_eq!(v1, v2,
                        "Versions with same epoch, version, and pkgrel should be equal");
                }
            }
        }
    }
}

// ============================================================================
// Property 7: Version parsing never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_parsing_never_panics(
        major in 0u32..10000u32,
        minor in 0u32..10000u32,
        patch in 0u32..10000u32,
        pkgrel in 1u32..1000u32,
        prefix in "[a-z]{0,5}",
        suffix in "[a-z0-9.\\-]{0,20}"
    ) {
        let version_str = format!("{}{}.{}.{}-{}{}", prefix, major, minor, patch, pkgrel, suffix);

        // Should never panic, even on malformed input
        let _ = AlpmVersion::from_str(&version_str);
    }
}

// ============================================================================
// Property 8: Display round-trip (if parse succeeds, display should work)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 500,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_display_roundtrip(
        major in 0u32..1000u32,
        minor in 0u32..1000u32,
        patch in 0u32..1000u32,
        pkgrel in 1u32..100u32
    ) {
        let version_str = format!("{}.{}.{}-{}", major, minor, patch, pkgrel);

        if let Ok(v) = AlpmVersion::from_str(&version_str) {
            // Display should not panic
            let displayed = format!("{}", v);
            prop_assert!(!displayed.is_empty(), "Display produced empty string");

            // Display should be parseable (though format may differ)
            let _ = AlpmVersion::from_str(&displayed);
        }
    }
}

// ============================================================================
// Property 9: Comparison consistency with Ord trait
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 500,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_ord_consistency(
        v1_major in 0u32..100u32,
        v2_major in 0u32..100u32,
        minor in 0u32..100u32
    ) {
        let v1_str = format!("{}.{}.0", v1_major, minor);
        let v2_str = format!("{}.{}.0", v2_major, minor);

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            // Ord trait should be consistent with cmp()
            let cmp_result = v1.cmp(&v2);
            let ord_result = match (v1 < v2, v1 == v2, v1 > v2) {
                (true, false, false) => Ordering::Less,
                (false, true, false) => Ordering::Equal,
                (false, false, true) => Ordering::Greater,
                _ => panic!("Inconsistent Ord trait implementation"),
            };

            prop_assert_eq!(cmp_result, ord_result,
                "cmp() and Ord trait inconsistent for {} vs {}",
                v1_str, v2_str);
        }
    }
}

// ============================================================================
// Property 10: Sorting stability
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 100,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_version_sorting_stability(
        versions in prop::collection::vec(
            (0u32..100u32, 0u32..100u32, 0u32..100u32),
            5..20
        )
    ) {
        let version_strs: Vec<String> = versions.iter()
            .map(|(major, minor, patch)| format!("{}.{}.{}", major, minor, patch))
            .collect();

        let parsed_versions: Vec<AlpmVersion> = version_strs.iter()
            .filter_map(|s| AlpmVersion::from_str(s).ok())
            .collect();

        if !parsed_versions.is_empty() {
            let mut sorted1 = parsed_versions.clone();
            let mut sorted2 = parsed_versions.clone();

            // Sort twice - results should be identical
            sorted1.sort();
            sorted2.sort();

            // Compare before consuming sorted1
            for i in 1..sorted1.len() {
                let prev = &sorted1[i - 1];
                let curr = &sorted1[i];
                prop_assert!(prev <= curr,
                    "Sorted list not in order: {} > {}",
                    prev, curr);
            }

            // Now check equality
            prop_assert_eq!(sorted1, sorted2, "Sorting is not stable");
        }
    }
}
