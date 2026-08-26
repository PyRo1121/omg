//! Contract tests for `src/core/license.rs` (cov-11).
//!
//! Pins observable contracts for tier gating, feature checks, and offline
//! validation against a cached (stored) license token. Every assertion is
//! falsifiable: mutating the protected product code must fail these tests.
//!
//! Untestable offline (documented, not skipped silently): the *positive*
//! path of `StoredLicense::verified_payload` requires an Ed25519 signature
//! made by the production licensing key embedded in the binary
//! (`STUB_JWT_VERIFICATION_KEY`). No externally-crafted token can verify
//! against it, so tests pin the fail-closed behavior instead — which is the
//! security-relevant contract.

pub mod common;

use common::*;
use omg_lib::core::license::{
    Feature, StoredLicense, Tier, current_tier, features_for_tier, get_machine_id, has_feature,
    load_license, remove_license, require_feature, save_license, status,
};

// ══════════════════════════════════════════════════════════════════════════
// Pure contracts: tier metadata and parsing
// ══════════════════════════════════════════════════════════════════════════

const ALL_TIERS: [Tier; 4] = [Tier::Free, Tier::Pro, Tier::Team, Tier::Enterprise];

/// Contract: `Tier::as_str()` round-trips through `Tier::parse`, parsing is
/// case-insensitive, and unknown input is rejected (never coerced).
#[test]
fn tier_names_round_trip_and_unknown_input_is_rejected() {
    for tier in ALL_TIERS {
        assert_eq!(
            Tier::parse(tier.as_str()),
            Some(tier),
            "parse({:?}) must recover the tier",
            tier.as_str()
        );
        // Case-insensitive parsing.
        let upper = tier.as_str().to_uppercase();
        assert_eq!(
            Tier::parse(&upper),
            Some(tier),
            "parse must be case-insensitive for {upper:?}"
        );
    }

    // Unknown input: None via parse(), Err(UnknownTier) via FromStr, whose
    // Display names the cause ("unknown license tier").
    assert_eq!(Tier::parse("premium"), None);
    assert_eq!(Tier::parse(""), None);
    let err = "premium".parse::<Tier>().expect_err("unknown tier");
    assert_eq!(err.to_string(), "unknown license tier");
}

/// Contract: tier pricing and display names are the advertised strings.
/// These appear in upgrade prompts; changing them silently changes user copy.
#[test]
fn tier_display_and_price_strings_are_exact() {
    assert_eq!(Tier::Free.display_name(), "Free");
    assert_eq!(Tier::Pro.display_name(), "Pro");
    assert_eq!(Tier::Team.display_name(), "Team");
    assert_eq!(Tier::Enterprise.display_name(), "Enterprise");

    assert_eq!(Tier::Free.price(), "Free");
    assert_eq!(Tier::Pro.price(), "$9/mo");
    assert_eq!(Tier::Team.price(), "$200/mo");
    assert_eq!(Tier::Enterprise.price(), "Contact us");
}

// ══════════════════════════════════════════════════════════════════════════
// Pure contracts: feature catalog and tier composition
// ══════════════════════════════════════════════════════════════════════════

const ALL_FEATURES: [Feature; 21] = [
    Feature::Packages,
    Feature::Runtimes,
    Feature::Container,
    Feature::EnvCapture,
    Feature::EnvShare,
    Feature::Sbom,
    Feature::Audit,
    Feature::Secrets,
    Feature::Fleet,
    Feature::TeamSync,
    Feature::TeamConfig,
    Feature::AuditLog,
    Feature::Policy,
    Feature::Slsa,
    Feature::Sso,
    Feature::PrioritySupport,
    Feature::EnterpriseReports,
    Feature::AuditExport,
    Feature::LicenseScan,
    Feature::Compliance,
    Feature::SelfHosted,
];

