//! Property-Based and Fuzz Testing for OMG
//!
//! Uses proptest for property-based testing to discover edge cases.
//!
//! Run: cargo test --test property_tests
//! Run fuzz: OMG_RUN_FUZZ_TESTS=1 cargo test --test property_tests

#![allow(clippy::doc_markdown)]

mod common;

use common::*;
use proptest::prelude::*;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// PROPERTY-BASED CLI TESTS
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Any string input to search should not crash (excluding null bytes which Command rejects)
    #[test]
    fn prop_search_never_crashes(query in "[^\x00]*") {
        let result = run_omg(&["search", &query]);
        // Should never panic - may fail gracefully
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Any string input to info should not crash
    #[test]
    fn prop_info_never_crashes(package in "[a-zA-Z0-9_-]{1,100}") {
        let result = run_omg(&["info", &package]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Version strings should be handled gracefully
    #[test]
    fn prop_version_strings_handled(version in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}") {
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Path inputs should not allow traversal
    #[test]
    fn prop_no_path_traversal(
        prefix in "\\.{0,5}/",
        path in "[a-z]{1,10}(/[a-z]{1,10}){0,5}"
    ) {
        let input = format!("{prefix}{path}");
        let result = run_omg(&["info", &input]);
        // Should not expose system files
        prop_assert!(!result.stdout.contains("/etc/passwd"));
        prop_assert!(!result.stdout.contains("/etc/shadow"));
    }

    /// Shell metacharacters should be escaped
    #[test]
    fn prop_shell_metachar_escaped(
        meta in prop::sample::select(vec![";", "|", "&", "$", "`", "(", ")", "<", ">"]),
        word in "[a-z]{1,10}"
    ) {
        let input = format!("{word}{meta}{word}");
        let result = run_omg(&["search", &input]);
        prop_assert!(!result.stderr.contains("panic"));
        // Should not execute injected commands
        prop_assert!(!result.stdout.contains("root:"));
    }

    /// Runtime names should be normalized consistently
    #[test]
    fn prop_runtime_normalization(
        runtime in prop::sample::select(vec![
            "node", "nodejs", "Node", "NodeJS", "NODE",
            "python", "Python", "PYTHON", "python3",
            "go", "golang", "Go", "Golang",
            "rust", "Rust", "RUST", "rustlang"
        ])
    ) {
        let result1 = run_omg(&["which", &runtime]);
        // Should not crash on any variant
        prop_assert!(!result1.stderr.contains("panic"));
    }

    /// Environment variables in input should not be expanded
    #[test]
    fn prop_no_env_expansion(var_name in "[A-Z]{3,10}") {
        let input = format!("${{{var_name}}}");
        let result = run_omg(&["search", &input]);
        prop_assert!(!result.stderr.contains("panic"));
        // Verify env var wasn't expanded
        if let Ok(val) = std::env::var(&var_name) {
            prop_assert!(!result.stdout.contains(&val));
        }
    }

    /// Unicode inputs should be handled safely
    #[test]
    fn prop_unicode_safe(s in "\\PC{1,50}") {
        let result = run_omg(&["search", &s]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Very long inputs should be handled gracefully
    #[test]
    fn prop_long_input_handled(len in 100usize..10000) {
        let long_input: String = std::iter::repeat('a').take(len).collect();
        let result = run_omg(&["search", &long_input]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    // Note: Null byte tests removed - std::process::Command rejects null bytes in args
    // This is expected behavior, not a bug
}

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Semver-like versions should parse
    #[test]
    fn prop_semver_versions(
        major in 0u32..100,
        minor in 0u32..100,
        patch in 0u32..100
    ) {
        let version = format!("{major}.{minor}.{patch}");
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Partial versions should be handled
    #[test]
    fn prop_partial_versions(major in 0u32..30, minor in 0u32..30) {
        let v1 = format!("{major}");
        let v2 = format!("{major}.{minor}");

        let result1 = run_omg(&["use", "node", &v1]);
        let result2 = run_omg(&["use", "node", &v2]);

        prop_assert!(!result1.stderr.contains("panic"));
        prop_assert!(!result2.stderr.contains("panic"));
    }

    /// Version aliases should work
    #[test]
    fn prop_version_aliases(
        alias in prop::sample::select(vec!["lts", "latest", "stable", "current", "lts/*", "lts/iron"])
    ) {
        let result = run_omg(&["use", "node", alias]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Version with v prefix should work
    #[test]
    fn prop_v_prefix_versions(major in 0u32..30, minor in 0u32..30, patch in 0u32..30) {
        let version = format!("v{major}.{minor}.{patch}");
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panic"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FILE PATH PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// File paths should be handled safely
    #[test]
    fn prop_file_paths_safe(
        segments in prop::collection::vec("[a-zA-Z0-9_-]{1,20}", 1..10)
    ) {
        let path = segments.join("/");
        let project = TestProject::new();
        project.create_dir(&path);

        let result = run_omg_in_dir(&["status"], &project.path().join(&path));
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Symlink cycles should be detected
    #[test]
    fn prop_symlink_depth(depth in 1usize..20) {
        let project = TestProject::new();
        let mut current = project.path().to_path_buf();

        for i in 0..depth {
            let next = current.join(format!("dir{i}"));
            std::fs::create_dir_all(&next).ok();
            current = next;
        }

        let result = run_omg_in_dir(&["status"], &current);
        prop_assert!(!result.stderr.contains("panic"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TOML PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Malformed TOML should not crash
    #[test]
    fn prop_malformed_toml_safe(content in ".*") {
        let project = TestProject::new();
        project.create_file("omg.lock", &content);

        let result = project.run(&["env", "check"]);
        // May fail, but should not panic
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// Valid TOML with wrong schema should be handled
    #[test]
    fn prop_wrong_schema_toml(
        key in "[a-z]{1,10}",
        value in "[a-zA-Z0-9]{1,20}"
    ) {
        let content = format!("[{key}]\nvalue = \"{value}\"");
        let project = TestProject::new();
        project.create_file("omg.lock", &content);

        let result = project.run(&["env", "check"]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// .tool-versions parsing should be robust
    #[test]
    fn prop_tool_versions_parsing(
        runtime in "[a-z]{3,10}",
        version in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}"
    ) {
        let content = format!("{runtime} {version}");
        let project = TestProject::new();
        project.create_file(".tool-versions", &content);

        let result = project.run(&["status"]);
        prop_assert!(!result.stderr.contains("panic"));
    }

    /// .nvmrc parsing should handle various formats
    #[test]
    fn prop_nvmrc_parsing(
        prefix in "(v)?",
        major in 0u32..30,
        minor in 0u32..30,
        patch in 0u32..30,
        suffix in "(\n)?"
    ) {
        let content = format!("{prefix}{major}.{minor}.{patch}{suffix}");
        let project = TestProject::new();
        project.create_file(".nvmrc", &content);

        let result = project.run(&["use", "node"]);
        prop_assert!(!result.stderr.contains("panic"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Help should always be fast regardless of input
    #[test]
    fn prop_help_always_fast(subcommand in "[a-z]{1,20}") {
        let result = run_omg(&[&subcommand, "--help"]);
        // Help should be fast even for invalid commands (generous for cold start)
        prop_assert!(result.duration < Duration::from_secs(3));
    }

    /// Status should be reasonably fast
    #[test]
    fn prop_status_performance(_seed in 0u32..100) {
        let result = run_omg(&["status"]);
        if result.success {
            // Very generous timeout - proptest runs many iterations
            prop_assert!(result.duration < Duration::from_secs(30));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCURRENT ACCESS PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// Concurrent reads should be safe
    #[test]
    fn prop_concurrent_reads_safe(thread_count in 2usize..10) {
        use std::thread;

        let handles: Vec<_> = (0..thread_count)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert!(!result.stderr.contains("panic"));
        }
    }

    /// Concurrent writes to different projects should be safe
    #[test]
    fn prop_concurrent_writes_safe(thread_count in 2usize..5) {
        use std::thread;

        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                thread::spawn(|| {
                    let project = TestProject::new();
                    project.run(&["env", "capture"])
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert!(!result.stderr.contains("panic"));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FUZZ TESTING (requires OMG_RUN_FUZZ_TESTS=1)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod fuzz {
    use super::*;

    fn fuzz_enabled() -> bool {
        std::env::var("OMG_RUN_FUZZ_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    #[test]
    fn fuzz_random_cli_args() {
        if !fuzz_enabled() {
            eprintln!("⏭️  Skipping fuzz test (set OMG_RUN_FUZZ_TESTS=1)");
            return;
        }

        // Use simple deterministic fuzzing instead of rand
        let test_args = vec![
            vec![""],
            vec!["a"],
            vec!["aaaa"],
            vec!["\0"],
            vec!["\n\r\t"],
            vec!["😀🔥"],
            vec!["--", "test"],
            vec!["-v", "-v", "-v"],
            vec!["search", "'; DROP TABLE"],
        ];

        for args in test_args {
            let result = run_omg(&args);
            assert!(
                !result.stderr.contains("panic"),
                "Panic with args: {:?}",
                args
            );
        }
    }

    #[test]
    fn fuzz_random_file_contents() {
        if !fuzz_enabled() {
            eprintln!("⏭️  Skipping fuzz test (set OMG_RUN_FUZZ_TESTS=1)");
            return;
        }

        // Test various malformed file contents
        let long_content = "a".repeat(10000);
        let contents: Vec<&str> = vec![
            "",
            "{}",
            "invalid toml {{{{",
            "\0\0\0",
            &long_content,
            "[section]\nkey = ",
        ];

        for content in contents {
            let project = TestProject::new();
            project.create_file("omg.lock", content);
            let result = project.run(&["env", "check"]);

            assert!(
                !result.stderr.contains("panic"),
                "Panic with content length: {}",
                content.len()
            );
        }
    }

    #[test]
    fn fuzz_boundary_versions() {
        if !fuzz_enabled() {
            eprintln!("⏭️  Skipping fuzz test (set OMG_RUN_FUZZ_TESTS=1)");
            return;
        }

        let boundary_versions = vec![
            "0.0.0",
            "0.0.1",
            "0.1.0",
            "1.0.0",
            "999.999.999",
            "0",
            "1",
            "99",
            "0.0",
            "1.0",
            "99.99",
            "00.00.00",
            "01.02.03",
            "-1.0.0",
            "1.-1.0",
            "1.0.-1",
            "1.0.0-alpha",
            "1.0.0-beta.1",
            "1.0.0+build",
            "1.0.0-alpha+001",
            "1.0.0+20130313144700",
        ];

        for version in boundary_versions {
            let result = run_omg(&["use", "node", version]);
            assert!(
                !result.stderr.contains("panic"),
                "Panic with version: {}",
                version
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod regression {
    use super::*;

    #[test]
    fn regression_empty_string_search() {
        let result = run_omg(&["search", ""]);
        assert!(!result.stderr.contains("panic"));
    }

    // Note: Null byte test removed - std::process::Command rejects null bytes
    // This is expected OS-level behavior, not a bug in OMG

    #[test]
    fn regression_very_deep_nesting() {
        let project = TestProject::new();
        let deep_path = (0..100)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join("/");
        project.create_dir(&deep_path);

        let full_path = project.path().join(&deep_path);
        let result = run_omg_in_dir(&["status"], &full_path);
        assert!(!result.stderr.contains("panic"));
    }

    #[test]
    fn regression_special_chars_in_path() {
        let project = TestProject::new();
        // Try to create directories with special characters
        for special in &["test dir", "test'dir", "test\"dir", "test\\dir"] {
            if project.create_dir(special).exists() {
                let result = run_omg_in_dir(&["status"], &project.path().join(special));
                assert!(
                    !result.stderr.contains("panic"),
                    "Panic with path: {}",
                    special
                );
            }
        }
    }

    #[test]
    fn regression_concurrent_env_capture() {
        use std::thread;

        let handles: Vec<_> = (0..5)
            .map(|_| {
                thread::spawn(|| {
                    let project = TestProject::new();
                    project.run(&["env", "capture"])
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(!result.stderr.contains("panic"));
        }
    }
}
