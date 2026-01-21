use std::sync::Arc;
use tempfile::TempDir;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_fuzzy_suggestions() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    // Initialize
    let _ = omg_lib::core::security::init_audit_logger();

    // We need to initialize DaemonState.
    // Note: This relies on the actual package manager backend or cache.
    // Since we can't easily mock the backend in this integration test without
    // more extensive refactoring, we'll try to rely on what's available or
    // check if we can populate the index manually.
    //
    // However, PackageIndex is read-only from the backend.
    //
    // A better approach for this unit test is to test the index logic directly
    // if we can create a mock index, but PackageIndex creation is coupled to backends.
    //
    // Let's try to initialize and see if we get any suggestions for a common package.
    // If the environment has no packages (e.g. CI without apt/pacman setup), this might be empty.

    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => {
            println!("Skipping test: Could not initialize DaemonState (no package manager?)");
            return;
        }
    };

    // If the index is empty, we can't test much.
    if state.index.is_empty() {
        println!("Skipping test: Package index is empty");
        return;
    }

    // Pick a package that likely exists (e.g. "coreutils" or "bash" or "sudo")
    // We'll try to find a real package name from the index first
    let all_pkgs = state.index.all_packages();
    if all_pkgs.is_empty() {
         println!("Skipping test: No packages in index");
         return;
    }

    let target_pkg = &all_pkgs[0].name;
    // Create a typo: remove last char
    let mut typo = target_pkg.clone();
    typo.pop();

    // Send Suggest request
    let req = Request::Suggest {
        id: 1,
        query: typo.clone(),
        limit: Some(5)
    };

    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { result: ResponseResult::Suggest(suggestions), .. } => {
            assert!(!suggestions.is_empty(), "Should return suggestions for '{}'", typo);
            assert!(suggestions.contains(target_pkg), "Suggestions for '{}' should contain '{}'", typo, target_pkg);
        },
        _ => panic!("Expected Suggest response"),
    }
}