/// Contract: every canonical feature name round-trips through
/// `Feature::from_str`, snake_case/kebab-case aliases map to the same
/// feature, and unknown names return `None`.
#[test]
fn feature_names_round_trip_and_aliases_resolve() {
    for feature in ALL_FEATURES {
        let name = feature.as_str();
        assert_eq!(
            Feature::from_str(name),
            Some(feature),
            "from_str({name:?}) must recover the feature"
        );
        let upper = name.to_uppercase();
        assert_eq!(
            Feature::from_str(&upper),
            Some(feature),
            "from_str must be case-insensitive for {upper:?}"
        );
    }

    // Alias forms resolve to the intended features (not just to *some* tier).
    assert_eq!(Feature::from_str("env_capture"), Some(Feature::EnvCapture));
    assert_eq!(Feature::from_str("env-share"), Some(Feature::EnvShare));
    assert_eq!(
        Feature::from_str("team_sync"),
        Some(Feature::TeamSync),
        "snake_case alias must not fall back to a different feature"
    );
    assert_eq!(
        Feature::from_str("enterprise_policy"),
        Some(Feature::Policy),
        "'enterprise_policy' must be Policy, not a distinct feature"
    );

    assert_eq!(Feature::from_str("sbom-plus"), None);
    assert_eq!(Feature::from_str(""), None);
}

/// Contract: `features_for_tier` is cumulative — each paid tier includes
/// every feature of the tiers below it, with the exact expected sizes
/// (5 free / 3 pro / 4 team / 9 enterprise = 21 total).
#[test]
fn features_for_tier_composition_is_cumulative() {
    let free = features_for_tier(Tier::Free);
    let pro = features_for_tier(Tier::Pro);
    let team = features_for_tier(Tier::Team);
    let enterprise = features_for_tier(Tier::Enterprise);

    assert_eq!(free.len(), 5, "free tier grants exactly 5 features");
    assert_eq!(pro.len(), 8, "pro = 5 free + 3 pro features");
    assert_eq!(team.len(), 12, "team = 5 free + 3 pro + 4 team features");
    assert_eq!(enterprise.len(), 21, "enterprise grants all 21 features");

    // Cumulative superset property.
    for f in &free {
        assert!(pro.contains(f), "pro must include free feature {f:?}");
        assert!(team.contains(f), "team must include free feature {f:?}");
        assert!(
            enterprise.contains(f),
            "enterprise must include free feature {f:?}"
        );
    }
    for &f in features_for_tier(Tier::Pro).iter().skip(5) {
        assert!(team.contains(&f), "team must include pro feature {f:?}");
        assert!(
            enterprise.contains(&f),
            "enterprise must include pro feature {f:?}"
        );
    }
    for &f in features_for_tier(Tier::Team).iter().skip(8) {
        assert!(
            enterprise.contains(&f),
            "enterprise must include team feature {f:?}"
        );
    }

    // Downgrade direction: lower tiers must NOT leak higher-tier features.
    assert!(
        !free.contains(&&Feature::Sbom),
        "free must not include sbom"
    );
    assert!(
        !pro.contains(&&Feature::TeamSync),
        "pro must not include team-sync"
    );
    assert!(
        !team.contains(&&Feature::Sso),
        "team must not include enterprise-only sso"
    );

    // Enterprise-specific anchors survive catalog edits.
    for name in ["self-hosted", "sso", "slsa", "compliance"] {
        let parsed = Feature::from_str(name).expect("anchor feature must exist");
        assert!(
            enterprise.contains(&&parsed),
            "enterprise must include {name}"
        );
    }
}

