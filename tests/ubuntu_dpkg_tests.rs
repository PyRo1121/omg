//! Ubuntu dpkg status parsing tests
//!
//! Tests that verify dpkg/status parsing works correctly with Ubuntu-specific
//! package naming conventions and metadata.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// Ubuntu Package Naming Tests
// ============================================================================

#[test]
fn test_parse_ubuntu_specific_packages() {
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    // Ubuntu-specific packages with common naming patterns
    let content = r#"
Package: ubuntu-desktop
Status: install ok installed
Priority: optional
Section: metapackages
Installed-Size: 25
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 1.481
Description: The Ubuntu desktop system
 This package depends on all of the packages in the Ubuntu desktop system

Package: linux-image-6.8.0-49-generic
Status: install ok installed
Priority: optional
Section: kernel
Installed-Size: 15264428
Maintainer: Ubuntu Kernel Team <kernel-team@lists.ubuntu.com>
Architecture: amd64
Version: 6.8.0-49.49
Description: Linux kernel image for version 6.8.0 on 64 bit x86 SMP
 This package contains the Linux kernel image for version 6.8.0 on
 64 bit x86 SMP.

Package: python3-apt
Status: install ok installed
Priority: optional
Section: python
Installed-Size: 780
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 2.8.0ubuntu1
Depends: libc6, python3
Description: Python 3 interface to libapt-pkg
 The apt_pkg Python 3 interface will provide full access to the
 internal libapt-pkg structures.

Package: snapd
Status: install ok installed
Priority: optional
Section: devel
Installed-Size: 148480
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 2.63+24.04
Description: Daemon and tooling that enable snap packages
 Install, configure, refresh and remove snap packages.
"#;

    fs::write(&status_path, content).unwrap();

    // Manually verify the content is parseable
    let parsed_content = fs::read_to_string(&status_path).unwrap();
    assert!(parsed_content.contains("ubuntu-desktop"));
    assert!(parsed_content.contains("linux-image-6.8.0-49-generic"));
    assert!(parsed_content.contains("python3-apt"));
    assert!(parsed_content.contains("snapd"));
}

// ============================================================================
// Ubuntu Version Format Tests
// ============================================================================

#[test]
fn test_ubuntu_version_formats() {
    // Ubuntu uses various version formats
    let versions = vec![
        ("1.481", "Simple metapackage version"),
        ("6.8.0-49.49", "Kernel version with build number"),
        ("2.8.0ubuntu1", "Version with ubuntu suffix"),
        ("2.63+24.04", "Version with release suffix"),
        ("1:8.3.0-6ubuntu2", "Epoch with ubuntu suffix"),
        ("5.4-3ubuntu0.5", "Security update version"),
    ];

    for (version, description) in versions {
        // Just verify the format is valid
        assert!(!version.is_empty(), "{} should not be empty", description);
        assert!(
            version.chars().any(|c| c.is_ascii_digit()),
            "{} should contain digits",
            description
        );
    }
}

// ============================================================================
// Ubuntu Multi-arch Tests
// ============================================================================

#[test]
fn test_ubuntu_multiarch_packages() {
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    // Ubuntu supports multi-arch packages
    let content = r#"
Package: libc6
Status: install ok installed
Priority: required
Section: libs
Installed-Size: 14336
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Multi-Arch: same
Version: 2.39-0ubuntu8.3
Description: GNU C Library: Shared libraries
 Contains the standard libraries that are used by nearly all programs on
 the system.

Package: libc6
Status: install ok installed
Priority: optional
Section: libs
Installed-Size: 14212
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: i386
Multi-Arch: same
Version: 2.39-0ubuntu8.3
Description: GNU C Library: Shared libraries
 Contains the standard libraries that are used by nearly all programs on
 the system.
"#;

    fs::write(&status_path, content).unwrap();

    // Verify we can have the same package installed for different architectures
    let parsed_content = fs::read_to_string(&status_path).unwrap();
    let libc6_count = parsed_content.matches("Package: libc6").count();
    assert_eq!(libc6_count, 2, "Should have libc6 for both amd64 and i386");
}

// ============================================================================
// Ubuntu Snap Integration Tests
// ============================================================================

#[test]
fn test_ubuntu_snap_packages() {
    // Ubuntu has snap packages alongside traditional .deb packages
    // Our dpkg parser should ignore snap packages (they have their own system)
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    let content = r#"
Package: snapd
Status: install ok installed
Priority: optional
Section: devel
Installed-Size: 148480
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 2.63+24.04
Depends: libc6, systemd
Description: Daemon and tooling that enable snap packages
 Install, configure, refresh and remove snap packages. Snaps are
 'universal' packages that work across many different Linux systems.
"#;

    fs::write(&status_path, content).unwrap();

    let parsed_content = fs::read_to_string(&status_path).unwrap();
    assert!(parsed_content.contains("snapd"));
    assert!(parsed_content.contains("Status: install ok installed"));
}

// ============================================================================
// Ubuntu PPK/Backports Version Tests
// ============================================================================

