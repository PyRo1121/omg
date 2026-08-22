#![expect(clippy::unwrap_used)]
pub mod common;

use common::report_skip;
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
        report_skip("could not initialize DaemonState (no package manager backend)");
        return;
    };

    // Probe the index through the public IPC seam (the `index` field is no
    // longer visible outside the daemon subtree): an empty result set for a
    // package that every seeded backend provides means there is nothing to
    // derive suggestions from.
    let probe = handle_request(
        Arc::clone(&state),
        Request::Search {
            id: 2,
            query: "git".to_string(),
            limit: Some(10),
        },
    )
    .await;
    let target_pkg = match probe {
        Response::Success {
            result: ResponseResult::Search(results),
            ..
        } if !results.packages.is_empty() => results.packages[0].name.clone(),
        _ => {
            report_skip("package index has no entries to derive a typo from");
            return;
        }
    };

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
            // `suggest` is prefix-based: every suggestion must extend the
            // queried prefix (pinned as a contract, not against a specific
            // package name, since which entries fill `limit` depends on the
            // host index).
            for suggestion in &suggestions {
                assert!(
                    suggestion.starts_with(typo.as_str()),
                    "suggestion '{suggestion}' must extend the query prefix '{typo}'"
                );
            }
        }
        _ => unreachable!("Expected Suggest response"),
    }
}
