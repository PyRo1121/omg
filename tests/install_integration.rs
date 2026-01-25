#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Integration tests for omg install command
//!
//! Tests both official and AUR package installation on Arch Linux

#[cfg(feature = "arch")]
mod arch_install_tests {
    use std::process::Command;

    /// Helper function to run omg command
    #[allow(dead_code)]
    fn run_omg(args: &[&str]) -> std::process::Output {
        Command::new("cargo")
            .args(["run", "--release", "--", args[0], args[1]])
            .output()
            .expect("Failed to execute omg command")
    }

    /// Check if a package is installed
    fn is_package_installed(pkg: &str) -> bool {
        Command::new("pacman")
            .args(["-Q", pkg])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Install a package and return success status
    fn install_package(pkg: &str) -> bool {
        let output = Command::new("sudo")
            .args([
                "-E",
                "cargo",
                "run",
                "--release",
                "--",
                "install",
                "-y",
                pkg,
            ])
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(e) => {
                eprintln!("Install command failed: {}", e);
                false
            }
        }
    }

    /// Remove a package
    fn remove_package(pkg: &str) -> bool {
        Command::new("sudo")
            .args(["pacman", "-Rdd", "--noconfirm", pkg])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    #[test]
    #[ignore] // System test - run with OMG_RUN_SYSTEM_TESTS=1
    fn test_install_official_package() {
        // Use a small, safe package for testing
        let test_pkg = "ripgrep";

        // Skip if already installed
        if is_package_installed(test_pkg) {
            println!("Package {} already installed, skipping test", test_pkg);
            return;
        }

        // Try to install
        let success = install_package(test_pkg);

        // Cleanup
        if is_package_installed(test_pkg) {
            remove_package(test_pkg);
        }

        assert!(success, "Failed to install official package {}", test_pkg);
    }

    #[test]
    #[ignore] // System test - run with OMG_RUN_SYSTEM_TESTS=1
    fn test_install_aur_package() {
        // Test with a small AUR package
        let test_pkg = "helium-browser-bin";

        // Skip if already installed
        if is_package_installed(test_pkg) {
            println!("Package {} already installed, skipping test", test_pkg);
            return;
        }

        // Try to install from AUR
        let success = install_package(test_pkg);

        // Cleanup
        if is_package_installed(test_pkg) {
            remove_package(test_pkg);
        }

        assert!(success, "Failed to install AUR package {}", test_pkg);
    }

    #[test]
    #[ignore] // System test - run with OMG_RUN_SYSTEM_TESTS=1
    fn test_install_mixed_packages() {
        // Test installing both official and AUR packages in one command
        let official_pkg = "bat";
        let aur_pkg = "helium-browser-bin";

        // Skip if already installed
        let official_installed = is_package_installed(official_pkg);
        let aur_installed = is_package_installed(aur_pkg);

        if official_installed && aur_installed {
            println!("Packages already installed, skipping test");
            return;
        }

        // Try to install both
        let output = Command::new("sudo")
            .args([
                "-E",
                "cargo",
                "run",
                "--release",
                "--",
                "install",
                "-y",
                official_pkg,
                aur_pkg,
            ])
            .output();

        let success = output.map(|out| out.status.success()).unwrap_or(false);

        // Cleanup
        if is_package_installed(official_pkg) && !official_installed {
            remove_package(official_pkg);
        }
        if is_package_installed(aur_pkg) && !aur_installed {
            remove_package(aur_pkg);
        }

        assert!(success, "Failed to install mixed packages");
    }

    #[test]
    #[ignore] // System test - run with OMG_RUN_SYSTEM_TESTS=1
    fn test_install_nonexistent_package() {
        let fake_pkg = "this-package-does-not-exist-12345";

        let output = Command::new("cargo")
            .args(["run", "--release", "--", "install", fake_pkg])
            .output()
            .expect("Failed to execute omg command");

        // Should fail
        assert!(!output.status.success());

        // Should contain error message
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let output = format!("{}{}", stdout, stderr);

        assert!(
            output.contains("not found") || output.contains("Package not found"),
            "Expected 'not found' in output, got: {}",
            output
        );
    }
}
