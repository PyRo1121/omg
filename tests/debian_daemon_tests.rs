#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[test]
fn test_daemon_initialization_debian_mock() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();

    // Set test mode to trigger mock paths
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "true");
        // Also set mock distro to debian to ensure correct package manager selection
        std::env::set_var("OMG_TEST_DISTRO", "debian");
        std::env::set_var("OMG_DAEMON_DATA_DIR", &temp_path);
    }

    // We expect this to succeed if integration is correct, 
    // but it currently fails because debian_db doesn't handle test mode in get_detailed_packages
    // or PackageIndex::new_apt doesn't handle the error gracefully.
    
    use omg_lib::daemon::handlers::DaemonState;
    
    // Initialize daemon state
    let state_result = DaemonState::new();
    
    // Assert success
    assert!(state_result.is_ok(), "DaemonState::new() failed: {:?}", state_result.err());
    
    let state = state_result.unwrap();
    
    // Check if index is populated (mock data should be present)
    // The mock data in debian_db::search_fast returns 1 package ("apt")
    // But PackageIndex::new_apt builds from get_detailed_packages.
    // We expect get_detailed_packages to return mock data in test mode.
    assert!(!state.index.is_empty(), "Package index should not be empty in mock mode");
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[tokio::test]
async fn test_handle_debian_search() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();
    
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "true");
        std::env::set_var("OMG_TEST_DISTRO", "debian");
        std::env::set_var("OMG_DAEMON_DATA_DIR", &temp_path);
    }

    use omg_lib::daemon::handlers::{DaemonState, handle_request};
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
    use std::sync::Arc;

    let state = Arc::new(DaemonState::new().unwrap());
    
    let req = Request::DebianSearch {
        id: 123,
        query: "apt".to_string(),
        limit: Some(10),
    };

    let response = handle_request(state, req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 123);
            if let ResponseResult::DebianSearch(pkgs) = result {
                assert!(!pkgs.is_empty());
                assert_eq!(pkgs[0], "apt");
            } else {
                panic!("Expected DebianSearch result, got {:?}", result);
            }
        }
        Response::Error { message, .. } => panic!("Search failed: {}", message),
    }
}
