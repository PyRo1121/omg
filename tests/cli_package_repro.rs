//! CLI package module reproduction tests
//!
//! These tests verify that CLI package functions compile and run correctly.
//! Requires arch or debian feature to have a working package manager.

#![cfg(any(feature = "arch", feature = "debian"))]

use omg_lib::cli::packages;

#[tokio::test]
async fn test_search_compilation() {
    let _ = packages::search("query", false, false, false).await;
}

#[tokio::test]
async fn test_install_compilation() {
    let _ = packages::install(&["package".to_string()], true, false).await;
}
