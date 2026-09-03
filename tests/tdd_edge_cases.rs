#![allow(clippy::unwrap_used)]
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
    assert_eq!(parse_version_or_zero("1.2.3").to_string(), "1.2.3");

    // Obscure versions - without arch feature, these return as-is
    assert_eq!(parse_version_or_zero("").to_string(), "");
    assert_eq!(parse_version_or_zero("   ").to_string(), "   ");
    assert_eq!(parse_version_or_zero("v1.0").to_string(), "v1.0");
    assert_eq!(
        parse_version_or_zero("99999999999999999999999999").to_string(),
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

    // Largest request ID must survive a frame round trip intact.
    let req = Request::Ping { id: u64::MAX };
    let frame = omg_lib::daemon::protocol::encode_frame(&req).expect("encode Ping");
    let (_, payload) = omg_lib::daemon::protocol::split_frame(&frame).expect("split Ping");
    let back: Request = bitcode::deserialize(payload).expect("decode Ping");
    assert_eq!(back.id(), u64::MAX);

    // Request::Batch was removed: zero production senders and its nested
    // decode was a stack-overflow class. Ping still pins id() extraction.
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
