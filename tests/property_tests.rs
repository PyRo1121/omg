#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::doc_markdown
)]
//! Property-Based and Fuzz Testing for OMG
//!
//! Uses proptest for property-based testing to discover edge cases.
//!
//! Run: cargo test --test property_tests
//! Run fuzz: OMG_RUN_FUZZ_TESTS=1 cargo test --test property_tests

pub mod common;

use common::*;
use proptest::prelude::*;

// ═══════════════════════════════════════════════════════════════════════════════
// PROPERTY-BASED CLI TESTS
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Any string input to search should not crash (excluding null bytes which Command rejects)
    #[test]
    fn prop_search_never_crashes(query in "[^\x00]*") {
        let result = run_omg(&["search", &query]);
        prop_assert!(!result.stderr.contains("panicked at"));

        if result.success && !result.stdout.is_empty() {
            // Successful searches must show a recognizable results header or
            // an explicit empty-result notice — not arbitrary prose (the old
            // "git"/"pacman" alternatives matched any mention).
            let has_valid_output = result.stdout.contains("Search Results") ||
                                  result.stdout.contains("Package") ||
                                  result.stdout.contains("No results");
            prop_assert!(
                has_valid_output,
                "Search output should contain results, got: {}",
                result.stdout.chars().take(100).collect::<String>()
            );
            prop_assert!(!result.stdout.contains("root:"), "Should not leak /etc/passwd");
            prop_assert!(!result.stdout.contains("PRIVATE_KEY"), "Should not leak secrets");
        }
    }

    /// Any string input to info should not crash
    #[test]
    fn prop_info_never_crashes(package in "[a-zA-Z0-9_-]{1,100}") {
        let result = run_omg(&["info", &package]);
        prop_assert!(!result.stderr.contains("panicked at"));
    }

    /// Version strings should be handled gracefully
    #[test]
    fn prop_version_strings_handled(version in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}") {
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panicked at"));
        // Generated versions always pass `validate_runtime_version`
        // (digits and dots only, never "current", no ':'/'~'), so the switch
        // header from src/cli/runtimes.rs must be announced before any
        // install attempt, regardless of whether the install then succeeds.
        prop_assert!(
            result.stdout.contains(&format!("Switching node to version {version}")),
            "`use node` must announce the target version, got stdout: {}",
            result.stdout.chars().take(200).collect::<String>()
        );
    }

    /// Path inputs should not allow traversal
    #[test]
    fn prop_no_path_traversal(
        prefix in "\\.{0,5}/",
        path in "[a-z]{1,10}(/[a-z]{1,10}){0,5}"
    ) {
        let input = format!("{prefix}{path}");
        let result = run_omg(&["info", &input]);
        prop_assert!(!result.stderr.contains("panicked at"));
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
        prop_assert!(!result.stderr.contains("panicked at"));

        prop_assert!(!result.stdout.contains("root:"), "Should not leak /etc/passwd");
        prop_assert!(!result.stdout.contains("/etc/shadow"), "Should not access shadow file");
        prop_assert!(!result.stdout.contains("uid="), "Should not execute `id` command");
        prop_assert!(!result.stderr.contains("sh:"), "Should not spawn shell");

        if result.success {
            let has_results = result.stdout.contains("Search Results") ||
                            result.stdout.contains("No results") ||
                            result.stdout.contains("Package");
            prop_assert!(
                has_results,
                "Valid search should return package results or no results"
            );
        }
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
        let result1 = run_omg(&["which", runtime]);
        // Should not crash on any variant
        prop_assert!(!result1.stderr.contains("panicked at"));
        // Every accepted alias must resolve through canonical_runtime_name
        // (src/cli/runtimes.rs) and print an answer naming the requested
        // runtime: either its active version or the explicit "no version set"
        // notice. A silent success would mean the lookup was skipped.
        if result1.success {
            prop_assert!(
                result1.stdout.contains(runtime) || result1.stderr.contains(runtime),
                "`which {runtime}` success must name the runtime, got: {}",
                result1.combined_output().chars().take(200).collect::<String>()
            );
        } else {
            prop_assert!(
                !result1.stderr.is_empty(),
                "`which {runtime}` failure must explain why on stderr"
            );
        }
    }

    /// Environment variables in input should not be expanded
    #[test]
    fn prop_no_env_expansion(var_name in "[A-Z]{3,10}") {
        // Inject a canary value for the variable so the non-expansion claim
        // is exercised deterministically instead of only when the variable
        // happens to be set in the test process.
        let canary = format!("canary-{var_name}-must-not-expand");
        let input = format!("${{{var_name}}}");
        let result = run_omg_with_env(&["search", &input], &[(&var_name, canary.as_str())]);
        prop_assert!(!result.stderr.contains("panicked at"));
        prop_assert!(
            !result.stdout.contains(&canary),
            "Search query env var must not be expanded into output"
        );
    }

    /// Unicode inputs should be handled safely
    #[test]
    fn prop_unicode_safe(s in "\\PC{1,50}") {
        let result = run_omg(&["search", &s]);
        prop_assert!(!result.stderr.contains("panicked at"));

        // UTF-8 validity is guaranteed by the harness (`from_utf8_lossy`),
        // so the real contract here is structured output on every success.
        if result.success {
            let has_structured_output = result.stdout.is_empty() ||
                                       result.stdout.contains("Search Results") ||
                                       result.stdout.contains("No results");
            prop_assert!(has_structured_output, "Valid output should be structured");
        } else {
            prop_assert!(
                !result.stderr.is_empty(),
                "Failed search must explain why on stderr"
            );
        }
    }

    /// Very long inputs should be handled gracefully
    #[test]
    fn prop_long_input_handled(len in 100usize..10000) {
        let long_input: String = "a".repeat(len);
        let result = run_omg(&["search", &long_input]);
        prop_assert!(!result.stderr.contains("panicked at"));

        prop_assert!(
            result.stdout.len() < len * 100,
            "Output should not be exponentially larger than input (input: {}, output: {})",
            len,
            result.stdout.len()
        );

        prop_assert!(
            result.stdout.len() + result.stderr.len() > 0,
            "Should produce some output (results header or error message)"
        );
    }

    // Note: Null byte tests removed - std::process::Command rejects null bytes in args
    // This is expected behavior, not a bug
}

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// Semver-like versions should parse
    #[test]
    fn prop_semver_versions(
        major in 0u32..100,
        minor in 0u32..100,
        patch in 0u32..100
    ) {
        let version = format!("{major}.{minor}.{patch}");
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panicked at"));
        // These generated versions always pass validation (digits and dots),
        // so the switch header from src/cli/runtimes.rs must be printed no
        // matter how the subsequent install attempt ends.
        prop_assert!(
            result.stdout.contains(&format!("Switching node to version {version}")),
            "`use node <semver>` must announce the target version, got stdout: {}",
            result.stdout.chars().take(200).collect::<String>()
        );
    }

    /// Version aliases should work
    #[test]
    fn prop_version_aliases(
        alias in prop::sample::select(vec!["lts", "latest", "stable", "current", "lts/*", "lts/iron"])
    ) {
        let result = run_omg(&["use", "node", alias]);
        prop_assert!(!result.stderr.contains("panicked at"));
        // Aliases containing '/' are rejected by validate_version; every other
        // failure must still carry a diagnostic.
        if !result.success {
            prop_assert!(
                !result.stderr.is_empty(),
                "Rejected alias `{alias}` must produce an error message"
            );
        }
    }

    /// 'v'-prefixed versions should be normalized and handled
    #[test]
    fn prop_v_prefix_versions(major in 0u32..30, minor in 0u32..30, patch in 0u32..30) {
        let version = format!("v{major}.{minor}.{patch}");
        let result = run_omg(&["use", "node", &version]);
        prop_assert!(!result.stderr.contains("panicked at"));
        // The switch header (src/cli/runtimes.rs) echoes the version as given
        // — the 'v' prefix is stripped later, inside install_or_use.
        prop_assert!(
            result.stdout.contains(&format!("Switching node to version {version}")),
            "`use node {version}` must announce the target version, got stdout: {}",
            result.stdout.chars().take(200).collect::<String>()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FILE PATH PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// File paths should be handled safely
    #[test]
    fn prop_file_paths_safe(
        segments in prop::collection::vec("[a-zA-Z0-9_-]{1,20}", 1..10)
    ) {
        let path = segments.join("/");
        let project = TestProject::new();
        project.create_dir(&path);

        let result = run_omg_in_dir(&["status"], &project.path().join(&path));
        prop_assert!(!result.stderr.contains("panicked at"));
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
        prop_assert!(!result.stderr.contains("panicked at"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TOML PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Malformed TOML should not crash
    #[test]
    fn prop_malformed_toml_safe(content in ".*") {
        let project = TestProject::new();
        project.create_file("omg.lock", &content);

        let result = project.run(&["env", "check"]);
        // May fail, but should not panic
        prop_assert!(!result.stderr.contains("panicked at"));
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
        prop_assert!(!result.stderr.contains("panicked at"));
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
        prop_assert!(!result.stderr.contains("panicked at"));
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
        prop_assert!(!result.stderr.contains("panicked at"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCURRENT ACCESS PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]

    /// Concurrent reads should be safe
    #[test]
    fn prop_concurrent_reads_safe(thread_count in 2usize..10) {
        use std::thread;

        let handles: Vec<_> = (0..thread_count)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert!(!result.stderr.contains("panicked at"));
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
            prop_assert!(!result.stderr.contains("panicked at"));
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
        // Note: no null-byte case here - std::process::Command rejects null
        // bytes at spawn time (OS-level behavior), which would panic the test
        // harness itself rather than exercising omg.
        let test_args = vec![
            vec![""],
            vec!["a"],
            vec!["aaaa"],
            vec!["\n\r\t"],
            vec!["😀🔥"],
            vec!["--", "test"],
            vec!["-v", "-v", "-v"],
            vec!["search", "'; DROP TABLE"],
        ];

        for args in test_args {
            let result = run_omg(&args);
            assert!(
                !result.stderr.contains("panicked at"),
                "Panic with args: {args:?}"
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
                !result.stderr.contains("panicked at"),
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
                !result.stderr.contains("panicked at"),
                "Panic with version: {version}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE NAME PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Valid package names should be handled consistently
    #[test]
    fn prop_package_name_handling(
        name in "[a-z][a-z0-9-]{2,50}"
    ) {
        let result = run_omg(&["info", &name]);
        prop_assert!(!result.stderr.contains("panicked at"));
    }

    /// Package names with numbers should work
    #[test]
    fn prop_package_with_numbers(
        prefix in "[a-z]{2,10}",
        number in 0u32..100
    ) {
        let name = format!("{prefix}{number}");
        let result = run_omg(&["search", &name]);
        prop_assert!(!result.stderr.contains("panicked at"));
    }

    /// Package names with hyphens should work
    #[test]
    fn prop_package_with_hyphens(
        parts in prop::collection::vec("[a-z]{2,10}", 2..5)
    ) {
        let name = parts.join("-");
        let result = run_omg(&["info", &name]);
        prop_assert!(!result.stderr.contains("panicked at"));
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
        assert!(!result.stderr.contains("panicked at"));
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
        assert!(!result.stderr.contains("panicked at"));
    }

    #[test]
    fn regression_special_chars_in_path() {
        let project = TestProject::new();
        // create_dir uses create_dir_all().unwrap(), so it either succeeds or
        // fails the test outright; no exists() guard is needed.
        for special in &["test dir", "test'dir", "test\"dir", "test\\dir"] {
            project.create_dir(special);
            let result = run_omg_in_dir(&["status"], &project.path().join(special));
            assert!(
                !result.stderr.contains("panicked at"),
                "Panic with path: {special}"
            );
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
            assert!(!result.stderr.contains("panicked at"));
        }
    }
}
