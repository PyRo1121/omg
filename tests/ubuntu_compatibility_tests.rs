//! Ubuntu-specific compatibility tests
//!
//! Tests that verify our Debian implementation works correctly on Ubuntu
//! including PPAs, Ubuntu-specific release names, and archive structure.
//!
//! Run with: `cargo test --features debian-pure --test ubuntu_compatibility_tests`

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::path::Path;

use omg_lib::package_managers::debian_db::{
    RepoType, Repository, parse_deb822_content, parse_sources_list_content,
};

// ============================================================================
// Ubuntu PPA Support Tests
// ============================================================================

#[test]
fn test_parse_ubuntu_ppa_format() {
    // Ubuntu PPAs use a specific sources.list format
    let content = r"
# Traditional PPA format (converted by add-apt-repository)
deb http://ppa.launchpad.net/deadsnakes/ppa/ubuntu noble main
deb-src http://ppa.launchpad.net/deadsnakes/ppa/ubuntu noble main
";

    let repos = parse_sources_list_content(
        content,
        Path::new("/etc/apt/sources.list.d/deadsnakes-ubuntu-ppa-noble.list"),
    )
    .unwrap();

    assert_eq!(repos.len(), 2, "Should parse PPA binary and source repos");
    assert_eq!(
        repos[0].uri,
        "http://ppa.launchpad.net/deadsnakes/ppa/ubuntu"
    );
    assert_eq!(repos[0].suite, "noble");
    assert_eq!(repos[0].components, vec!["main"]);
    assert!(repos[0].enabled);
}

#[test]
fn test_parse_ubuntu_ppa_with_signed_by() {
    // Modern PPAs with GPG key
    let content = r"
deb [signed-by=/etc/apt/keyrings/deadsnakes-ubuntu-ppa.gpg] https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu noble main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1);
    assert_eq!(
        repos[0].uri,
        "https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu"
    );
    assert!(repos[0].signed_by.is_some());
}

// ============================================================================
// Ubuntu Release Name Tests
// ============================================================================

#[test]
fn test_ubuntu_release_names() {
    // Test all modern Ubuntu LTS releases
    let releases = vec![
        "noble",  // 24.04 LTS
        "jammy",  // 22.04 LTS
        "focal",  // 20.04 LTS
        "bionic", // 18.04 LTS (EOL but still in use)
    ];

    for release in releases {
        let content = format!(
            "
deb http://archive.ubuntu.com/ubuntu {release} main restricted universe multiverse
deb http://archive.ubuntu.com/ubuntu {release}-updates main restricted universe multiverse
deb http://archive.ubuntu.com/ubuntu {release}-security main restricted universe multiverse
"
        );

        let repos = parse_sources_list_content(&content, Path::new("/test")).unwrap();

        assert_eq!(
            repos.len(),
            3,
            "Should parse all Ubuntu repos for {release}"
        );
        assert_eq!(repos[0].suite, release);
        assert_eq!(repos[1].suite, format!("{release}-updates"));
        assert_eq!(repos[2].suite, format!("{release}-security"));
    }
}

#[test]
fn test_ubuntu_components() {
    // Ubuntu has 4 main components vs Debian's 3
    let content = r"
deb http://archive.ubuntu.com/ubuntu noble main restricted universe multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1);
    assert_eq!(
        repos[0].components,
        vec!["main", "restricted", "universe", "multiverse"],
        "Ubuntu has 4 component sections"
    );
}

// ============================================================================
// Ubuntu Archive Structure Tests
// ============================================================================

#[test]
fn test_ubuntu_packages_url_generation() {
    let repo = Repository {
        repo_type: RepoType::Binary,
        uri: "http://archive.ubuntu.com/ubuntu".to_string(),
        suite: "noble".to_string(),
        components: vec!["main".to_string()],
        arch: None,
        signed_by: None,
        enabled: true,
        source_file: Path::new("/test").to_path_buf(),
        options: std::collections::HashMap::new(),
    };

    let url = repo.packages_url("amd64");
    assert_eq!(
        url, "http://archive.ubuntu.com/ubuntu/dists/noble/main/binary-amd64/Packages",
        "Should generate correct Ubuntu Packages URL"
    );
}

#[test]
fn test_ubuntu_release_url() {
    let repo = Repository {
        repo_type: RepoType::Binary,
        uri: "http://archive.ubuntu.com/ubuntu".to_string(),
        suite: "noble-updates".to_string(),
        components: vec!["main".to_string()],
        arch: None,
        signed_by: None,
        enabled: true,
        source_file: Path::new("/test").to_path_buf(),
        options: std::collections::HashMap::new(),
    };

    let url = repo.release_url();
    assert_eq!(
        url, "http://archive.ubuntu.com/ubuntu/dists/noble-updates/InRelease",
        "Should generate correct Ubuntu Release URL"
    );
}

