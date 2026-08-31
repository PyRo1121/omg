#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! AUR dependency-graph and build-wave planning contracts.
//!
//! Run real AUR scenarios with both `OMG_RUN_SYSTEM_TESTS=1` and
//! `OMG_RUN_NETWORK_TESTS=1`.

#![cfg(all(feature = "arch", target_os = "linux"))]

pub mod common;

use common::*;
use std::collections::{HashMap, HashSet};
use std::process::Command;

// Import the parallel build types directly
use omg_lib::package_managers::aur::{BuildJob, ParallelBuilder};

// ============================================================================
// DEPENDENCY GRAPH CONSTRUCTION TESTS
// ============================================================================

#[test]
fn test_depgraph_empty_dependencies() {
    println!("\n=== Test: Empty Dependencies ===");

    let jobs = vec![
        BuildJob::new("pkg1".to_string(), vec![]),
        BuildJob::new("pkg2".to_string(), vec![]),
        BuildJob::new("pkg3".to_string(), vec![]),
    ];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);

    assert_eq!(graph.len(), 3);
    assert_eq!(graph.get("pkg1").unwrap().len(), 0);
    assert_eq!(graph.get("pkg2").unwrap().len(), 0);
    assert_eq!(graph.get("pkg3").unwrap().len(), 0);

    println!("✓ Empty dependencies handled correctly");
}

#[test]
fn test_depgraph_linear_chain() {
    println!("\n=== Test: Linear Dependency Chain ===");

    let jobs = vec![
        BuildJob::new("base".to_string(), vec![]),
        BuildJob::new("middle".to_string(), vec!["base".to_string()]),
        BuildJob::new("top".to_string(), vec!["middle".to_string()]),
    ];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);

    assert_eq!(graph.get("base").unwrap().len(), 0);
    assert!(graph.get("middle").unwrap().contains("base"));
    assert!(graph.get("top").unwrap().contains("middle"));

    println!("✓ Linear chain constructed correctly");
}

#[test]
fn test_depgraph_diamond_dependency() {
    println!("\n=== Test: Diamond Dependency Pattern ===");

    //     top
    //    /   \
    //   left right
    //    \   /
    //    base

    let jobs = vec![
        BuildJob::new("base".to_string(), vec![]),
        BuildJob::new("left".to_string(), vec!["base".to_string()]),
        BuildJob::new("right".to_string(), vec!["base".to_string()]),
        BuildJob::new(
            "top".to_string(),
            vec!["left".to_string(), "right".to_string()],
        ),
    ];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);

    assert_eq!(graph.get("base").unwrap().len(), 0);
    assert!(graph.get("left").unwrap().contains("base"));
    assert!(graph.get("right").unwrap().contains("base"));
    assert!(graph.get("top").unwrap().contains("left"));
    assert!(graph.get("top").unwrap().contains("right"));

    println!("✓ Diamond dependency handled correctly");
}

#[test]
fn test_depgraph_mixed_internal_external() {
    println!("\n=== Test: Mixed Internal/External Dependencies ===");

    // Some dependencies are in the build set, some are external (filtered out)
    let jobs = vec![
        BuildJob::new("pkg-a".to_string(), vec!["external-lib".to_string()]),
        BuildJob::new(
            "pkg-b".to_string(),
            vec!["pkg-a".to_string(), "system-lib".to_string()],
        ),
    ];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);

    // External deps should be filtered out
    assert_eq!(graph.get("pkg-a").unwrap().len(), 0);
    assert!(graph.get("pkg-b").unwrap().contains("pkg-a"));
    assert!(!graph.get("pkg-b").unwrap().contains("system-lib"));

    println!("✓ External dependencies filtered correctly");
}

// ============================================================================
// TOPOLOGICAL SORT TESTS
// ============================================================================

#[test]
fn test_toposort_simple_sequence() {
    println!("\n=== Test: Topological Sort - Simple Sequence ===");

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), HashSet::new());
    graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());
    graph.insert("c".to_string(), ["b".to_string()].into_iter().collect());

    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], vec!["a"]);
    assert_eq!(levels[1], vec!["b"]);
    assert_eq!(levels[2], vec!["c"]);

    println!("✓ Simple sequence sorted correctly");
}

#[test]
fn test_toposort_parallel_independent() {
    println!("\n=== Test: Topological Sort - Parallel Independent ===");

    let mut graph = HashMap::new();
    graph.insert("pkg1".to_string(), HashSet::new());
    graph.insert("pkg2".to_string(), HashSet::new());
    graph.insert("pkg3".to_string(), HashSet::new());

    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    // All should be in same level since independent
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].len(), 3);

    println!("✓ Independent packages in same level");
}

