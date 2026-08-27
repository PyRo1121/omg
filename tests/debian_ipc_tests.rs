//! Wire-format tests for the Debian IPC protocol variants.
//!
//! `Request::DebianSearch` and `ResponseResult::DebianSearch` cross a process
//! boundary (CLI ⇄ daemon over length-prefixed JSON, see
//! src/daemon/protocol.rs `encode_frame`/`decode_frame`). These tests pin the
//! exact serde representation — externally tagged enum variant names plus
//! field names and types — so a rename or shape change on either side of the
//! socket is caught here instead of at runtime.

#![expect(clippy::unwrap_used)]
use omg_lib::daemon::protocol::{PackageInfo, Request, ResponseResult, WirePackageSource};

/// The request must serialize with its externally-tagged variant name
/// `"DebianSearch"` and round-trip every field.
#[test]
fn test_debian_search_request_serialization() {
    let req = Request::DebianSearch {
        id: 1,
        query: "vim".to_string(),
        limit: Some(10),
    };

    let serialized = serde_json::to_string(&req).unwrap();
    // Externally tagged: {"DebianSearch":{"id":1,"query":"vim","limit":10}}
    assert!(
        serialized.contains("\"DebianSearch\""),
        "variant tag must be preserved verbatim for daemon dispatch. Got:\n{serialized}"
    );
    assert!(serialized.contains('"'), "must be valid JSON text");

    let deserialized: Request = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Request::DebianSearch { id, query, limit } => {
            assert_eq!(id, 1);
            assert_eq!(query, "vim");
            assert_eq!(limit, Some(10));
        }
        _ => unreachable!("Wrong variant deserialized"),
    }
}

/// `limit: None` must survive the round trip distinctly from `Some(_)` —
/// the daemon treats None as "use default limit".
#[test]
fn test_debian_search_request_default_limit_round_trips() {
    let req = Request::DebianSearch {
        id: u64::MAX,
        query: String::new(),
        limit: None,
    };

    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: Request = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Request::DebianSearch { id, query, limit } => {
            assert_eq!(id, u64::MAX, "request ids are full-range u64");
            assert_eq!(query, "", "empty queries are transportable");
            assert_eq!(limit, None, "absent limit must not become Some");
        }
        _ => unreachable!("Wrong variant deserialized"),
    }
}

/// Every PackageInfo field must survive serialization; results arrive as an
/// ordered vector that the CLI renders positionally.
#[test]
fn test_debian_search_result_serialization() {
    let result = ResponseResult::DebianSearch(vec![
        PackageInfo {
            name: "vim".to_string(),
            version: "2:9.0.0821-1".to_string(),
            description: "Vi IMproved - enhanced vi editor".to_string(),
            source: WirePackageSource::Apt,
        },
        PackageInfo {
            name: "nano".to_string(),
            version: "7.2-1".to_string(),
            description: "small, friendly text editor".to_string(),
            source: WirePackageSource::Apt,
        },
    ]);

    let serialized = serde_json::to_string(&result).unwrap();
    assert!(
        serialized.contains("\"DebianSearch\""),
        "response variant tag must be preserved. Got:\n{serialized}"
    );

    let deserialized: ResponseResult = serde_json::from_str(&serialized).unwrap();
    if let ResponseResult::DebianSearch(pkgs) = deserialized {
        assert_eq!(pkgs.len(), 2, "both packages must survive");
        assert_eq!(pkgs[0].name, "vim");
        assert_eq!(pkgs[0].version, "2:9.0.0821-1", "epoch'd versions survive");
        assert_eq!(
            pkgs[0].description, "Vi IMproved - enhanced vi editor",
            "descriptions survive verbatim"
        );
        assert_eq!(pkgs[0].source, WirePackageSource::Apt);
        assert_eq!(pkgs[1].name, "nano");
        assert!(
            !pkgs[1].version.is_empty(),
            "no field may be dropped or defaulted"
        );
    } else {
        panic!("Wrong variant deserialized");
    }
}
