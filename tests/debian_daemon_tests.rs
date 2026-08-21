#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::daemon::handlers::DaemonState;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::sync::Arc;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[test]
fn test_daemon_initialization_debian_mock() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();

    let state_result = temp_env::with_vars(
        [
            ("OMG_TEST_MODE", Some("true")),
            ("OMG_TEST_DISTRO", Some("debian")),
            ("OMG_DAEMON_DATA_DIR", Some(temp_path.as_str())),
        ],
        DaemonState::new,
    );

    // Assert success
    assert!(
        state_result.is_ok(),
        "DaemonState::new() failed: {:?}",
        state_result.err()
    );

    let state = state_result.unwrap();

    // Check if index is populated (mock data should be present)
    // The mock data in debian_db::search_fast returns 1 package ("apt")
    // But PackageIndex::new_apt builds from get_detailed_packages.
    // We expect get_detailed_packages to return mock data in test mode.
    assert!(
        !state.index.is_empty(),
        "Package index should not be empty in mock mode"
    );
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::daemon::handlers::handle_request;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[tokio::test]
async fn test_handle_debian_search() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();

    let state = temp_env::with_vars(
        [
            ("OMG_TEST_MODE", Some("true")),
            ("OMG_TEST_DISTRO", Some("debian")),
            ("OMG_DAEMON_DATA_DIR", Some(temp_path.as_str())),
        ],
        DaemonState::new,
    )
    .unwrap();
    let state = Arc::new(state);

    let req = Request::DebianSearch {
        id: 123,
        query: "apt".to_string(),
        limit: Some(10),
    };

    let response = handle_request(state, req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 123);
            assert!(
                matches!(result, ResponseResult::DebianSearch(_)),
                "Expected DebianSearch result"
            );
            let ResponseResult::DebianSearch(pkgs) = result else {
                return;
            };
            assert!(!pkgs.is_empty());
            assert_eq!(pkgs[0].name, "apt");
        }
        Response::Error { message, .. } => {
            unreachable!("Search failed: {message}");
        }
    }
}
