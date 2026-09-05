#![cfg(all(target_os = "linux", feature = "fedora"))]

use anyhow::Result;
use omg_lib::package_managers::{DnfPackageManager, PackageManager};

pub mod common;
pub mod platform_semantics;

use platform_semantics::{assert_no_arch_terms, assert_no_debian_terms, assert_no_macos_terms};

mod dnf_integration {
    use super::*;

    #[tokio::test]
    async fn test_dnf_package_manager_creation() {
        let pm = DnfPackageManager::new();
        assert_eq!(pm.name(), "dnf");
        let identity = pm.name().to_string();
        assert_no_debian_terms(&identity, "Fedora package manager identity");
        assert_no_arch_terms(&identity, "Fedora package manager identity");
        assert_no_macos_terms(&identity, "Fedora package manager identity");
    }

    #[tokio::test]
    async fn repository_lookup_finds_tree_before_installation() -> Result<()> {
        if common::TestConfig::default().skip_if_no_system("dnf_uninstalled_repository_lookup") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let pm = DnfPackageManager::new();
        assert!(
            !pm.list_installed()
                .await?
                .iter()
                .any(|package| package.name == "tree"),
            "run this regression in the fresh Fedora smoke image with tree absent"
        );
        let search = pm.search("tree").await?;
        assert!(
            search
                .iter()
                .any(|package| package.name == "tree" && !package.installed)
        );
        let info = pm
            .info("tree")
            .await?
            .expect("available repository package");
        assert_eq!(info.name, "tree");
        assert!(!info.installed);

        for arguments in [vec!["info", "tree"], vec!["--json", "info", "tree"]] {
            let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("omg"))
                .args(arguments)
                .output()?;
            assert!(
                output.status.success(),
                "CLI info failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("tree"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn explicit_cli_matches_native_backend_without_a_daemon() -> Result<()> {
        if common::TestConfig::default().skip_if_no_system("dnf_explicit_cli") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let mut expected = DnfPackageManager::new().list_explicit().await?;
        expected.sort();
        expected.dedup();
        let binary = assert_cmd::cargo::cargo_bin!("omg");
        let listing = std::process::Command::new(binary)
            .args(["--json", "explicit"])
            .output()?;
        assert!(
            listing.status.success(),
            "{}",
            String::from_utf8_lossy(&listing.stderr)
        );
        let listing: serde_json::Value = serde_json::from_slice(&listing.stdout)?;
        assert_eq!(listing["packages"], serde_json::json!(expected));
        let count = std::process::Command::new(binary)
            .args(["--json", "explicit", "--count"])
            .output()?;
        assert!(
            count.status.success(),
            "{}",
            String::from_utf8_lossy(&count.stderr)
        );
        let count: serde_json::Value = serde_json::from_slice(&count.stdout)?;
        assert_eq!(count["count"], serde_json::json!(expected.len()));
        Ok(())
    }

    #[test]
    fn size_cli_matches_native_installed_packages() -> Result<()> {
        if common::TestConfig::default().skip_if_no_system("dnf_size_cli") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let before = std::process::Command::new("rpm")
            .args([
                "-qa",
                "--qf",
                "%{NAME}\t%{NAME}-%{EPOCHNUM}:%{VERSION}-%{RELEASE}.%{ARCH}\t%{SIZE}\n",
            ])
            .output()?;
        assert!(before.status.success());
        let snapshot = String::from_utf8(before.stdout)?;
        let mut packages = snapshot
            .lines()
            .filter(|line| !line.starts_with("gpg-pubkey\t"))
            .map(|line| -> Result<(&str, i64)> {
                let (_, sized_identity) = line.split_once('\t').expect("RPM package name");
                let (name, size) = sized_identity.split_once('\t').expect("RPM size row");
                Ok((name, size.parse()?))
            })
            .collect::<Result<Vec<_>>>()?;
        packages.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        let binary = assert_cmd::cargo::cargo_bin!("omg");
        let top = std::process::Command::new(binary)
            .env("NO_COLOR", "1")
            .env("LC_ALL", "C")
            .args(["size", "--limit", "3"])
            .output()?;
        assert!(
            top.status.success(),
            "{}",
            String::from_utf8_lossy(&top.stderr)
        );
        let top = String::from_utf8(top.stdout)?;
        let mut cursor = 0;
        for (name, _) in packages.iter().take(3) {
            cursor += top[cursor..].find(name).expect("ranked native identity") + name.len();
        }
        assert!(top.contains(&format!("Number of Packages: {}", packages.len())));
        let providers = std::process::Command::new("dnf")
            .args([
                "--setopt=disable_excludes=*",
                "repoquery",
                "--installed",
                "--providers-of=requires",
                "--qf",
                "%{full_nevra}\n",
                "glibc",
            ])
            .output()?;
        assert!(providers.status.success());
        let providers = String::from_utf8(providers.stdout)?;
        let root = std::process::Command::new("rpm")
            .args([
                "-q",
                "glibc",
                "--qf",
                "%{NAME}-%{EPOCHNUM}:%{VERSION}-%{RELEASE}.%{ARCH}\n",
            ])
            .output()?;
        assert!(root.status.success());
        let root = String::from_utf8(root.stdout)?;
        let expected: std::collections::BTreeSet<_> =
            providers.lines().chain(root.lines()).collect();
        let tree = std::process::Command::new(binary)
            .env("NO_COLOR", "1")
            .env("LC_ALL", "C")
            .args(["size", "--tree", "glibc", "--limit", "10000"])
            .output()?;
        assert!(
            tree.status.success(),
            "{}",
            String::from_utf8_lossy(&tree.stderr)
        );
        let tree = String::from_utf8(tree.stdout)?;
        for name in &expected {
            assert!(tree.contains(name), "missing provider {name}");
        }
        assert!(tree.contains(&format!("Number of Packages: {}", expected.len())));
        assert!(tree.contains("not a minimal dependency closure"));
        let missing = std::process::Command::new(binary)
            .args(["size", "--tree", "omg-no-such-package"])
            .output()?;
        assert!(!missing.status.success());
        assert!(String::from_utf8_lossy(&missing.stderr).contains("is not installed"));
        let after = std::process::Command::new("rpm")
            .args([
                "-qa",
                "--qf",
                "%{NAME}\t%{NAME}-%{EPOCHNUM}:%{VERSION}-%{RELEASE}.%{ARCH}\t%{SIZE}\n",
            ])
            .output()?;
        assert!(after.status.success());
        let after = String::from_utf8(after.stdout)?;
        let mut before: Vec<_> = snapshot.lines().collect();
        let mut after: Vec<_> = after.lines().collect();
        before.sort_unstable();
        after.sort_unstable();
        assert_eq!(before, after);
        Ok(())
    }

    #[test]
    fn installed_metadata_does_not_hide_excluded_packages() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        if common::TestConfig::default().skip_if_no_system("dnf_installed_metadata_exclusions") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let cache = dirs::cache_dir().expect("user cache directory");
        std::fs::create_dir_all(&cache)?;
        let fixture = tempfile::tempdir_in(cache)?;
        let dnf = fixture.path().join("dnf");
        std::fs::write(
            &dnf,
            "#!/bin/sh\nexec /usr/bin/dnf --setopt=excludepkgs=glibc \"$@\"\n",
        )?;
        std::fs::set_permissions(&dnf, std::fs::Permissions::from_mode(0o700))?;
        let hidden = std::process::Command::new(&dnf)
            .args(["repoquery", "--installed", "--qf", "%{name}\n", "glibc"])
            .output()?;
        assert!(hidden.status.success());
        assert!(
            hidden.stdout.is_empty(),
            "exclusion fixture must hide glibc from ordinary queries"
        );
        let selector = format!("glibc.{}", std::env::consts::ARCH);
        let inherited = std::env::var_os("PATH").expect("native command PATH");
        let path = std::env::join_paths(
            std::iter::once(fixture.path().to_path_buf()).chain(std::env::split_paths(&inherited)),
        )?;
        for arguments in [
            vec!["size", "--limit", "0"],
            vec!["size", "--tree", &selector],
            vec!["why", &selector],
            vec!["why", "--reverse", &selector],
        ] {
            let baseline = std::process::Command::new(assert_cmd::cargo::cargo_bin!("omg"))
                .env("NO_COLOR", "1")
                .args(&arguments)
                .output()?;
            assert!(
                baseline.status.success(),
                "{}",
                String::from_utf8_lossy(&baseline.stderr)
            );
            let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("omg"))
                .env("PATH", &path)
                .env("NO_COLOR", "1")
                .args(&arguments)
                .output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                baseline.stdout, output.stdout,
                "exclusions changed {arguments:?}"
            );
        }
        fixture.close()?;
        Ok(())
    }

    #[test]
    fn why_cli_matches_native_reasons_and_reverse_requirements() -> Result<()> {
        if common::TestConfig::default().skip_if_no_system("dnf_why_cli") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let selector = format!("glibc.{}", std::env::consts::ARCH);
        #[expect(
            clippy::literal_string_with_formatting_args,
            reason = "DNF interprets these query placeholders, not Rust"
        )]
        let format = "%{full_nevra}\t%{reason}\n";
        let native = std::process::Command::new("dnf")
            .args([
                "--setopt=disable_excludes=*",
                "repoquery",
                "--installed",
                "--qf",
                format,
                &selector,
            ])
            .output()?;
        assert!(native.status.success());
        let native = String::from_utf8(native.stdout)?;
        assert_eq!(native.lines().count(), 1);
        let (identity, reason) = native
            .trim_end_matches('\n')
            .split_once('\t')
            .expect("native installation reason");
        let binary = assert_cmd::cargo::cargo_bin!("omg");
        let output = std::process::Command::new(binary)
            .env("NO_COLOR", "1")
            .args(["why", &selector])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = String::from_utf8(output.stdout)?;
        assert!(output.contains(identity));
        assert!(output.contains(&format!("Reason: {reason}")));
        let native = std::process::Command::new("dnf")
            .args([
                "--setopt=disable_excludes=*",
                "repoquery",
                "--installed",
                "--qf",
                format,
                &format!("--whatrequires={selector}"),
            ])
            .output()?;
        assert!(native.status.success());
        let native = String::from_utf8(native.stdout)?;
        let expected: Vec<_> = native
            .lines()
            .map(|line| line.split_once('\t').expect("native dependent"))
            .filter(|(name, _)| *name != identity)
            .collect();
        let output = std::process::Command::new(binary)
            .env("NO_COLOR", "1")
            .args(["why", "--reverse", &selector])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = String::from_utf8(output.stdout)?;
        for (identity, reason) in &expected {
            assert!(
                output.contains(&format!("{identity}: {reason}")),
                "missing {identity}"
            );
        }
        assert!(
            !expected.is_empty(),
            "glibc fixture must have native requiring packages"
        );
        assert!(output.contains(&format!("Dependents ({})", expected.len())));
        assert!(!output.contains("Safe to remove:"));
        for arguments in [
            vec!["why", "omg-no-such-package"],
            vec!["why", "--reverse", "omg-no-such-package"],
        ] {
            let output = std::process::Command::new(binary)
                .args(arguments)
                .output()?;
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("is not installed"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn cleanup_preview_preserves_installed_packages() -> Result<()> {
        if common::TestConfig::default().skip_if_no_system("dnf_cleanup_preview") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }
        let snapshot = || -> Result<Vec<String>> {
            let output = std::process::Command::new("rpm")
                .args(["-qa", "--qf", "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n"])
                .output()?;
            assert!(output.status.success());
            let mut packages: Vec<String> = String::from_utf8(output.stdout)?
                .lines()
                .map(str::to_owned)
                .collect();
            packages.sort();
            Ok(packages)
        };
        let before = snapshot()?;
        let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("omg"))
            .args(["clean", "--all", "--dry-run"])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8(output.stdout)?.contains("No changes made (dry run)"));
        assert_eq!(snapshot()?, before);
        Ok(())
    }

    #[tokio::test]
    async fn test_search_common_package() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let results = pm.search("vim").await.unwrap();

        assert!(!results.is_empty(), "Should find vim package");
        assert!(
            results.iter().any(|p| p.name.contains("vim")),
            "Results should contain vim"
        );
    }

    /// Searches shell out to the host's dnf/rpm, so they require a real
    /// Fedora-class system just like the other dnf_integration tests.
    #[tokio::test]
    async fn test_search_nonexistent_package() -> Result<()> {
        // Gate inline (instead of require_system_tests!, whose `return;` is
        // incompatible with this test's Result signature).
        if common::TestConfig::default().skip_if_no_system("dnf_search_nonexistent") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        let results = pm.search("nonexistent-package-xyz-12345").await?;

        assert!(results.is_empty(), "Should not find nonexistent package");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_packages() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await.unwrap();

        assert!(
            !installed.is_empty(),
            "Should have installed packages on Fedora system"
        );
    }

    #[tokio::test]
    async fn test_list_updates() -> Result<()> {
        // Gate inline (instead of require_system_tests!, whose `return;` is
        // incompatible with this test's Result signature).
        if common::TestConfig::default().skip_if_no_system("dnf_list_updates") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        // The call must succeed on a real system, and every reported update
        // must be well-formed: named package with distinct old/new versions.
        let updates = pm.list_updates().await?;
        for update in &updates {
            assert!(
                !update.name.is_empty(),
                "every update entry needs a package name"
            );
            assert!(
                update.old_version != update.new_version,
                "update for {} must change version ({} -> {})",
                update.name,
                update.old_version,
                update.new_version
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_is_installed_check() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let is_bash_installed = pm.is_installed("bash").await.unwrap();
        assert!(
            is_bash_installed,
            "bash should be installed on Fedora system"
        );
    }
}

mod dnf_rpm_database {
    use super::*;

    #[tokio::test]
    async fn test_rpm_database_query() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await.unwrap();

        assert!(
            !installed.is_empty(),
            "Should read packages from RPM database"
        );

        let has_rpm = installed.iter().any(|p| p.name.contains("rpm"));
        assert!(has_rpm, "Should find rpm package itself in database");
    }
}

