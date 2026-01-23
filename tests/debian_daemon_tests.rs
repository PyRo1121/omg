#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[test]
fn test_daemon_initialization_debian_mock() {
    // Set test mode to trigger mock paths
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "true");
        // Also set mock distro to debian to ensure correct package manager selection
        std::env::set_var("OMG_TEST_DISTRO", "debian");
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
