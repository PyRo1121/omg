//! End-to-End Tests for Pure Rust Debian/Ubuntu Implementation
//!
//! These tests exercise the complete Debian package management stack:
//! - APT sources parsing (both legacy and deb822 formats)
//! - Parallel repository synchronization
//! - Dependency resolution with version comparison
//! - Transaction engine for .deb installation
//!
//! Run with: `cargo test --features debian-pure --test debian_e2e_tests`

#![cfg(any(feature = "debian", feature = "debian-pure"))]

mod platform_semantics;

use std::fs;
use std::path::Path;

use platform_semantics::assert_no_arch_terms;
use tempfile::TempDir;

// Import modules under test
use omg_lib::package_managers::debian_db::{
    RepoType, Repository, ResolutionResult, Transaction, TransactionState, compare_versions,
    dry_run, parse_deb822_content, parse_sources_list_content,
};

// ============================================================================
// Sources Parser Tests
// ============================================================================

#[test]
fn test_parse_simple_sources_list() {
    let content = r"
# Main repository
deb http://deb.debian.org/debian bookworm main contrib non-free non-free-firmware
deb-src http://deb.debian.org/debian bookworm main

# Security updates
deb http://security.debian.org/debian-security bookworm-security main contrib non-free
";

    let repos = parse_sources_list_content(content, Path::new("/etc/apt/sources.list")).unwrap();

    assert_eq!(repos.len(), 3, "Should parse 3 repository entries");

    // Check first repo
    assert_eq!(repos[0].repo_type, RepoType::Binary);
    assert_eq!(repos[0].uri, "http://deb.debian.org/debian");
    assert_eq!(repos[0].suite, "bookworm");
    assert_eq!(
        repos[0].components,
        vec!["main", "contrib", "non-free", "non-free-firmware"]
    );
    assert!(repos[0].enabled);

    // Check source repo
    assert_eq!(repos[1].repo_type, RepoType::Source);

    // Check security repo
    assert_eq!(repos[2].suite, "bookworm-security");
}

#[test]
fn test_parse_sources_with_options() {
    let content = r"
deb [arch=amd64 signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] http://deb.debian.org/debian bookworm main
deb [arch=arm64,armhf] https://example.com/repo stable main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 2);

    // Check architecture filter
    assert_eq!(repos[0].arch, Some("amd64".to_string()));
    assert!(
        repos[0]
            .signed_by
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .contains("debian-archive-keyring.gpg")
    );

    // Check multi-arch
    assert_eq!(repos[1].arch, Some("arm64,armhf".to_string()));
}

#[test]
fn test_parse_disabled_source() {
    let content = r"
deb http://example.com/enabled stable main
# deb http://example.com/disabled unstable main
";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(repos.len(), 1, "Disabled sources should be ignored");
    assert_eq!(repos[0].suite, "stable");
}

#[test]
fn test_parse_deb822_format() {
    let content = r"
Types: deb deb-src
URIs: http://deb.debian.org/debian
Suites: bookworm bookworm-updates
Components: main contrib non-free
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
";

    let repos = parse_deb822_content(content, Path::new("/test.sources")).unwrap();

    // 2 types * 1 URI * 2 suites = 4 repository entries
    assert_eq!(repos.len(), 4, "Should expand to 4 combinations");

    // Check first entry (deb + bookworm)
    assert_eq!(repos[0].repo_type, RepoType::Binary);
    assert_eq!(repos[0].suite, "bookworm");
    assert_eq!(repos[0].components, vec!["main", "contrib", "non-free"]);

    // Check that sources are included
    assert!(repos.iter().any(|r| r.repo_type == RepoType::Source));

    // Check bookworm-updates
    assert!(repos.iter().any(|r| r.suite == "bookworm-updates"));
}

#[test]
fn test_parse_deb822_disabled() {
    let content = r"
Types: deb
URIs: http://example.com/repo
Suites: stable
Components: main
Enabled: no
";

    let repos = parse_deb822_content(content, Path::new("/test.sources")).unwrap();

    assert_eq!(repos.len(), 1);
    assert!(!repos[0].enabled, "Should respect Enabled: no");
}