// ============================================================================
// Ubuntu Mirror Tests
// ============================================================================

#[test]
fn test_ubuntu_mirrors() {
    let mirrors = vec![
        "http://archive.ubuntu.com/ubuntu",
        "http://us.archive.ubuntu.com/ubuntu",
        "http://mirror.math.princeton.edu/pub/ubuntu",
        "http://mirrors.kernel.org/ubuntu",
    ];

    for mirror in mirrors {
        let content = format!(
            "
deb {mirror} noble main
"
        );

        let repos = parse_sources_list_content(&content, Path::new("/test")).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].uri, mirror);
    }
}

// ============================================================================
// Ubuntu Backports Tests
// ============================================================================

#[test]
fn test_ubuntu_backports() {
    let content = r"
deb http://archive.ubuntu.com/ubuntu noble-backports main restricted universe multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].suite, "noble-backports");
    assert_eq!(repos[0].components.len(), 4);
}

// ============================================================================
// Ubuntu Proposed Tests (for testing newer packages)
// ============================================================================

#[test]
fn test_ubuntu_proposed_repo() {
    let content = r"
# Uncommented for testing
deb http://archive.ubuntu.com/ubuntu noble-proposed main restricted universe multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].suite, "noble-proposed");
}

// ============================================================================
// deb822 Format Tests (Ubuntu 24.04+)
// ============================================================================

#[test]
fn test_ubuntu_deb822_format() {
    // Ubuntu 24.04 (Noble) introduced deb822 format by default
    let content = r"
Types: deb
URIs: http://archive.ubuntu.com/ubuntu
Suites: noble noble-updates noble-security
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
";

    let repos =
        parse_deb822_content(content, Path::new("/etc/apt/sources.list.d/ubuntu.sources")).unwrap();

    // 1 type * 1 URI * 3 suites = 3 repos
    assert_eq!(repos.len(), 3);
    assert_eq!(repos[0].suite, "noble");
    assert_eq!(repos[1].suite, "noble-updates");
    assert_eq!(repos[2].suite, "noble-security");

    // All should have the same components
    for repo in &repos {
        assert_eq!(
            repo.components,
            vec!["main", "restricted", "universe", "multiverse"]
        );
    }
}

#[test]
fn test_ubuntu_ppa_deb822() {
    // PPAs can also use deb822 format
    let content = r"
Types: deb
URIs: https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu
Suites: noble
Components: main
Signed-By: /etc/apt/keyrings/deadsnakes-ubuntu-ppa.gpg
";

    let repos = parse_deb822_content(
        content,
        Path::new("/etc/apt/sources.list.d/deadsnakes.sources"),
    )
    .unwrap();

    assert_eq!(repos.len(), 1);
    assert_eq!(
        repos[0].uri,
        "https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu"
    );
    assert!(repos[0].signed_by.is_some());
}

// ============================================================================
// Multi-architecture Tests
// ============================================================================

#[test]
fn test_ubuntu_multiarch() {
    // Ubuntu supports multiple architectures
    let content = r"
deb [arch=amd64] http://archive.ubuntu.com/ubuntu noble main
deb [arch=i386] http://archive.ubuntu.com/ubuntu noble main
deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports noble main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 3);
    assert_eq!(repos[0].arch, Some("amd64".to_string()));
    assert_eq!(repos[1].arch, Some("i386".to_string()));
    assert_eq!(repos[2].arch, Some("arm64".to_string()));
}

// ============================================================================
// Ubuntu Ports Tests (ARM/PowerPC)
// ============================================================================

#[test]
fn test_ubuntu_ports() {
    let content = r"
deb http://ports.ubuntu.com/ubuntu-ports noble main restricted universe multiverse
deb http://ports.ubuntu.com/ubuntu-ports noble-updates main restricted universe multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].uri, "http://ports.ubuntu.com/ubuntu-ports");
}

// ============================================================================
// Edge Cases and Malformed Input Tests
// ============================================================================

#[test]
fn test_mixed_debian_ubuntu_sources() {
    // Some users might mix Debian and Ubuntu sources (not recommended but we should handle it)
    let content = r"
deb http://archive.ubuntu.com/ubuntu noble main
deb http://deb.debian.org/debian bookworm main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].suite, "noble");
    assert_eq!(repos[1].suite, "bookworm");
}

