#![expect(clippy::unwrap_used)]
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_fuzzy_suggestions() {
    // Setup
    let temp_dir = TempDir::new().unwrap();

    // Initialize with scoped env: the daemon and audit logger capture their
    // data-dir paths during construction, so isolation holds after the guard
    // restores the process environment.
    //
    // Note: This relies on the actual package manager backend or cache.
    // PackageIndex is read-only from the backend, so we initialize and see if
    // we get any suggestions for a common package. If the environment has no
    // packages (e.g. CI without apt/pacman setup), this might be empty.
    let data_dir = temp_dir.path().to_path_buf();
    let state = temp_env::with_vars(
        [
            ("OMG_DAEMON_DATA_DIR", Some(data_dir.as_os_str())),
            ("OMG_DATA_DIR", Some(data_dir.as_os_str())),
        ],
        || {
            let _ = omg_lib::core::security::init_audit_logger();
            match DaemonState::new() {
                Ok(s) => Some(Arc::new(s)),
                Err(_) => None,
            }
        },
    );
    let Some(state) = state else {
        println!("Skipping test: Could not initialize DaemonState (no package manager?)");
        return;
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
        limit: Some(5),
    };

    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success {
            result: ResponseResult::Suggest(suggestions),
            ..
        } => {
            assert!(
                !suggestions.is_empty(),
                "Should return suggestions for '{typo}'"
            );
            assert!(
                suggestions.contains(target_pkg),
                "Suggestions for '{typo}' should contain '{target_pkg}'"
            );
        }
        _ => unreachable!("Expected Suggest response"),
    }
}
