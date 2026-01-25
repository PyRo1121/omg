#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Edge case testing for the TDD suite
//!
//! This file focuses on obscure error paths and "absolute everything" testing.

use omg_lib::core::security::validation;

#[cfg(not(feature = "arch"))]
use omg_lib::package_managers::types::parse_version_or_zero;
#[test]
#[cfg(not(feature = "arch"))]
fn test_version_parsing_edge_cases() {
    // Standard versions
    assert_eq!(parse_version_or_zero("1.2.3"), "1.2.3");

    // Obscure versions - without arch feature, these return as-is
    assert_eq!(parse_version_or_zero(""), "");
    assert_eq!(parse_version_or_zero("   "), "   ");
    assert_eq!(parse_version_or_zero("v1.0"), "v1.0");
    assert_eq!(
        parse_version_or_zero("99999999999999999999999999"),
        "99999999999999999999999999"
    );
}

#[test]
fn test_package_name_validation_rigorous() {
    // Valid names
    assert!(validation::validate_package_name("vim").is_ok());
    assert!(validation::validate_package_name("lib-6.0").is_ok());

    // Invalid names (Injection attempts)
    assert!(validation::validate_package_name("vim; rm -rf /").is_err());
    assert!(validation::validate_package_name("vim|grep").is_err());
    assert!(validation::validate_package_name("$(id)").is_err());
    assert!(validation::validate_package_name("../etc/passwd").is_err());

    // Boundary cases
    assert!(validation::validate_package_name("").is_err());
    assert!(validation::validate_package_name(&"a".repeat(256)).is_err());
}

#[tokio::test]
async fn test_daemon_protocol_boundaries() {
    use omg_lib::daemon::protocol::Request;

    // Test large request ID
    let req = Request::Ping { id: u64::MAX };
    assert_eq!(req.id(), u64::MAX);

    // Test batch with maximum items
    let mut batch = Vec::new();
    for i in 0..100 {
        batch.push(Request::Ping {
            id: u64::try_from(i).unwrap(),
        });
    }
    let req_batch = Request::Batch {
        id: 0,
        requests: Box::new(batch),
    };
    assert_eq!(req_batch.id(), 0);
}

#[test]
fn test_path_sanitization_rigorous() {
    use omg_lib::core::security::validation::validate_relative_path;

    assert!(validate_relative_path("foo/bar").is_ok());
    assert!(validate_relative_path("foo/../bar").is_err());
    assert!(validate_relative_path("/absolute").is_err());
    assert!(validate_relative_path("..").is_err());
    assert!(validate_relative_path(".").is_ok());
}