#[test]
fn test_toposort_diamond_pattern() {
    println!("\n=== Test: Topological Sort - Diamond Pattern ===");

    let mut graph = HashMap::new();
    graph.insert("base".to_string(), HashSet::new());
    graph.insert(
        "left".to_string(),
        ["base".to_string()].into_iter().collect(),
    );
    graph.insert(
        "right".to_string(),
        ["base".to_string()].into_iter().collect(),
    );
    graph.insert(
        "top".to_string(),
        ["left".to_string(), "right".to_string()]
            .into_iter()
            .collect(),
    );

    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], vec!["base"]);
    assert_eq!(levels[1].len(), 2); // left and right can be parallel
    assert!(levels[1].contains(&"left".to_string()));
    assert!(levels[1].contains(&"right".to_string()));
    assert_eq!(levels[2], vec!["top"]);

    println!("✓ Diamond pattern sorted correctly with parallelism");
}

#[test]
fn test_toposort_complex_graph() {
    println!("\n=== Test: Topological Sort - Complex Graph ===");

    //       h
    //      / \
    //     f   g
    //    / \ /
    //   d   e
    //    \ /
    //     c
    //     |
    //     b
    //     |
    //     a

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), HashSet::new());
    graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());
    graph.insert("c".to_string(), ["b".to_string()].into_iter().collect());
    graph.insert("d".to_string(), ["c".to_string()].into_iter().collect());
    graph.insert("e".to_string(), ["c".to_string()].into_iter().collect());
    graph.insert(
        "f".to_string(),
        ["d".to_string(), "e".to_string()].into_iter().collect(),
    );
    graph.insert("g".to_string(), ["e".to_string()].into_iter().collect());
    graph.insert(
        "h".to_string(),
        ["f".to_string(), "g".to_string()].into_iter().collect(),
    );

    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    // Verify correct ordering
    assert_eq!(levels[0], vec!["a"]);
    assert_eq!(levels[1], vec!["b"]);
    assert_eq!(levels[2], vec!["c"]);
    // d and e can be parallel
    assert_eq!(levels[3].len(), 2);
    // f and g in different levels or same depending on completion
    assert!(levels.len() >= 5);

    println!("✓ Complex graph sorted correctly");
}

// ============================================================================
// CIRCULAR DEPENDENCY DETECTION TESTS
// ============================================================================

#[test]
fn test_circular_simple_cycle() {
    println!("\n=== Test: Circular Dependency - Simple Cycle ===");

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), ["b".to_string()].into_iter().collect());
    graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());

    let result = ParallelBuilder::topological_levels(&graph);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Circular"));

    println!("✓ Simple cycle detected");
}

#[test]
fn test_circular_three_node_cycle() {
    println!("\n=== Test: Circular Dependency - Three Node Cycle ===");

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), ["b".to_string()].into_iter().collect());
    graph.insert("b".to_string(), ["c".to_string()].into_iter().collect());
    graph.insert("c".to_string(), ["a".to_string()].into_iter().collect());

    let result = ParallelBuilder::topological_levels(&graph);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Circular"));

    println!("✓ Three-node cycle detected");
}

#[test]
fn test_circular_self_dependency() {
    println!("\n=== Test: Circular Dependency - Self Loop ===");

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), ["a".to_string()].into_iter().collect());

    let result = ParallelBuilder::topological_levels(&graph);

    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Circular"),
        "self-dependency must be reported as a circular dependency"
    );

    println!("✓ Self-dependency detected");
}

#[test]
fn test_circular_complex_with_cycle() {
    println!("\n=== Test: Circular Dependency - Complex Graph with Cycle ===");

    // Valid deps plus a cycle
    let mut graph = HashMap::new();
    graph.insert("a".to_string(), HashSet::new());
    graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());
    graph.insert(
        "c".to_string(),
        ["b".to_string(), "d".to_string()].into_iter().collect(),
    );
    graph.insert("d".to_string(), ["c".to_string()].into_iter().collect()); // c -> d -> c cycle

    let result = ParallelBuilder::topological_levels(&graph);

    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Circular"),
        "cycle embedded in a larger graph must be reported as circular"
    );

    println!("✓ Cycle in complex graph detected");
}

// ============================================================================
// PARALLEL BUILD OPTIMIZATION TESTS
// ============================================================================

#[test]
fn test_parallel_maximum_parallelism() {
    println!("\n=== Test: Maximum Parallelism ===");

    // 10 independent packages should all be in level 0
    let jobs: Vec<BuildJob> = (0..10)
        .map(|i| BuildJob::new(format!("pkg-{}", i), vec![]))
        .collect();

    let graph = ParallelBuilder::build_dependency_graph(&jobs);
    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].len(), 10);

    println!("✓ Maximum parallelism achieved for independent packages");
}

