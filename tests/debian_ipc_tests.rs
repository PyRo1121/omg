use omg_lib::daemon::protocol::{PackageInfo, Request, ResponseResult};

#[test]
fn test_debian_search_request_serialization() {
    // This variant does not exist yet, so this test will fail to compile (Red phase)
    let req = Request::DebianSearch {
        id: 1,
        query: "vim".to_string(),
        limit: Some(10),
    };

    let serialized = serde_json::to_string(&req).unwrap();
    // Verify it serializes correctly with the variant name
    assert!(serialized.contains("DebianSearch"));
    assert!(serialized.contains("vim"));
    assert!(serialized.contains('1'));

    // Test round-trip deserialization
    let deserialized: Request = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Request::DebianSearch { id, query, limit } => {
            assert_eq!(id, 1);
            assert_eq!(query, "vim");
            assert_eq!(limit, Some(10));
        }
        _ => panic!("Wrong variant deserialized"),
    }
}

#[test]
fn test_debian_search_result_serialization() {
    // This variant does not exist yet either
    let result = ResponseResult::DebianSearch(vec![
        PackageInfo {
            name: "vim".to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: "apt".to_string(),
        },
        PackageInfo {
            name: "nano".to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: "apt".to_string(),
        },
    ]);

    let serialized = serde_json::to_string(&result).unwrap();
    assert!(serialized.contains("DebianSearch"));
    assert!(serialized.contains("vim"));

    let deserialized: ResponseResult = serde_json::from_str(&serialized).unwrap();
    if let ResponseResult::DebianSearch(pkgs) = deserialized {
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "vim");
    } else {
        panic!("Wrong variant deserialized");
    }
}
