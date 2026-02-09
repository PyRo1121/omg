//! Simplified property-based testing for version comparison
//!
//! Run: `cargo test --test property_version_advanced_simple --features arch`

#![cfg(feature = "arch")]
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use std::cmp::Ordering;
use std::str::FromStr;

use alpm_types::Version as AlpmVersion;

proptest! {
    #[test]
    fn prop_version_parsing_never_panics(
        major in 0u32..1000u32,
        minor in 0u32..1000u32,
        patch in 0u32..1000u32,
        pkgrel in 1u32..100u32
    ) {
        let version_str = format!("{major}.{minor}.{patch}-{pkgrel}");
        // Should never panic
        let _ = AlpmVersion::from_str(&version_str);
    }

    #[test]
    fn prop_version_reflexivity(
        major in 0u32..1000u32,
        minor in 0u32..1000u32
    ) {
        let v_str = format!("{major}.{minor}.0");
        if let Ok(v) = AlpmVersion::from_str(&v_str) {
            // v should equal itself
            prop_assert_eq!(v.cmp(&v), Ordering::Equal);
        }
    }

    #[test]
    fn prop_version_transitivity(
        v1_major in 0u32..100u32,
        v2_major in 0u32..100u32,
        v3_major in 0u32..100u32
    ) {
        #[allow(clippy::tuple_array_conversions)]
        let mut versions = [v1_major, v2_major, v3_major];
        versions.sort_unstable();

        let v1_str = format!("{}.0.0", versions[0]);
        let v2_str = format!("{}.0.0", versions[1]);
        let v3_str = format!("{}.0.0", versions[2]);

        if let (Ok(v1), Ok(v2), Ok(v3)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str),
            AlpmVersion::from_str(&v3_str)
        ) {
            // Transitivity
            if v1 <= v2 && v2 <= v3 {
                prop_assert!(v1 <= v3);
            }
        }
    }

    #[test]
    fn prop_version_epoch_dominates(
        epoch1 in 0u32..10u32,
        epoch2 in 0u32..10u32,
        major in 0u32..100u32
    ) {
        let v1_str = format!("{epoch1}:{major}.0.0");
        let v2_str = format!("{epoch2}:{major}.0.0");

        if let (Ok(v1), Ok(v2)) = (
            AlpmVersion::from_str(&v1_str),
            AlpmVersion::from_str(&v2_str)
        ) {
            match epoch1.cmp(&epoch2) {
                Ordering::Greater => prop_assert!(v1 > v2),
                Ordering::Less => prop_assert!(v1 < v2),
                Ordering::Equal => prop_assert_eq!(v1.cmp(&v2), Ordering::Equal),
            }
        }
    }
}