/// Contract: full feature → minimum-tier mapping as advertised in docs.
#[test]
fn every_feature_requires_its_documented_tier() {
    let cases: [(Feature, Tier); 21] = [
        (Feature::Packages, Tier::Free),
        (Feature::Runtimes, Tier::Free),
        (Feature::Container, Tier::Free),
        (Feature::EnvCapture, Tier::Free),
        (Feature::EnvShare, Tier::Free),
        (Feature::Sbom, Tier::Pro),
        (Feature::Audit, Tier::Pro),
        (Feature::Secrets, Tier::Pro),
        (Feature::Fleet, Tier::Team),
        (Feature::TeamSync, Tier::Team),
        (Feature::TeamConfig, Tier::Team),
        (Feature::AuditLog, Tier::Team),
        (Feature::Policy, Tier::Enterprise),
        (Feature::Slsa, Tier::Enterprise),
        (Feature::Sso, Tier::Enterprise),
        (Feature::PrioritySupport, Tier::Enterprise),
        (Feature::EnterpriseReports, Tier::Enterprise),
        (Feature::AuditExport, Tier::Enterprise),
        (Feature::LicenseScan, Tier::Enterprise),
        (Feature::Compliance, Tier::Enterprise),
        (Feature::SelfHosted, Tier::Enterprise),
    ];
    for (feature, tier) in cases {
        assert_eq!(
            feature.required_tier(),
            tier,
            "{feature:?} must require {tier:?}"
        );
    }
}