#[test]
fn test_ubuntu_sources_with_trailing_slash() {
    let content = r"
deb http://archive.ubuntu.com/ubuntu/ noble main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1);
    // URL generation should handle trailing slashes correctly
    let url = repos[0].packages_url("amd64");
    assert!(!url.contains("//dists"), "Should not have double slashes");
}

#[test]
fn test_ubuntu_sources_with_comments() {
    let content = r"
# Noble main repositories
deb http://archive.ubuntu.com/ubuntu noble main restricted

# Universe and multiverse
deb http://archive.ubuntu.com/ubuntu noble universe multiverse

# Updates
deb http://archive.ubuntu.com/ubuntu noble-updates main restricted universe multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 3);
}

// ============================================================================
// Real-world Ubuntu Configuration Tests
// ============================================================================

#[test]
fn test_default_ubuntu_2404_sources() {
    // Typical Ubuntu 24.04 (Noble) default configuration
    let content = r"
Types: deb
URIs: http://archive.ubuntu.com/ubuntu
Suites: noble noble-updates noble-backports
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

Types: deb
URIs: http://security.ubuntu.com/ubuntu
Suites: noble-security
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
";

    let repos =
        parse_deb822_content(content, Path::new("/etc/apt/sources.list.d/ubuntu.sources")).unwrap();

    // First stanza: 1 type * 1 URI * 3 suites = 3 repos
    // Second stanza: 1 type * 1 URI * 1 suite = 1 repo
    // Total: 4 repos
    assert_eq!(repos.len(), 4);

    // Verify suites
    assert_eq!(repos[0].suite, "noble");
    assert_eq!(repos[1].suite, "noble-updates");
    assert_eq!(repos[2].suite, "noble-backports");
    assert_eq!(repos[3].suite, "noble-security");

    // Verify security repo uses different URI
    assert_eq!(repos[3].uri, "http://security.ubuntu.com/ubuntu");
}

#[test]
fn test_default_ubuntu_2204_sources() {
    // Ubuntu 22.04 (Jammy) still uses legacy format by default
    let content = r"
deb http://archive.ubuntu.com/ubuntu jammy main restricted
deb http://archive.ubuntu.com/ubuntu jammy-updates main restricted
deb http://archive.ubuntu.com/ubuntu jammy universe
deb http://archive.ubuntu.com/ubuntu jammy-updates universe
deb http://archive.ubuntu.com/ubuntu jammy multiverse
deb http://archive.ubuntu.com/ubuntu jammy-updates multiverse
deb http://archive.ubuntu.com/ubuntu jammy-backports main restricted universe multiverse
deb http://security.ubuntu.com/ubuntu jammy-security main restricted
deb http://security.ubuntu.com/ubuntu jammy-security universe
deb http://security.ubuntu.com/ubuntu jammy-security multiverse
";

    let repos = parse_sources_list_content(content, Path::new("/etc/apt/sources.list")).unwrap();

    assert_eq!(repos.len(), 10);
}

// ============================================================================
// Package URL Generation Tests for Ubuntu
// ============================================================================

#[test]
fn test_ubuntu_package_url_all_components() {
    let components = vec!["main", "restricted", "universe", "multiverse"];

    for component in components {
        let repo = Repository {
            repo_type: RepoType::Binary,
            uri: "http://archive.ubuntu.com/ubuntu".to_string(),
            suite: "noble".to_string(),
            components: vec![component.to_string()],
            arch: None,
            signed_by: None,
            enabled: true,
            source_file: Path::new("/test").to_path_buf(),
            options: std::collections::HashMap::new(),
        };

        let url = repo.packages_url("amd64");
        assert_eq!(
            url,
            format!(
                "http://archive.ubuntu.com/ubuntu/dists/noble/{component}/binary-amd64/Packages"
            )
        );
    }
}

#[test]
fn test_ppa_package_url() {
    let repo = Repository {
        repo_type: RepoType::Binary,
        uri: "https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu".to_string(),
        suite: "noble".to_string(),
        components: vec!["main".to_string()],
        arch: None,
        signed_by: None,
        enabled: true,
        source_file: Path::new("/test").to_path_buf(),
        options: std::collections::HashMap::new(),
    };

    let url = repo.packages_url("amd64");
    assert_eq!(
        url,
        "https://ppa.launchpadcontent.net/deadsnakes/ppa/ubuntu/dists/noble/main/binary-amd64/Packages"
    );
}
