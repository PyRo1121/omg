use assert_cmd::Command;
use predicates::prelude::*;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[test]
fn test_omg_search_debian_daemon_routing() {
    // This test simulates a Debian environment and checks if search works
    // Since we can't easily spawn a full daemon in this test context without
    // complex setup, we primarily verifying the CLI doesn't crash and returns
    // "Fetched via daemon" if it managed to hit the daemon (which it won't here,
    // so it will fall back, but the code path is exercised).
    // To truly test the daemon routing, we'd need a running omgd instance.
    // However, we can check if it attempts to connect.
    
    // For now, we'll verify basic execution with the feature flag.
    let mut cmd = Command::cargo_bin("omg").unwrap();
    cmd.env("OMG_TEST_DISTRO", "debian")
       .env("OMG_TEST_MODE", "true") // Triggers test mode which avoids real socket connection for now
       .arg("search")
       .arg("vim")
       .assert()
       .success();
}