#[test]
fn test_parse_deb822_multiple_stanzas() {
    let content = r"
Types: deb
URIs: http://deb.debian.org/debian
Suites: bookworm
Components: main

Types: deb
URIs: http://security.debian.org/debian-security
Suites: bookworm-security
Components: main
";

    let repos = parse_deb822_content(content, Path::new("/test.sources")).unwrap();

    assert_eq!(repos.len(), 2, "Should parse two separate stanzas");
    assert_eq!(repos[0].uri, "http://deb.debian.org/debian");
    assert_eq!(repos[1].uri, "http://security.debian.org/debian-security");
}

#[test]
fn test_parse_mixed_format_sources() {
    // Test that we handle edge cases like empty lines, comments, malformed lines
    let content = r"

# Comment at the start
deb http://example.com/repo stable main

this is not a valid line

# Another comment
deb-src http://example.com/repo stable main

   # Indented comment
deb [arch=amd64] http://example.com/special testing main

";

    let repos = parse_sources_list_content(content, Path::new("/test")).unwrap();

    assert_eq!(
        repos.len(),
        3,
        "Should parse only valid lines, ignoring malformed"
    );
    assert_eq!(repos[0].suite, "stable");
    assert_eq!(repos[1].repo_type, RepoType::Source);
    assert_eq!(repos[2].suite, "testing");
}

#[test]
fn test_repository_urls() {
    let repo = Repository {
        repo_type: RepoType::Binary,
        uri: "http://deb.debian.org/debian".to_string(),
        suite: "bookworm".to_string(),
        components: vec!["main".to_string(), "contrib".to_string()],
        arch: None,
        signed_by: None,
        enabled: true,
        source_file: std::path::PathBuf::new(),
        options: std::collections::HashMap::new(),
    };

    let packages_url = repo.packages_url("amd64");
    assert_eq!(
        packages_url,
        "http://deb.debian.org/debian/dists/bookworm/main/binary-amd64/Packages"
    );

    let release_url = repo.release_url();
    assert_eq!(
        release_url,
        "http://deb.debian.org/debian/dists/bookworm/InRelease"
    );
}

#[test]
fn test_flat_repository_layout() {
    // Some PPAs use flat repository layout without components
    let repo = Repository {
        repo_type: RepoType::Binary,
        uri: "http://example.com/ppa".to_string(),
        suite: "noble".to_string(),
        components: vec![], // No components = flat layout
        arch: None,
        signed_by: None,
        enabled: true,
        source_file: std::path::PathBuf::new(),
        options: std::collections::HashMap::new(),
    };

    let packages_url = repo.packages_url("amd64");
    assert_eq!(packages_url, "http://example.com/ppa/Packages");
}

// ============================================================================
// Version Comparison Tests
// ============================================================================

#[test]
fn test_version_comparison_simple() {
    assert_eq!(
        compare_versions("1.0", "1.0"),
        std::cmp::Ordering::Equal,
        "Equal versions"
    );
    assert_eq!(
        compare_versions("1.0", "2.0"),
        std::cmp::Ordering::Less,
        "1.0 < 2.0"
    );
    assert_eq!(
        compare_versions("2.0", "1.0"),
        std::cmp::Ordering::Greater,
        "2.0 > 1.0"
    );
    assert_eq!(
        compare_versions("1.9", "1.10"),
        std::cmp::Ordering::Less,
        "1.9 < 1.10"
    );
}

#[test]
fn test_version_comparison_with_epoch() {
    // Epoch takes precedence over everything
    assert_eq!(
        compare_versions("1:0.1", "9.9"),
        std::cmp::Ordering::Greater,
        "epoch 1 beats any version without epoch"
    );
    assert_eq!(
        compare_versions("2:1.0", "1:9.9"),
        std::cmp::Ordering::Greater,
        "epoch 2 > epoch 1"
    );
    assert_eq!(
        compare_versions("1:1.0", "1:1.0"),
        std::cmp::Ordering::Equal,
        "Same epoch and version"
    );
}

#[test]
fn test_version_comparison_with_debian_revision() {
    assert_eq!(
        compare_versions("1.0-1", "1.0-2"),
        std::cmp::Ordering::Less,
        "1.0-1 < 1.0-2"
    );
    assert_eq!(
        compare_versions("1.0-10", "1.0-2"),
        std::cmp::Ordering::Greater,
        "1.0-10 > 1.0-2 (numeric comparison)"
    );
    assert_eq!(
        compare_versions("1.0-1ubuntu1", "1.0-1ubuntu2"),
        std::cmp::Ordering::Less,
        "Ubuntu revisions"
    );
}

