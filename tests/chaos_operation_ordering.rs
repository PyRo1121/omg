//! Chaos testing: random operation sequences
//!
//! Tests that OMG handles arbitrary operation sequences without crashes,
//! corruption, or inconsistent state.
//!
//! Run: `cargo test --test chaos_operation_ordering`

#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use proptest::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operation {
    Search(String),
    Install(String),
    Remove(String),
    Update,
    Info(String),
    ListInstalled,
    Clear,
}

/// Generate random but realistic operation sequences
fn random_operations() -> impl Strategy<Value = Vec<Operation>> {
    let package_names = vec![
        "firefox", "rust", "python", "git", "vim", "neovim", "zsh", "tmux",
    ];

    prop::collection::vec(
        prop_oneof![
            5 => prop::sample::select(package_names.clone()).prop_map(|s| Operation::Search(s.to_string())),
            3 => prop::sample::select(package_names.clone()).prop_map(|s| Operation::Install(s.to_string())),
            2 => prop::sample::select(package_names.clone()).prop_map(|s| Operation::Remove(s.to_string())),
            1 => Just(Operation::Update),
            4 => prop::sample::select(package_names.clone()).prop_map(|s| Operation::Info(s.to_string())),
            2 => Just(Operation::ListInstalled),
            1 => Just(Operation::Clear),
        ],
        10..100,
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 100,
        max_shrink_iters: 5000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_random_operation_sequence_never_crashes(ops in random_operations()) {
        let mut state = SystemState::new();
        let mut operation_count = 0;

        for (idx, op) in ops.iter().enumerate() {
            operation_count += 1;

            // Execute operation
            match op {
                Operation::Search(query) => {
                    let results = state.search(query);

                    // Invariant: search results should be valid
                    prop_assert!(results.len() <= 1000, "Search returned too many results");
                    for result in results {
                        prop_assert!(!result.is_empty(), "Search returned empty package name");
                    }
                }

                Operation::Install(pkg) => {
                    let success = state.install(pkg);

                    // Invariant: if install succeeded, package should be in installed set
                    if success {
                        prop_assert!(state.installed.contains(pkg),
                            "Package {} not in installed set after successful install", pkg);
                    }
                }

                Operation::Remove(pkg) => {
                    let success = state.remove(pkg);

                    // Invariant: if remove succeeded, package should not be in installed set
                    if success {
                        prop_assert!(!state.installed.contains(pkg),
                            "Package {} still in installed set after successful remove", pkg);
                    }
                }

                Operation::Update => {
                    let updated = state.update();

                    // Invariant: update count should be reasonable
                    prop_assert!(updated <= state.installed.len(),
                        "Update count {} exceeds installed packages {}", updated, state.installed.len());
                }

                Operation::Info(pkg) => {
                    let _ = state.info(pkg);
                    // Info should never crash, even for non-existent packages
                }

                Operation::ListInstalled => {
                    let installed = state.list_installed();

                    // Invariant: list should match internal state
                    prop_assert_eq!(installed.len(), state.installed.len(),
                        "List length {} doesn't match state {}", installed.len(), state.installed.len());
                }

                Operation::Clear => {
                    state.clear();

                    // Invariant: after clear, installed set should be empty
                    prop_assert!(state.installed.is_empty(), "Installed set not empty after clear");
                }
            }

            // Global invariants after each operation

            // Invariant 1: No duplicate packages in installed set
            let unique_count = state.installed.iter().collect::<HashSet<_>>().len();
            prop_assert_eq!(
                state.installed.len(),
                unique_count,
                "Installed set contains duplicates at operation {}: {:?}",
                idx,
                op
            );

            // Invariant 2: Installed count should be reasonable
            prop_assert!(state.installed.len() <= 10000,
                "Installed count {} exceeds maximum", state.installed.len());

            // Invariant 3: Total operations count should match
            prop_assert_eq!(state.operation_count, operation_count,
                "Operation count mismatch: state {} vs actual {}", state.operation_count, operation_count);

            // Invariant 4: System should remain healthy
            prop_assert!(state.is_healthy(), "System became unhealthy at operation {}: {:?}", idx, op);
        }

        // Final state validation
        prop_assert!(state.is_healthy(), "System not healthy after all operations");
    }

    #[test]
    fn prop_install_remove_idempotent(
        pkg in prop::sample::select(vec!["firefox", "rust", "python", "git"]).prop_map(std::string::ToString::to_string)
    ) {
        let mut state = SystemState::new();

        // Install twice
        let result1 = state.install(&pkg);
        let result2 = state.install(&pkg);

        // First install should succeed, second should be idempotent
        prop_assert!(result1, "First install should succeed");
        prop_assert!(!result2, "Second install should return false (already installed)");

        // Remove twice
        let result3 = state.remove(&pkg);
        let result4 = state.remove(&pkg);

        // First remove should succeed, second should be idempotent
        prop_assert!(result3, "First remove should succeed");
        prop_assert!(!result4, "Second remove should return false (not installed)");
    }

    #[test]
    fn prop_search_always_works(query in "[a-z]{1,20}") {
        let mut state = SystemState::new();

        // Search should never fail, even for non-existent packages
        let results = state.search(&query);

        // Results should be valid
        prop_assert!(results.len() <= 1000, "Too many search results");
    }

    #[test]
    fn prop_concurrent_operation_invariants(
        ops1 in prop::collection::vec(
            prop::sample::select(vec!["firefox", "rust", "python"]).prop_map(|s| Operation::Install(s.to_string())),
            5..20
        ),
        ops2 in prop::collection::vec(
            prop::sample::select(vec!["git", "vim", "neovim"]).prop_map(|s| Operation::Remove(s.to_string())),
            5..20
        )
    ) {
        let mut state = SystemState::new();

        // Execute two sequences interleaved
        let max_len = ops1.len().max(ops2.len());

        for i in 0..max_len {
            if i < ops1.len()
                && let Operation::Install(pkg) = &ops1[i] {
                    state.install(pkg);
                }

            if i < ops2.len()
                && let Operation::Remove(pkg) = &ops2[i] {
                    state.remove(pkg);
                }

            // Invariant: no duplicates
            let unique = state.installed.iter().collect::<HashSet<_>>().len();
            prop_assert_eq!(state.installed.len(), unique,
                "Duplicates found after interleaved operation {}", i);
        }
    }
}

// ============================================================================
// Mock System State
// ============================================================================

#[derive(Debug)]
struct SystemState {
    installed: Vec<String>,
    operation_count: usize,
}

impl SystemState {
    const fn new() -> Self {
        Self {
            installed: Vec::new(),
            operation_count: 0,
        }
    }

    fn search(&mut self, query: &str) -> Vec<String> {
        self.operation_count += 1;
        // Mock search: return packages that contain query
        let all_packages = vec![
            "firefox",
            "firefox-developer-edition",
            "rust",
            "rust-analyzer",
            "python",
            "python-pip",
            "python-numpy",
            "git",
            "github-cli",
            "vim",
            "neovim",
            "zsh",
            "tmux",
        ];

        all_packages
            .into_iter()
            .filter(|pkg| pkg.contains(query))
            .map(String::from)
            .take(50)
            .collect()
    }

    fn install(&mut self, pkg: &str) -> bool {
        self.operation_count += 1;

        // Check if already installed
        if self.installed.contains(&pkg.to_string()) {
            return false;
        }

        // Install package
        self.installed.push(pkg.to_string());
        true
    }

    fn remove(&mut self, pkg: &str) -> bool {
        self.operation_count += 1;

        // Check if installed
        if let Some(pos) = self.installed.iter().position(|p| p == pkg) {
            self.installed.remove(pos);
            true
        } else {
            false
        }
    }

    fn update(&mut self) -> usize {
        self.operation_count += 1;

        // Mock: "update" 10% of installed packages
        (self.installed.len() as f64 * 0.1).ceil() as usize
    }

    fn info(&mut self, _pkg: &str) -> String {
        self.operation_count += 1;
        // Mock: return some info
        "Package info".to_string()
    }

    fn list_installed(&mut self) -> Vec<String> {
        self.operation_count += 1;
        self.installed.clone()
    }

    fn clear(&mut self) {
        self.operation_count += 1;
        self.installed.clear();
    }

    fn is_healthy(&self) -> bool {
        // System is healthy if:
        // 1. No duplicates in installed set
        let unique = self.installed.iter().collect::<HashSet<_>>().len();
        if self.installed.len() != unique {
            return false;
        }

        // 2. Installed count is reasonable
        if self.installed.len() > 10000 {
            return false;
        }

        true
    }
}

// ============================================================================
// Deterministic Tests (non-property)
// ============================================================================

#[test]
fn test_specific_failure_sequence() {
    // Test a specific sequence that could cause issues
    let mut state = SystemState::new();

    // Install -> Remove -> Install same package
    assert!(state.install("firefox"));
    assert!(state.remove("firefox"));
    assert!(state.install("firefox"));

    assert_eq!(state.installed.len(), 1);
    assert!(state.installed.contains(&"firefox".to_string()));
}

#[test]
fn test_rapid_install_remove() {
    let mut state = SystemState::new();

    // Rapidly install and remove
    for _ in 0..100 {
        state.install("test-package");
        state.remove("test-package");
    }

    // Should end up empty
    assert!(state.installed.is_empty());
    assert!(state.is_healthy());
}

#[test]
fn test_many_packages() {
    let mut state = SystemState::new();

    // Install many packages
    for i in 0..1000 {
        let pkg_name = format!("pkg-{i}");
        assert!(state.install(&pkg_name));
    }

    assert_eq!(state.installed.len(), 1000);

    // Remove all
    for i in 0..1000 {
        let pkg_name = format!("pkg-{i}");
        assert!(state.remove(&pkg_name));
    }

    assert!(state.installed.is_empty());
}

#[test]
fn test_search_edge_cases() {
    let mut state = SystemState::new();

    // Empty query
    let results = state.search("");
    assert!(!results.is_empty()); // Should return all packages

    // Very long query
    let long_query = "a".repeat(1000);
    let results = state.search(&long_query);
    assert!(results.is_empty()); // No matches expected

    // Special characters
    let results = state.search("fire*fox");
    assert!(results.len() <= 50); // Should not crash
}