/// Contract: machine fingerprint is stable within a process, exactly 16
/// lowercase hex characters (truncated SHA-256).
#[test]
fn machine_id_is_stable_sixteen_lowercase_hex() {
    let first = get_machine_id();
    let second = get_machine_id();
    assert_eq!(first, second, "machine id must be stable within a process");
    assert_eq!(
        first.len(),
        16,
        "machine id must be 16 chars, got {}",
        first.len()
    );
    assert!(
        first
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "machine id must be lowercase hex, got {first:?}"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Stateful contracts: persistence, gating, offline validation
// (each isolates its own OMG_DATA_DIR and runs #[serial])
// ══════════════════════════════════════════════════════════════════════════

fn sample_license(key: &str, tier: &str, token: Option<&str>) -> StoredLicense {
    StoredLicense {
        key: key.to_string(),
        tier: tier.to_string(),
        features: vec!["sbom".to_string()],
        customer: Some("acme".to_string()),
        expires_at: Some("2030-01-01".to_string()),
        validated_at: 1_700_000_000,
        token: token.map(str::to_string),
        machine_id: Some(get_machine_id()),
    }
}

/// Contract: `save_license` persists `<OMG_DATA_DIR>/license.json` readable
/// only by the owner (0600), `load_license` restores it field-for-field,
/// `remove_license` deletes it, is idempotent when absent, and loading after
/// removal yields `None`.
#[test]
#[serial]
fn license_persistence_round_trips_with_owner_only_permissions() {
    let dir = tempfile::TempDir::new().expect("data dir");
    let data_dir = dir.path().to_string_lossy().into_owned();

    with_test_env(&[("OMG_DATA_DIR", &data_dir)], || {
        let stored = sample_license("OMG-KEY-PERSIST", "pro", Some("cached.jwt.token"));

        save_license(&stored).expect("save must succeed");

        let path = dir.path().join("license.json");
        assert!(
            path.is_file(),
            "license.json must be written into the data dir"
        );

        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("license metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "license.json must be owner-only (0600)");

        let loaded = load_license().expect("saved license must reload");
        assert_eq!(loaded.key, "OMG-KEY-PERSIST");
        assert_eq!(loaded.tier, "pro");
        assert_eq!(loaded.features, vec!["sbom"]);
        assert_eq!(loaded.customer.as_deref(), Some("acme"));
        assert_eq!(loaded.expires_at.as_deref(), Some("2030-01-01"));
        assert_eq!(loaded.validated_at, 1_700_000_000);
        assert_eq!(loaded.token.as_deref(), Some("cached.jwt.token"));
        // status() mirrors load_license().
        assert_eq!(
            status().expect("status mirrors load").key,
            "OMG-KEY-PERSIST"
        );

        remove_license().expect("remove must succeed");
        assert!(!path.exists(), "remove_license must delete license.json");
        assert!(load_license().is_none(), "removed license must not reload");

        // Removing again without a file is still Ok (idempotent).
        remove_license().expect("removing a missing license stays Ok");
    });
}

/// Contract: a malformed license.json is integrity-relevant state — it is
/// reported (warned) and treated as *no license*, never as a fabricated one.
#[test]
#[serial]
fn corrupt_license_file_degrades_to_no_license() {
    let dir = tempfile::TempDir::new().expect("data dir");
    std::fs::write(dir.path().join("license.json"), "{not json at all")
        .expect("seed corrupt license");
    let data_dir = dir.path().to_string_lossy().into_owned();

    with_test_env(&[("OMG_DATA_DIR", &data_dir)], || {
        assert!(
            load_license().is_none(),
            "corrupt license must load as none"
        );
        assert!(status().is_none(), "corrupt license must report none");
    });
}

/// Contract: with no license at all, the effective tier is Free — free
/// features pass, paid features are denied with the exact upgrade hint
/// naming the tier, its price, and the pricing URL.
#[test]
#[serial]
fn no_license_yields_free_tier_and_exact_upgrade_hint_on_denial() {
    let dir = tempfile::TempDir::new().expect("data dir");
    let data_dir = dir.path().to_string_lossy().into_owned();

    with_test_env(&[("OMG_DATA_DIR", &data_dir)], || {
        assert_eq!(current_tier(), Tier::Free, "no license means Free tier");

        // Free features pass without any license.
        assert!(has_feature("packages"), "packages must be free");
        assert!(
            require_feature("packages").is_ok(),
            "free feature must not error"
        );

        // Paid features denied, with the exact advertised message.
        assert!(!has_feature("sbom"), "sbom needs a Pro license");
        let err = require_feature("sbom").expect_err("denied feature must error");
        assert_eq!(
            err.to_string(),
            "Feature 'sbom' requires Pro tier ($9/mo). \
             Upgrade at https://pyro1121.com/pricing"
        );

        // Unknown features are denied with a distinct cause.
        assert!(!has_feature("teleport"));
        let err = require_feature("teleport").expect_err("unknown feature must error");
        assert!(
            err.to_string().contains("Unknown feature 'teleport'"),
            "got: {err}"
        );
    });
}

/// Contract (offline validation, fail-closed): a stored license whose cached
/// JWT was not produced by the production signing key gates as Free through
/// the FULL gating path (`current_tier`, `has_feature`, `require_feature`),
/// even though the plaintext `tier` field claims enterprise. The tamper
/// evidence must also surface on the loaded struct itself.
#[test]
#[serial]
fn stored_enterprise_claim_without_verifiable_token_gates_as_free() {
    let dir = tempfile::TempDir::new().expect("data dir");
    let license_json = serde_json::json!({
        "key": "OMG-ENT-CLAIMED",
        "tier": "enterprise",
        "features": ["policy", "sso"],
        "customer": "acme",
        "expires_at": null,
        "validated_at": 1_700_000_000,
        "token": "eyJhbGciOiJFZERTQSJ9.forged.claims",
        "machine_id": get_machine_id(),
    });
    std::fs::write(
        dir.path().join("license.json"),
        serde_json::to_string(&license_json).unwrap(),
    )
    .expect("seed claimed-enterprise license");
    let data_dir = dir.path().to_string_lossy().into_owned();

    with_test_env(&[("OMG_DATA_DIR", &data_dir)], || {
        let stored = load_license().expect("well-formed file must load");
        assert_eq!(stored.tier, "enterprise", "plaintext claim is preserved");
        assert!(!stored.is_token_valid(), "forged token must not validate");
        assert_eq!(
            stored.tier_enum(),
            Tier::Free,
            "unverifiable token degrades to Free"
        );

        assert_eq!(
            current_tier(),
            Tier::Free,
            "gating must follow the verified token, not the plaintext tier"
        );
        assert!(
            !has_feature("policy"),
            "forged enterprise must not unlock policy"
        );
        assert!(has_feature("packages"), "free features remain available");

        let err = require_feature("sso").expect_err("forged enterprise must not unlock sso");
        assert_eq!(
            err.to_string(),
            "Feature 'sso' requires Enterprise tier (Contact us). \
             Upgrade at https://pyro1121.com/pricing"
        );
    });
}