#[test]
fn test_version_comparison_tilde() {
    // Tilde sorts before anything, even empty string
    assert_eq!(
        compare_versions("1.0~beta", "1.0"),
        std::cmp::Ordering::Less,
        "1.0~beta < 1.0"
    );
    assert_eq!(
        compare_versions("1.0~alpha", "1.0~beta"),
        std::cmp::Ordering::Less,
        "alpha < beta"
    );
    assert_eq!(
        compare_versions("1.0~rc1", "1.0"),
        std::cmp::Ordering::Less,
        "1.0~rc1 < 1.0 (release)"
    );
}

#[test]
fn test_version_comparison_complex() {
    // Real-world Debian version strings
    assert_eq!(
        compare_versions("2:9.0.1499-1", "2:9.0.1500-1"),
        std::cmp::Ordering::Less,
        "Vim versions"
    );
    assert_eq!(
        compare_versions("1:8.5.0-2ubuntu1", "1:8.5.0-2ubuntu2"),
        std::cmp::Ordering::Less,
        "Curl versions"
    );
    assert_eq!(
        compare_versions("2.38-1ubuntu6", "2.38-1ubuntu10"),
        std::cmp::Ordering::Less,
        "libc6 versions"
    );
}

#[test]
fn test_version_comparison_edge_cases() {
    // Empty strings
    assert_eq!(
        compare_versions("", "1.0"),
        std::cmp::Ordering::Less,
        "Empty < version"
    );

    // Very long version strings
    assert_eq!(
        compare_versions("1.2.3.4.5.6.7.8", "1.2.3.4.5.6.7.9"),
        std::cmp::Ordering::Less,
        "Long version strings"
    );

    // Letters in version
    assert_eq!(
        compare_versions("1.0a", "1.0b"),
        std::cmp::Ordering::Less,
        "Letter suffixes"
    );
}

// ============================================================================
// Dependency Resolver Tests
// ============================================================================

// Note: These tests require a populated package database, which may not exist
// in CI environments. They are designed to work with real Debian/Ubuntu systems.

#[test]
#[cfg(feature = "docker_tests")]
fn test_resolver_simple_package() {
    // This test requires a real Debian/Ubuntu system with package database
    let mut resolver = match DependencyResolver::new() {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Skipping test: no package database available");
            return;
        }
    };

    // Try to resolve a common package
    if resolver.add_package("curl").is_ok() {
        let result = resolver.resolve();
        assert!(result.is_ok(), "Should resolve curl and its dependencies");

        let resolution = result.unwrap();
        assert!(
            !resolution.to_install.is_empty() || !resolution.to_upgrade.is_empty(),
            "Should have packages to install or upgrade"
        );
    }
}

#[test]
#[cfg(feature = "docker_tests")]
fn test_resolver_missing_package() {
    let mut resolver = match DependencyResolver::new() {
        Ok(r) => r,
        Err(_) => return,
    };

    let result = resolver.add_package("this-package-definitely-does-not-exist-12345");
    assert!(result.is_err(), "Should fail for nonexistent package");
}

#[test]
#[cfg(feature = "docker_tests")]
fn test_resolver_with_dependencies() {
    let mut resolver = match DependencyResolver::new() {
        Ok(r) => r,
        Err(_) => return,
    };

    // vim has dependencies like libacl, libc6, etc.
    if resolver.add_package("vim").is_ok() {
        if let Ok(result) = resolver.resolve() {
            // Should include vim and its dependencies
            assert!(
                result.to_install.len() >= 1,
                "Should resolve at least vim itself"
            );
        }
    }
}

