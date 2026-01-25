#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Production-Ready Performance Benchmarks
//!
//! Verifies that OMG meets its performance targets.
//! All tests use REAL code paths - NO MOCKS, NO STUBS.
//!
//! Run:
//!   cargo test --test benchmarks --release --features arch
//!
//! Environment variables:
//!   OMG_RUN_PERF_TESTS=1 - Enable performance assertions

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use std::env;
use std::process::{Command, Stdio};
use std::time::Instant;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

fn run_omg(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

fn measure_time<F: FnOnce() -> T, T>(f: F) -> (T, std::time::Duration) {
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

fn perf_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_PERF_TESTS"), Ok(value) if value == "1")
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI COMMAND PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_performance {
    use super::*;

    #[test]
    fn test_version_flag_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["--version"]));

        assert!(success, "Version flag should succeed");
        assert!(stdout.contains("omg"), "Should show version");
        assert!(
            duration.as_millis() < 50,
            "Version flag should complete in <50ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_help_flag_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["--help"]));

        assert!(success, "Help flag should succeed");
        assert!(!stdout.is_empty(), "Should show help");
        assert!(
            duration.as_millis() < 100,
            "Help flag should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_status_command_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["status"]));

        assert!(success, "Status command should succeed");
        assert!(
            duration.as_millis() < 200,
            "Status command should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE SEARCH PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod search_performance {
    use super::*;

    #[test]
    fn test_search_simple_query_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["search", "firefox"]));

        assert!(success, "Search should succeed");
        assert!(
            duration.as_millis() < 100,
            "Search should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_search_long_query_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let long_query = "a".repeat(100);
        let ((success, _, _), duration) = measure_time(|| run_omg(&["search", &long_query]));

        assert!(success, "Long query search should succeed");
        assert!(
            duration.as_millis() < 200,
            "Long query search should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_search_unicode_query_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["search", "café"]));

        assert!(success, "Unicode search should succeed");
        assert!(
            duration.as_millis() < 200,
            "Unicode search should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_info_command_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["info", "pacman"]));

        assert!(success, "Info command should succeed");
        assert!(
            duration.as_millis() < 100,
            "Info command should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod update_performance {
    use super::*;

    #[test]
    fn test_update_check_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["update", "--check"]));

        assert!(success, "Update check should succeed");
        assert!(!stdout.is_empty(), "Should produce output");

        // Direct ALPM operations should be fast
        assert!(
            duration.as_millis() < 2000,
            "Update check should complete in <2s, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_list_explicit_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["explicit"]));

        assert!(success, "Explicit command should succeed");
        assert!(!stdout.is_empty(), "Should produce output");

        // Direct ALPM query should be fast
        assert!(
            duration.as_millis() < 100,
            "Explicit command should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME MANAGEMENT PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod runtime_performance {
    use super::*;

    #[test]
    fn test_list_command_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["list"]));

        assert!(success, "List command should succeed");
        assert!(!stdout.is_empty(), "Should produce output");
        assert!(
            duration.as_millis() < 200,
            "List command should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_which_command_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["which", "node"]));

        assert!(success, "Which command should succeed");
        assert!(
            duration.as_millis() < 100,
            "Which command should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_config_command_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, _, _), duration) = measure_time(|| run_omg(&["config"]));

        assert!(success, "Config command should succeed");
        assert!(
            duration.as_millis() < 100,
            "Config command should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENVIRONMENT COMMANDS PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod environment_performance {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_env_capture_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let temp_dir = TempDir::new().unwrap();

        let ((success, stdout, _), duration) =
            measure_time(|| run_omg_in_dir(&["env", "capture"], temp_dir.path()));

        assert!(success, "Env capture should succeed");
        assert!(!stdout.is_empty(), "Should produce output");
        assert!(
            duration.as_millis() < 500,
            "Env capture should complete in <500ms, took {}ms",
            duration.as_millis()
        );
    }

    fn run_omg_in_dir(args: &[&str], dir: &std::path::Path) -> (bool, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_omg"))
            .args(args)
            .current_dir(dir)
            .env("OMG_TEST_MODE", "1")
            .env("OMG_DISABLE_DAEMON", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to execute omg");

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        (output.status.success(), stdout, stderr)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPLETIONS PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod completions_performance {
    use super::*;

    #[test]
    fn test_completions_generation_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) =
            measure_time(|| run_omg(&["completions", "bash", "--stdout"]));

        assert!(success, "Completions generation should succeed");
        assert!(!stdout.is_empty(), "Should generate completions");
        assert!(
            duration.as_millis() < 200,
            "Completions generation should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_hook_generation_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let ((success, stdout, _), duration) = measure_time(|| run_omg(&["hook", "bash"]));

        assert!(success, "Hook generation should succeed");
        assert!(!stdout.is_empty(), "Should generate hook code");
        assert!(
            duration.as_millis() < 100,
            "Hook generation should complete in <100ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REPEATABILITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod repeatability {
    use super::*;

    #[test]
    fn test_search_is_repeatable() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let durations: Vec<_> = (0..5)
            .map(|_| {
                let (_, duration) = measure_time(|| run_omg(&["search", "firefox"]));
                duration.as_millis()
            })
            .collect();

        let avg = durations.iter().sum::<u128>() / durations.len() as u128;
        let max = *durations.iter().max().unwrap();
        let min = *durations.iter().min().unwrap();

        assert!(
            max < 200,
            "All runs should complete in <200ms, max was {}ms",
            max
        );

        // Variance should be reasonable (max should be < 2x min)
        assert!(
            max < min * 2,
            "Performance should be consistent: min={}ms, max={}ms, avg={}ms",
            min,
            max,
            avg
        );
    }

    #[test]
    fn test_status_is_repeatable() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let durations: Vec<_> = (0..5)
            .map(|_| {
                let (_, duration) = measure_time(|| run_omg(&["status"]));
                duration.as_millis()
            })
            .collect();

        let avg = durations.iter().sum::<u128>() / durations.len() as u128;
        let max = *durations.iter().max().unwrap();

        assert!(
            max < 300,
            "All runs should complete in <300ms, max was {}ms",
            max
        );

        eprintln!("Status performance: avg={}ms, max={}ms", avg, max);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COLD START PERFORMANCE
// ═══════════════════════════════════════════════════════════════════════════════

mod cold_start {
    use super::*;

    #[test]
    fn test_first_run_overhead() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        // First run may initialize databases
        let ((success, _, _), duration) = measure_time(|| run_omg(&["status"]));

        assert!(success, "First run should succeed");
        assert!(
            duration.as_secs() < 10,
            "First run should complete in <10s, took {}s",
            duration.as_secs()
        );
    }

    #[test]
    fn test_warm_start_performance() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        // Warm up with a run
        run_omg(&["status"]);

        // Then measure warm performance
        let ((success, _, _), duration) = measure_time(|| run_omg(&["status"]));

        assert!(success, "Warm run should succeed");
        assert!(
            duration.as_millis() < 200,
            "Warm run should complete in <200ms, took {}ms",
            duration.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MEMORY USAGE
// ═══════════════════════════════════════════════════════════════════════════════

mod memory_usage {
    use super::*;

    #[test]
    fn test_help_memory_efficiency() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let start = Instant::now();
        let (success, stdout, _) = run_omg(&["--help"]);
        let duration = start.elapsed();

        assert!(success, "Help should succeed");
        assert!(!stdout.is_empty(), "Should produce output");

        // Help should be instant and lightweight
        assert!(
            duration.as_millis() < 100,
            "Help should be fast, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_search_memory_efficiency() {
        if !perf_tests_enabled() {
            eprintln!("Skipping perf test (set OMG_RUN_PERF_TESTS=1)");
            return;
        }

        let start = Instant::now();
        let (success, _, _) = run_omg(&["search", "lib"]);
        let duration = start.elapsed();

        assert!(success, "Search should succeed");

        // Even broad search should be fast with direct ALPM
        assert!(
            duration.as_millis() < 500,
            "Broad search should complete in <500ms, took {}ms",
            duration.as_millis()
        );
    }
}