#[test]
fn test_parallel_minimum_parallelism() {
    println!("\n=== Test: Minimum Parallelism (Sequential) ===");

    // Linear chain: each depends on previous
    let jobs: Vec<BuildJob> = (0..5)
        .map(|i| {
            if i == 0 {
                BuildJob::new(format!("pkg-{}", i), vec![])
            } else {
                BuildJob::new(format!("pkg-{}", i), vec![format!("pkg-{}", i - 1)])
            }
        })
        .collect();

    let graph = ParallelBuilder::build_dependency_graph(&jobs);
    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    // Should be 5 levels (one for each package)
    assert_eq!(levels.len(), 5);
    for level in &levels {
        assert_eq!(level.len(), 1); // Only one package per level
    }

    println!("✓ Sequential build for linear dependencies");
}

#[test]
fn test_parallel_mixed_parallelism() {
    println!("\n=== Test: Mixed Parallelism ===");

    //    pkg5   pkg6
    //      \   /
    //      pkg4
    //      /  \
    //   pkg2  pkg3
    //      \  /
    //      pkg1

    let jobs = vec![
        BuildJob::new("pkg1".to_string(), vec![]),
        BuildJob::new("pkg2".to_string(), vec!["pkg1".to_string()]),
        BuildJob::new("pkg3".to_string(), vec!["pkg1".to_string()]),
        BuildJob::new(
            "pkg4".to_string(),
            vec!["pkg2".to_string(), "pkg3".to_string()],
        ),
        BuildJob::new("pkg5".to_string(), vec!["pkg4".to_string()]),
        BuildJob::new("pkg6".to_string(), vec!["pkg4".to_string()]),
    ];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);
    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 4);
    assert_eq!(levels[0], vec!["pkg1"]);
    assert_eq!(levels[1].len(), 2); // pkg2 and pkg3 parallel
    assert_eq!(levels[2], vec!["pkg4"]);
    assert_eq!(levels[3].len(), 2); // pkg5 and pkg6 parallel

    println!("✓ Mixed parallelism optimized correctly");
}

// ============================================================================
// REAL-WORLD DEPENDENCY SCENARIOS
// ============================================================================

#[test]
fn test_realworld_aur_helper_dependencies() {
    require_system_tests!();
    require_network_tests!();
    require_arch!();

    println!("\n=== Test: Real-World AUR Helper Dependencies ===");

    // yay depends on pacman, go (for building from source), etc.
    let package = "yay-bin";

    cleanup_package(package);

    let result = run_omg(&["install", package]);
    result.assert_success();

    // Verify all dependencies were resolved
    let info = Command::new("pacman")
        .args(["-Qi", package])
        .output()
        .expect("Failed to query package");

    let output = String::from_utf8_lossy(&info.stdout);
    assert!(output.contains("Depends On"));

    println!("✓ Real-world dependencies resolved");
}

#[test]
fn test_realworld_optional_dependencies_metadata() {
    require_system_tests!();
    require_network_tests!();
    require_arch!();

    println!("\n=== Test: Optional Dependencies Handling ===");

    let package = "yay-bin";

    cleanup_package(package);

    let result = run_omg(&["install", package]);
    result.assert_success();

    // Contract: after installation, pacman's local database exposes the AUR
    // package's optional-dependency metadata rather than dropping the field.
    let info = Command::new("pacman")
        .args(["-Qi", package])
        .output()
        .expect("Failed to query package");

    let output = String::from_utf8_lossy(&info.stdout);
    assert!(
        output.contains("Optional Deps"),
        "pacman -Qi {package} must list the 'Optional Deps' field:\n{output}"
    );

    println!("✓ Optional dependencies metadata present");
}

// ============================================================================
// EDGE CASES & ERROR CONDITIONS
// ============================================================================

#[test]
fn test_edge_empty_graph() {
    println!("\n=== Test: Edge Case - Empty Graph ===");

    let graph: HashMap<String, HashSet<String>> = HashMap::new();
    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 0);

    println!("✓ Empty graph handled");
}

#[test]
fn test_edge_single_package() {
    println!("\n=== Test: Edge Case - Single Package ===");

    let mut graph = HashMap::new();
    graph.insert("only-package".to_string(), HashSet::new());

    let levels = ParallelBuilder::topological_levels(&graph).unwrap();

    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0], vec!["only-package"]);

    println!("✓ Single package handled");
}

#[test]
fn test_edge_duplicate_dependencies() {
    println!("\n=== Test: Edge Case - Duplicate Dependencies ===");

    let jobs = vec![BuildJob::new(
        "pkg".to_string(),
        vec![
            "dep1".to_string(),
            "dep1".to_string(),
            "dep2".to_string(),
            "dep2".to_string(),
        ],
    )];

    let graph = ParallelBuilder::build_dependency_graph(&jobs);

    // Should deduplicate
    assert!(graph.get("pkg").unwrap().len() <= 2);

    println!("✓ Duplicate dependencies handled");
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn cleanup_package(package: &str) {
    use std::fs;

    let cache_dir = omg_lib::core::paths::cache_dir().join("aur").join(package);

    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).expect("Failed to clear the AUR package cache");
    }

    let _ = Command::new("sudo")
        .args(["pacman", "-Rns", "--noconfirm", package])
        .output();
}