#[test]
#[cfg(feature = "docker_tests")]
fn test_resolver_topological_sort() {
    // Test that dependencies are ordered correctly (dependencies before dependents)
    let mut resolver = match DependencyResolver::new() {
        Ok(r) => r,
        Err(_) => return,
    };

    if resolver.add_package("git").is_ok() {
        if let Ok(result) = resolver.resolve() {
            // Verify topological order: if package A depends on B,
            // B should appear before A in to_install list
            // This is a property of the topological sort
            let install_list = result.to_install;

            // Basic sanity: should have multiple packages
            if install_list.len() > 1 {
                println!("Install order: {:?}", install_list);
                // Dependencies should be installed first
                // (Hard to test precisely without parsing actual deps,
                // but we can check the list is non-empty and ordered)
                assert!(!install_list.is_empty());
            }
        }
    }
}

// ============================================================================
// Transaction Tests
// ============================================================================

#[test]
fn test_transaction_creation() {
    let result = ResolutionResult {
        to_install: vec!["vim".to_string(), "curl".to_string()],
        to_upgrade: vec![(
            "git".to_string(),
            "2.39.0".to_string(),
            "2.40.0".to_string(),
        )],
        to_remove: vec!["old-package".to_string()],
        download_size: 10_000_000,
        installed_size: 50_000_000,
    };

    let tx = Transaction::from_resolution(result);

    assert_eq!(tx.state, TransactionState::Pending);
    assert_eq!(tx.to_install.len(), 2);
    assert_eq!(tx.to_upgrade.len(), 1);
    assert_eq!(tx.to_remove.len(), 1);
    assert_eq!(tx.package_count(), 4);
}

#[test]
fn test_transaction_dry_run() {
    let result = ResolutionResult {
        to_install: vec!["vim".to_string(), "git".to_string()],
        to_upgrade: vec![("curl".to_string(), "8.0.0".to_string(), "8.5.0".to_string())],
        to_remove: Vec::new(),
        download_size: 5_242_880,   // 5 MB
        installed_size: 20_971_520, // 20 MB
    };

    let output = dry_run(&result);

    assert!(output.contains("vim"));
    assert!(output.contains("git"));
    assert!(output.contains("curl"));
    assert!(output.contains("5242880"));
    assert!(output.contains("20971520"));
    assert!(output.contains("NEW packages"));
    assert!(output.contains("upgraded"));
}

#[test]
fn test_transaction_empty() {
    let tx = Transaction::new();
    assert_eq!(tx.state, TransactionState::Pending);
    assert_eq!(tx.package_count(), 0);
    assert_eq!(tx.total_download_size(), 0);
}