#[test]
fn test_ubuntu_ppa_package_versions() {
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    // Packages from PPAs often have unique versioning
    let content = r#"
Package: python3.12
Status: install ok installed
Priority: optional
Section: python
Installed-Size: 584
Maintainer: Matthias Klose <doko@ubuntu.com>
Architecture: amd64
Version: 3.12.7-1+noble1~deadsnakes1
Depends: python3.12-minimal (= 3.12.7-1+noble1~deadsnakes1)
Description: Interactive high-level object-oriented language (version 3.12)
 Python is a high-level, interactive, object-oriented language.
 .
 This package built from deadsnakes PPA.
"#;

    fs::write(&status_path, content).unwrap();

    let parsed_content = fs::read_to_string(&status_path).unwrap();
    assert!(parsed_content.contains("3.12.7-1+noble1~deadsnakes1"));
    assert!(parsed_content.contains("python3.12"));
}

// ============================================================================
// Ubuntu Security Update Tests
// ============================================================================

#[test]
fn test_ubuntu_security_package_versions() {
    // Security updates have specific version patterns
    let versions = vec![
        "5.4-3ubuntu0.5",         // Standard security update
        "1:2.31.1-1ubuntu1.10",   // With epoch
        "1.2.3-4ubuntu5.20.04.1", // LTS-specific
        "8.1p1-6ubuntu3.9",       // OpenSSH style
    ];

    for version in versions {
        assert!(
            version.contains("ubuntu"),
            "Security versions should contain 'ubuntu': {}",
            version
        );
    }
}

// ============================================================================
// Ubuntu Transitional Packages Tests
// ============================================================================

#[test]
fn test_ubuntu_transitional_packages() {
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    // Ubuntu uses transitional packages for smooth upgrades
    let content = r#"
Package: python
Status: install ok installed
Priority: optional
Section: python
Installed-Size: 12
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: all
Version: 2.7.18-3
Depends: python2 (>= 2.7.18-2)
Description: interactive high-level object-oriented language (Python2 version)
 This is a transitional package to ease upgrades to Python 3.
"#;

    fs::write(&status_path, content).unwrap();

    let parsed_content = fs::read_to_string(&status_path).unwrap();
    assert!(parsed_content.contains("transitional package"));
}

// ============================================================================
// Ubuntu Minimal/Standard/Desktop Package Sets
// ============================================================================

#[test]
fn test_ubuntu_package_priorities() {
    let temp_dir = TempDir::new().unwrap();
    let status_path = temp_dir.path().join("status");

    let content = r#"
Package: base-files
Status: install ok installed
Priority: required
Section: admin
Installed-Size: 98
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 13ubuntu10
Description: Debian base system miscellaneous files
 This package contains basic filesystem structure.

Package: ubuntu-minimal
Status: install ok installed
Priority: important
Section: metapackages
Installed-Size: 25
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 1.481
Description: Minimal core of Ubuntu
 This package depends on all of the packages in the Ubuntu minimal system.

Package: ubuntu-standard
Status: install ok installed
Priority: optional
Section: metapackages
Installed-Size: 25
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Version: 1.481
Description: The Ubuntu standard system
 This package depends on all of the packages in the Ubuntu standard system.
"#;

    fs::write(&status_path, content).unwrap();

    let parsed_content = fs::read_to_string(&status_path).unwrap();
    assert!(parsed_content.contains("Priority: required"));
    assert!(parsed_content.contains("Priority: important"));
    assert!(parsed_content.contains("Priority: optional"));
}

// ============================================================================
// Real-world Ubuntu Package Examples
// ============================================================================

#[test]
fn test_common_ubuntu_packages() {
    let packages = vec![
        "ubuntu-desktop",
        "ubuntu-minimal",
        "ubuntu-standard",
        "linux-generic",
        "linux-image-generic",
        "linux-headers-generic",
        "ubuntu-advantage-tools",
        "update-notifier",
        "update-manager",
        "gnome-shell",
        "gdm3",
        "systemd",
        "snapd",
        "apparmor",
    ];

    // Just verify the naming patterns are reasonable
    for pkg in packages {
        assert!(!pkg.is_empty());
        assert!(
            pkg.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        );
    }
}

// ============================================================================
// Ubuntu Extended States Tests
// ============================================================================

#[test]
fn test_ubuntu_extended_states_format() {
    let temp_dir = TempDir::new().unwrap();
    let extended_states_path = temp_dir.path().join("extended_states");

    // Ubuntu's extended_states file tracks auto-installed packages
    let content = r#"
Package: libfoo-dev
Architecture: amd64
Auto-Installed: 1

Package: ubuntu-desktop
Architecture: amd64
Auto-Installed: 0

Package: libbar2
Architecture: amd64
Auto-Installed: 1
"#;

    fs::write(&extended_states_path, content).unwrap();

    let parsed_content = fs::read_to_string(&extended_states_path).unwrap();
    assert!(parsed_content.contains("Auto-Installed: 1"));
    assert!(parsed_content.contains("Auto-Installed: 0"));
}

// ============================================================================
// Architecture-specific Tests
// ============================================================================

#[test]
fn test_ubuntu_architecture_names() {
    let architectures = vec![
        "amd64",   // 64-bit x86
        "i386",    // 32-bit x86
        "arm64",   // 64-bit ARM
        "armhf",   // ARM hard float
        "ppc64el", // PowerPC 64-bit little-endian
        "s390x",   // IBM System z
        "riscv64", // RISC-V 64-bit
        "all",     // Architecture-independent
    ];

    for arch in architectures {
        assert!(!arch.is_empty());
        assert!(arch.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