mod dnf_operations {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_install_and_remove_package() -> Result<()> {
        let pm = DnfPackageManager::new();

        let test_package = "nano";

        let initial_check = pm.is_installed(test_package).await.unwrap();
        if initial_check {
            pm.remove(&[test_package.to_string()]).await?;
        }

        pm.install(&[test_package.to_string()]).await?;
        assert!(
            pm.is_installed(test_package).await.unwrap(),
            "Package should be installed"
        );

        pm.remove(&[test_package.to_string()]).await?;
        assert!(
            !pm.is_installed(test_package).await.unwrap(),
            "Package should be removed"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_update_all_packages() -> Result<()> {
        let pm = DnfPackageManager::new();

        pm.update().await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_sync_repository_metadata() -> Result<()> {
        let pm = DnfPackageManager::new();

        pm.sync().await?;

        Ok(())
    }
}

mod dnf_error_handling {
    use super::*;

    #[tokio::test]
    async fn test_install_invalid_package() {
        if !omg_lib::core::is_root() {
            common::report_skip("requires root privileges");
            return;
        }

        let pm = DnfPackageManager::new();

        let result = pm
            .install(&["nonexistent-package-xyz-12345".to_string()])
            .await;

        assert!(
            result.is_err(),
            "Should fail to install nonexistent package"
        );
    }
}