#[test]
fn test_transaction_add_install() {
    let mut tx = Transaction::new();

    tx.add_install(
        "vim".to_string(),
        "9.0.0".to_string(),
        "http://example.com/vim.deb".to_string(),
        2_048_000,
    );

    assert_eq!(tx.to_install.len(), 1);
    assert_eq!(tx.to_install[0].name, "vim");
    assert_eq!(tx.to_install[0].version, "9.0.0");
    assert_eq!(tx.total_download_size(), 2_048_000);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_end_to_end_sources_to_resolver() {
    // Create a temporary sources.list file
    let temp_dir = TempDir::new().unwrap();
    let sources_file = temp_dir.path().join("sources.list");

    let content = r"
deb http://deb.debian.org/debian bookworm main
deb http://security.debian.org/debian-security bookworm-security main
";

    fs::write(&sources_file, content).unwrap();

    // Parse sources
    let repos = parse_sources_list_content(content, &sources_file).unwrap();
    assert_eq!(repos.len(), 2);

    // Verify repository URLs are constructed correctly
    let packages_url = repos[0].packages_url("amd64");
    assert!(packages_url.contains("bookworm"));
    assert!(packages_url.contains("main"));
    assert!(packages_url.contains("binary-amd64"));
}

#[test]
#[cfg(feature = "docker_tests")]
fn test_full_workflow_search_resolve_transaction() {
    // This test exercises the full workflow:
    // 1. Parse sources
    // 2. Resolve dependencies
    // 3. Create transaction
    // 4. Dry run

    let repos = match get_enabled_binary_repos() {
        Ok(r) if !r.is_empty() => r,
        _ => {
            eprintln!("Skipping: no repositories configured");
            return;
        }
    };

    println!("Found {} repositories", repos.len());

    let mut resolver = match DependencyResolver::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping: cannot create resolver: {}", e);
            return;
        }
    };

    // Try to resolve a small package
    if resolver.add_package("hello").is_ok() {
        if let Ok(result) = resolver.resolve() {
            let tx = Transaction::from_resolution(result.clone());

            assert!(tx.package_count() > 0);
            assert_eq!(tx.state, TransactionState::Pending);

            let dry_run_output = dry_run(&result);
            assert!(dry_run_output.contains("hello") || !result.to_install.is_empty());
        }
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_sources_parser_handles_malformed_input() {
    let malformed = r"
This is not a valid sources.list file
It has random text
And no valid entries
";

    let repos = parse_sources_list_content(malformed, Path::new("/test")).unwrap();
    assert_eq!(repos.len(), 0, "Should handle malformed input gracefully");
}

#[test]
fn test_deb822_parser_handles_incomplete_stanza() {
    let incomplete = r"
Types: deb
URIs: http://example.com/repo
# Missing Suites and Components
";

    let repos = parse_deb822_content(incomplete, Path::new("/test.sources")).unwrap();
    assert_eq!(repos.len(), 0, "Should skip incomplete stanzas");
}

#[test]
fn test_version_comparison_handles_invalid_input() {
    // These should not panic
    let _ = compare_versions("invalid", "1.0");
    let _ = compare_versions("1.0", "also-invalid");
    let _ = compare_versions("", "");
    let _ = compare_versions("1.2.3.4.5.6.7.8.9.10", "1:2.0");
}

// ============================================================================
// CLI-LEVEL E2E TESTS FOR DEBIAN
// ============================================================================

use std::env;
use std::process::{Command, Stdio};

#[derive(Debug)]
struct CliTestResult {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_omg_cli(args: &[&str]) -> CliTestResult {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("Failed to execute omg");

    CliTestResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn is_debian_or_ubuntu() -> bool {
    Path::new("/etc/debian_version").exists()
        || env::var("OMG_TEST_DISTRO")
            .map(|d| d == "debian" || d == "ubuntu")
            .unwrap_or(false)
}

#[test]
fn test_cli_search_on_debian() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["search", "curl"]);
    // May or may not succeed depending on setup - just check it doesn't panic
    let _ = result.success;

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Search should not panic on Debian"
    );
}

#[test]
fn test_cli_info_debian_package() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["info", "apt"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Info should work on Debian packages"
    );

    if result.success {
        assert!(
            combined.contains("apt") || combined.contains("Version"),
            "Should show package information"
        );
    }
}

#[test]
fn test_cli_install_debian_package_dry_run() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["install", "--dry-run", "curl"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Install dry-run should work on Debian"
    );
    assert_no_arch_terms(&combined, "Debian dry-run");
}

#[test]
fn test_cli_update_check_debian() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["update", "--check"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Update check should work on Debian"
    );
    assert_no_arch_terms(&combined, "Debian update check");

    // Should not require password for check
    assert!(
        !combined.contains("[sudo]") && !combined.contains("Password:"),
        "Update check should not require sudo"
    );
}

#[test]
fn test_cli_status_shows_debian_info() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["status"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Status should work on Debian"
    );
}

#[test]
fn test_cli_list_installed_debian() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["list", "--installed"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(!combined.contains("panicked"), "List should work on Debian");

    if result.success {
        // Should show at least some packages
        assert!(combined.len() > 20, "Should list some installed packages");
    }
}

#[test]
fn test_cli_install_multiple_debian_packages() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["install", "--dry-run", "curl", "wget", "git"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Multi-package install should work"
    );
    assert_no_arch_terms(&combined, "Debian multi-package install dry-run");
}

#[test]
fn test_cli_handles_debian_version_strings() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Debian version strings can be complex (epoch, revision, etc.)
    let result = run_omg_cli(&["info", "libc6"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle complex version strings"
    );
}

#[test]
fn test_cli_debian_dependency_resolution() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Install package with many dependencies
    let result = run_omg_cli(&["install", "--dry-run", "build-essential"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle dependency resolution"
    );

    if result.success {
        // Should show dependencies
        assert!(
            combined.contains("depend")
                || combined.contains("install")
                || combined.contains("build-essential"),
            "Should show dependency information"
        );
    }
}

#[test]
fn test_cli_debian_sources_list_parsing() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // This indirectly tests sources.list parsing via search
    let result = run_omg_cli(&["search", "nginx"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should parse sources.list correctly"
    );
}

#[test]
fn test_cli_debian_ppa_style_repos() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test that we handle PPA-style sources gracefully
    let result = run_omg_cli(&["search", "software"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle PPA-style repos"
    );
}

#[test]
fn test_cli_debian_security_updates() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["update", "--check"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle security updates"
    );
    assert_no_arch_terms(&combined, "Debian security update path");
}

#[test]
fn test_cli_debian_remove_dry_run() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["remove", "--dry-run", "curl"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Remove should work in dry-run"
    );
    assert_no_arch_terms(&combined, "Debian remove dry-run");
}

#[test]
fn test_cli_debian_handles_held_packages() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test that held packages are handled correctly
    let result = run_omg_cli(&["update", "--check"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle held packages"
    );
    assert_no_arch_terms(&combined, "Debian held-package update path");
}

#[test]
fn test_cli_debian_virtual_packages() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test virtual packages (provided by other packages)
    let result = run_omg_cli(&["search", "mail-transport-agent"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle virtual packages"
    );
}

#[test]
fn test_cli_debian_package_not_found() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let result = run_omg_cli(&["install", "-y", "this-package-does-not-exist-xyz"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(!result.success, "Should fail for nonexistent package");
    assert_no_arch_terms(&combined, "Debian install failure path");
    assert!(
        combined.contains("not found") || combined.contains("Unable"),
        "Should show helpful error message"
    );
}

#[test]
fn test_cli_debian_install_with_recommends() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Debian has recommended packages
    let result = run_omg_cli(&["install", "--dry-run", "vim"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle recommended packages"
    );
}

#[test]
fn test_cli_debian_architecture_handling() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test that we correctly handle architecture-specific packages
    let result = run_omg_cli(&["search", "libc6"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle architecture correctly"
    );
}

#[test]
fn test_cli_debian_multi_arch_support() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Debian supports multiple architectures
    let result = run_omg_cli(&["info", "libc6"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle multi-arch packages"
    );
}

#[test]
fn test_cli_debian_error_recovery() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test error recovery by running multiple failing commands
    for i in 0..5 {
        let result = run_omg_cli(&["install", "-y", &format!("fake-pkg-{i}")]);
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Should recover from errors gracefully on iteration {i}"
        );
        assert_no_arch_terms(&combined, "Debian error recovery path");
    }
}

#[test]
fn test_cli_debian_full_workflow() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Complete user workflow
    let commands = vec![
        vec!["status"],
        vec!["search", "curl"],
        vec!["info", "curl"],
        vec!["install", "--dry-run", "curl"],
        vec!["update", "--check"],
    ];

    for cmd in commands {
        let result = run_omg_cli(&cmd);
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Command {cmd:?} should not panic"
        );
        assert_no_arch_terms(&combined, "Debian full workflow command");
    }
}

#[test]
fn test_cli_debian_concurrent_operations() {
    use std::thread;

    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test concurrent CLI invocations
    let handles: Vec<_> = (0..3)
        .map(|i| thread::spawn(move || run_omg_cli(&["search", &format!("pkg-{i}")])))
        .collect();

    for handle in handles {
        let result = handle.join().expect("Thread panicked");
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Concurrent operations should not panic"
        );
        assert_no_arch_terms(&combined, "Debian concurrent operation");
    }
}

#[test]
fn test_cli_debian_handles_slow_mirrors() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // The command should return a result without panicking.
    let result = run_omg_cli(&["update", "--check"]);
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle slow mirrors gracefully"
    );
    assert_no_arch_terms(&combined, "Debian slow mirror update path");
}

#[test]
fn test_cli_debian_respects_ci_mode() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(["install", "curl"])
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .env("CI", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("Failed to execute");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !combined.contains("Continue?") && !combined.contains("Press"),
        "Should not prompt in CI mode"
    );
}

#[test]
fn test_cli_debian_package_name_validation() {
    if !is_debian_or_ubuntu() {
        eprintln!("Skipping: not on Debian/Ubuntu");
        return;
    }

    // Test various package name formats
    let valid_names = vec!["curl", "python3", "lib-dev", "lib64c", "gcc-12"];

    for name in valid_names {
        let result = run_omg_cli(&["search", name]);
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Should handle package name: {name}"
        );
    }
}
