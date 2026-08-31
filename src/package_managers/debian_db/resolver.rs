//! Pure Rust Dependency Resolver for Debian/Ubuntu
//!
//! Resolves package dependencies with:
//! - Debian version comparison (epoch:upstream-revision)
//! - Alternative dependency handling (`pkg1 | pkg2`)
//! - Topological ordering for the installation sequence
//!
//! Known limitations (fail explicitly rather than pretending):
//!
//! - `Provides:` virtual packages are NOT resolved;
//! - `Conflicts:`/`Breaks:` are NOT checked.
//!
//! Both require parsing fields the repository index does not currently
//! carry; see the `debian-pure` install flow for the resulting contract.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result};

use super::{DebianPackage, get_detailed_best_candidates, list_installed_fast};

/// A dependency specification parsed from the Depends: field
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Package name
    pub name: String,
    /// Version constraint (optional)
    pub version_constraint: Option<VersionConstraint>,
    /// Alternative dependencies (for "pkg1 | pkg2" syntax)
    pub alternatives: Vec<Dependency>,
}

/// Version constraint types
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionConstraint {
    /// Comparison operator
    pub op: VersionOp,
    /// Version string
    pub version: String,
}

/// Version comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOp {
    /// Exactly equal (=)
    Eq,
    /// Greater than or equal (>=)
    Ge,
    /// Greater than (>>)
    Gt,
    /// Less than or equal (<=)
    Le,
    /// Less than (<<)
    Lt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DependencyTarget {
    Installed,
    Available(String),
}

/// Result of dependency resolution
#[derive(Debug)]
pub struct ResolutionResult {
    /// Packages to install in order (dependencies first)
    pub to_install: Vec<String>,
    /// Packages that need upgrading
    pub to_upgrade: Vec<(String, String, String)>, // (name, old_version, new_version)
    /// Packages that would be removed due to conflicts
    pub to_remove: Vec<String>,
    /// Total download size in bytes
    pub download_size: u64,
    /// Total installed size in bytes
    pub installed_size: u64,
}

/// Dependency resolver
pub struct DependencyResolver {
    /// Available packages (from repository index)
    available: HashMap<String, DebianPackage>,
    /// Currently installed packages
    installed: HashMap<String, String>, // name -> version
    /// Packages selected for installation
    selected: HashSet<String>,
    /// Resolved dependency graph
    dep_graph: HashMap<String, HashSet<String>>,
}

impl DependencyResolver {
    /// Create a new resolver with current system state
    pub fn new() -> Result<Self> {
        let packages = get_detailed_best_candidates()?;
        let installed_list = list_installed_fast()?;

        let mut available = HashMap::with_capacity(packages.len());

        for pkg in packages {
            available.insert(pkg.name.clone(), pkg);
        }

        let installed: HashMap<String, String> = installed_list
            .into_iter()
            .map(|p| (p.name, p.version))
            .collect();

        Ok(Self {
            available,
            installed,
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        })
    }

    /// Add a package to resolve
    pub fn add_package(&mut self, name: &str) -> Result<()> {
        if !self.available.contains_key(name) {
            // Try to find similar package names for helpful suggestions
            let similar: Vec<_> = self
                .available
                .keys()
                .filter(|k| {
                    // Simple similarity check: starts with same prefix or contains substring
                    k.starts_with(similarity_prefix(name))
                        || k.contains(name)
                        || name.contains(k.as_str())
                })
                .take(5)
                .map(String::as_str)
                .collect();

            let error_message = if similar.is_empty() {
                format!(
                    "Package '{name}' not found in repositories.\n\
                    💡 Make sure you have:\n\
                    - Run 'omg sync' to refresh package database\n\
                    - Enabled the correct repositories\n\
                    - Spelled the package name correctly\n\
                    Try: omg search {name} to find available packages"
                )
            } else {
                format!(
                    "Package '{name}' not found in repositories.\n\
                    💡 Did you mean one of these?\n  - {}\n\
                    Try: omg search {name} to find more options",
                    similar.join("\n  - ")
                )
            };
            anyhow::bail!(error_message);
        }
        self.selected.insert(name.to_string());
        Ok(())
    }

    /// Resolve all selected packages and their dependencies
    /// Uses parallel resolution for multiple independent packages
    pub fn resolve(&mut self) -> Result<ResolutionResult> {
        use rayon::prelude::*;
        use rustc_hash::FxHashSet;

        let mut to_install = Vec::new();
        let mut to_upgrade = Vec::new();
        let mut download_size = 0u64;
        let mut installed_size = 0u64;

        // Process each selected package
        let selected: Vec<_> = self.selected.iter().cloned().collect();

        // For multiple independent packages, resolve initial dependencies in parallel
        if selected.len() > 1 {
            tracing::debug!("Resolving {} packages in parallel", selected.len());

            // First pass: resolve each selected package independently in parallel
            let parallel_results: Vec<_> = selected
                .par_iter()
                .map(|pkg_name| {
                    let mut local_visited = FxHashSet::default();
                    let mut local_to_install = Vec::new();

                    self.resolve_package(pkg_name, &mut local_visited, &mut local_to_install)?;
                    Ok::<_, anyhow::Error>(local_to_install)
                })
                .collect();

            // Merge results (deduplicate packages).
            let mut seen = FxHashSet::default();
            for result in parallel_results {
                for pkg in result? {
                    if seen.insert(pkg.clone()) {
                        to_install.push(pkg);
                    }
                }
            }
        } else {
            // Single package: use sequential resolution (no overhead).
            let mut local_visited = FxHashSet::default();
            for pkg_name in selected {
                self.resolve_package(&pkg_name, &mut local_visited, &mut to_install)?;
            }
        }

        // The shared read-only resolver cannot mutate `dep_graph`; record
        // edges once after either collection strategy completes.
        self.record_dependency_edges(&to_install)?;

        // Topologically sort the install order
        let sorted = self.topological_sort(&to_install)?;
        self.validate_projected_dependencies(&sorted)?;

        // Calculate sizes and determine upgrades
        let mut final_install = Vec::with_capacity(sorted.len());
        for name in sorted {
            if let Some(pkg) = self.available.get(&name) {
                if let Some(installed_ver) = self.installed.get(&name) {
                    // Package already installed - check if upgrade needed
                    if compare_versions(installed_ver, &pkg.version) == std::cmp::Ordering::Less {
                        to_upgrade.push((name.clone(), installed_ver.clone(), pkg.version.clone()));
                        download_size += pkg.size;
                        installed_size += pkg.installed_size;
                    }
                } else {
                    // New package
                    final_install.push(name.clone());
                    download_size += pkg.size;
                    installed_size += pkg.installed_size;
                }
            }
        }

        Ok(ResolutionResult {
            to_install: final_install,
            to_upgrade,
            // Future enhancement: Parse Conflicts/Breaks fields for automatic conflict resolution
            to_remove: Vec::new(),
            download_size,
            installed_size,
        })
    }

    /// Recursively resolve a package and its dependencies
    fn resolve_package(
        &self,
        name: &str,
        visited: &mut rustc_hash::FxHashSet<String>,
        to_install: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }
        visited.insert(name.to_string());

        let pkg = self
            .available
            .get(name)
            .with_context(|| format!("Package '{name}' not found in repository"))?;

        tracing::debug!(
            "Resolving package (parallel): {} (version: {})",
            name,
            pkg.version
        );

        // Parse and resolve dependencies
        for dep_str in &pkg.depends {
            let dep = parse_dependency(dep_str)
                .with_context(|| format!("Invalid dependency '{dep_str}' for package '{name}'"))?;
            match self.resolve_dependency(&dep, visited, to_install) {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!(
                        "Failed to resolve dependency '{}' for package '{}': {}",
                        dep_str,
                        name,
                        e
                    );
                    return Err(e).with_context(|| {
                        format!("Cannot satisfy dependency '{dep_str}' for package '{name}'")
                    });
                }
            }
        }

        // `visited` guarantees this package is added exactly once per
        // resolution walk, so no second linear membership scan is needed.
        to_install.push(name.to_string());

        Ok(())
    }

    /// Populate dependency edges for a fully resolved package closure.
    fn record_dependency_edges(&mut self, packages: &[String]) -> Result<()> {
        for name in packages {
            if self.dep_graph.contains_key(name) {
                continue;
            }
            if let Some(pkg) = self.available.get(name) {
                let mut dependencies = HashSet::new();
                for raw in &pkg.depends {
                    let dependency = parse_dependency(raw).with_context(|| {
                        format!("Invalid dependency '{raw}' for package '{name}'")
                    })?;
                    if let Some(DependencyTarget::Available(name)) =
                        self.dependency_target(&dependency)
                    {
                        dependencies.insert(name);
                    }
                }
                self.dep_graph.insert(name.clone(), dependencies);
            }
        }
        Ok(())
    }

    /// Resolve a single dependency
    fn resolve_dependency(
        &self,
        dep: &Dependency,
        visited: &mut rustc_hash::FxHashSet<String>,
        to_install: &mut Vec<String>,
    ) -> Result<()> {
        match self.dependency_target(dep) {
            Some(DependencyTarget::Installed) => return Ok(()),
            Some(DependencyTarget::Available(name)) => {
                return self.resolve_package(&name, visited, to_install);
            }
            None => {}
        }

        // Build helpful error message
        use std::fmt::Write;
        let mut error_msg = format!("Cannot satisfy dependency: {}", dep.name);
        if !dep.alternatives.is_empty() {
            let alt_names: Vec<_> = dep.alternatives.iter().map(|a| a.name.as_str()).collect();
            expect_infallible(write!(
                &mut error_msg,
                "\n  Tried alternatives: {}",
                alt_names.join(", ")
            ));
        }
        if let Some(constraint) = &dep.version_constraint {
            expect_infallible(write!(
                &mut error_msg,
                " (version: {:?} {})",
                constraint.op, constraint.version
            ));
        }
        error_msg.push_str(
            "\n💡 This dependency is not available in any enabled repository.\n\
            Try:\n\
            - omg sync (refresh package database)\n\
            - Check that required repositories are enabled\n\
            - Install the dependency manually first",
        );

        anyhow::bail!(error_msg)
    }

    /// Select the first satisfiable dependency using Debian alternative order.
    fn dependency_target(&self, dep: &Dependency) -> Option<DependencyTarget> {
        std::iter::once(dep)
            .chain(dep.alternatives.iter())
            .find_map(|candidate| {
                if self.installed.get(&candidate.name).is_some_and(|version| {
                    candidate
                        .version_constraint
                        .as_ref()
                        .is_none_or(|constraint| Self::version_satisfies(version, constraint))
                }) {
                    return Some(DependencyTarget::Installed);
                }

                self.available.get(&candidate.name).and_then(|package| {
                    candidate
                        .version_constraint
                        .as_ref()
                        .is_none_or(|constraint| {
                            Self::version_satisfies(&package.version, constraint)
                        })
                        .then(|| DependencyTarget::Available(candidate.name.clone()))
                })
            })
    }

    /// Re-check every package in the plan against the versions that the plan
    /// will leave installed. This prevents one requested upgrade from
    /// invalidating a dependency that was satisfied by the pre-transaction
    /// installed state during the initial walk.
    fn validate_projected_dependencies(&self, packages: &[String]) -> Result<()> {
        let mut projected = self.installed.clone();
        for name in packages {
            if let Some(package) = self.available.get(name) {
                projected.insert(name.clone(), package.version.clone());
            }
        }

        for name in packages {
            let Some(package) = self.available.get(name) else {
                continue;
            };
            for raw in &package.depends {
                let dependency = parse_dependency(raw)
                    .with_context(|| format!("Invalid dependency '{raw}' for package '{name}'"))?;
                let satisfied = std::iter::once(&dependency)
                    .chain(dependency.alternatives.iter())
                    .any(|candidate| {
                        projected.get(&candidate.name).is_some_and(|version| {
                            candidate
                                .version_constraint
                                .as_ref()
                                .is_none_or(|constraint| {
                                    Self::version_satisfies(version, constraint)
                                })
                        })
                    });
                anyhow::ensure!(
                    satisfied,
                    "Planned version changes leave dependency '{raw}' for package '{name}' unsatisfied"
                );
            }
        }
        Ok(())
    }

    /// Check if a version satisfies a constraint
    fn version_satisfies(version: &str, constraint: &VersionConstraint) -> bool {
        let cmp = compare_versions(version, &constraint.version);
        match constraint.op {
            VersionOp::Eq => cmp == std::cmp::Ordering::Equal,
            VersionOp::Ge => cmp != std::cmp::Ordering::Less,
            VersionOp::Gt => cmp == std::cmp::Ordering::Greater,
            VersionOp::Le => cmp != std::cmp::Ordering::Greater,
            VersionOp::Lt => cmp == std::cmp::Ordering::Less,
        }
    }

    /// Topologically sort packages (dependencies first)
    /// Uses Kahn's algorithm with optimized data structures
    fn topological_sort(&self, packages: &[String]) -> Result<Vec<String>> {
        use ahash::{AHashMap, AHashSet};

        let in_packages: AHashSet<&str> = packages.iter().map(String::as_str).collect();
        let mut in_degree: AHashMap<&str, usize> = in_packages
            .iter()
            .copied()
            .map(|package| (package, 0))
            .collect();
        let mut graph: AHashMap<&str, Vec<&str>> = AHashMap::with_capacity(packages.len());

        // Build reverse graph (package -> packages that depend on it)
        for package in packages {
            let package = package.as_str();
            if let Some(dependencies) = self.dep_graph.get(package) {
                for dependency in dependencies {
                    if let Some(dependency) = in_packages.get(dependency.as_str()) {
                        graph.entry(*dependency).or_default().push(package);
                        *in_degree.entry(package).or_default() += 1;
                    }
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter_map(|(&package, &degree)| (degree == 0).then_some(package))
            .collect();
        let mut result = Vec::with_capacity(packages.len());

        while let Some(package) = queue.pop_front() {
            result.push(package);

            if let Some(dependents) = graph.get(package) {
                for &dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }

        if result.len() != packages.len() {
            let ordered: AHashSet<&str> = result.iter().copied().collect();
            let missing: Vec<_> = packages
                .iter()
                .map(String::as_str)
                .filter(|package| !ordered.contains(package))
                .collect();

            tracing::error!("Circular dependency detected among packages: {:?}", missing);
            anyhow::bail!(
                "Circular dependency detected. Unable to determine installation order for: {}",
                missing.join(", ")
            );
        }

        tracing::debug!(
            "Topological sort complete: {} packages ordered",
            result.len()
        );
        Ok(result.into_iter().map(str::to_owned).collect())
    }
}

/// `write!` into a `String` cannot fail; surface the invariant explicitly.
fn expect_infallible(result: std::fmt::Result) {
    result.expect("invariant: write! to a String cannot fail");
}

/// First up-to-3 characters of `name`, safe on multi-byte UTF-8 input.
#[must_use]
fn similarity_prefix(name: &str) -> &str {
    let end = name
        .char_indices()
        .nth(3)
        .map_or(name.len(), |(idx, _)| idx);
    &name[..end]
}

/// Parse a dependency string like "libc6 (>= 2.38)"
fn parse_dependency(s: &str) -> Result<Dependency> {
    let mut parts = s.trim().split('|').map(str::trim);
    let mut dependency = parse_single_dep(parts.next().unwrap_or_default())?;
    dependency.alternatives = parts.map(parse_single_dep).collect::<Result<Vec<_>>>()?;
    Ok(dependency)
}

/// Parse a single dependency without alternatives
/// Strip architecture qualifier from package name (e.g., "perl:any" -> "perl")
/// Debian multi-arch qualifiers: `:any`, `:native`, `:amd64`, `:i386`, etc.
#[inline]
fn strip_arch_qualifier(name: &str) -> &str {
    // OPTIMIZATION: Use memchr for faster colon search (SIMD-accelerated)
    if let Some(colon_pos) = memchr::memchr(b':', name.as_bytes()) {
        &name[..colon_pos]
    } else {
        name
    }
}

#[inline]
fn parse_single_dep(s: &str) -> Result<Dependency> {
    // OPTIMIZATION: Use memchr for faster parenthesis search (SIMD)
    if let Some(paren_start) = memchr::memchr(b'(', s.as_bytes()) {
        let raw_name = s[..paren_start].trim();
        let name = strip_arch_qualifier(raw_name).to_string();
        anyhow::ensure!(!name.is_empty(), "dependency package name is empty");
        let constraint_start = paren_start + 1;
        let remainder = &s[constraint_start..];
        let relative_end = memchr::memchr(b')', remainder.as_bytes())
            .context("dependency version constraint is missing a closing parenthesis")?;
        let constraint_str = &remainder[..relative_end];
        let constraint = parse_version_constraint(constraint_str)?;
        return Ok(Dependency {
            name,
            version_constraint: Some(constraint),
            alternatives: Vec::new(),
        });
    }

    anyhow::ensure!(
        !s.contains(')'),
        "dependency contains a closing parenthesis without an opening parenthesis"
    );
    let name = strip_arch_qualifier(s.trim()).to_string();
    anyhow::ensure!(!name.is_empty(), "dependency package name is empty");
    Ok(Dependency {
        name,
        version_constraint: None,
        alternatives: Vec::new(),
    })
}

/// Parse a version constraint like `">= 2.38"`
/// OPTIMIZATION: Inline and use byte comparisons for faster parsing
#[inline]
fn parse_version_constraint(s: &str) -> Result<VersionConstraint> {
    let s = s.trim();
    let (op, version) = if let Some(version) = s.strip_prefix(">=") {
        (VersionOp::Ge, version)
    } else if let Some(version) = s.strip_prefix(">>") {
        (VersionOp::Gt, version)
    } else if let Some(version) = s.strip_prefix("<=") {
        (VersionOp::Le, version)
    } else if let Some(version) = s.strip_prefix("<<") {
        (VersionOp::Lt, version)
    } else if let Some(version) = s.strip_prefix('=') {
        (VersionOp::Eq, version)
    } else if let Some(version) = s.strip_prefix('>') {
        // Legacy dpkg syntax: `>` is an alias for `>=`.
        (VersionOp::Ge, version)
    } else if let Some(version) = s.strip_prefix('<') {
        // Legacy dpkg syntax: `<` is an alias for `<=`.
        (VersionOp::Le, version)
    } else {
        anyhow::bail!("unsupported dependency version constraint '{s}'");
    };
    let version = version.trim();
    anyhow::ensure!(!version.is_empty(), "dependency version is empty");
    Ok(VersionConstraint {
        op,
        version: version.to_string(),
    })
}

/// Compare two Debian version strings
///
/// Format: `[epoch:]upstream_version[-debian_revision]`
///
/// Lenient by design for comparator use: a malformed epoch (non-numeric or
/// overflowing `u32`) compares as epoch `0` rather than failing, matching
/// dpkg's tolerance of foreign version strings.
#[must_use]
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    crate::package_managers::types::compare_deb_versions(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_simple() {
        assert_eq!(compare_versions("1.0", "1.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.0", "2.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_versions("2.0", "1.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_with_epoch() {
        assert_eq!(
            compare_versions("1:1.0", "2.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("2.0", "1:1.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1:1.0", "1:1.0"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_versions_with_revision() {
        assert_eq!(compare_versions("1.0-1", "1.0-2"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1.0-2", "1.0-1"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_tilde() {
        // Tilde versions are considered earlier
        assert_eq!(
            compare_versions("1.0~beta", "1.0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("1.0", "1.0~beta"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_complex() {
        assert_eq!(
            compare_versions("2:1.0.5-1ubuntu1", "2:1.0.5-1ubuntu2"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("1.2.3-4", "1.2.3-4"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn malformed_parentheses_do_not_panic() {
        let parsed = std::panic::catch_unwind(|| parse_dependency("a)b (>= 1"));
        assert!(parsed.is_ok(), "malformed repository data must not panic");
        assert!(parsed.unwrap().is_err(), "malformed data must fail closed");
    }

    #[test]
    fn malformed_constraints_fail_closed() {
        assert!(parse_dependency("libc6 (!= 2.38)").is_err());
        assert!(parse_dependency("libc6 (>= )").is_err());
        assert!(parse_dependency(" | libc6").is_err());
    }

    #[test]
    fn legacy_constraint_operators_remain_supported() {
        let less = parse_dependency("libc6 (< 3.0)").unwrap();
        assert_eq!(less.version_constraint.unwrap().op, VersionOp::Le);
        let greater = parse_dependency("libc6 (> 2.0)").unwrap();
        assert_eq!(greater.version_constraint.unwrap().op, VersionOp::Ge);
    }

    #[test]
    fn test_parse_dependency_simple() {
        let dep = parse_dependency("libc6").unwrap();
        assert_eq!(dep.name, "libc6");
        assert!(dep.version_constraint.is_none());
    }

    #[test]
    fn test_parse_dependency_with_version() {
        let dep = parse_dependency("libc6 (>= 2.38)").unwrap();
        assert_eq!(dep.name, "libc6");
        assert!(dep.version_constraint.is_some());
        let vc = dep.version_constraint.unwrap();
        assert_eq!(vc.op, VersionOp::Ge);
        assert_eq!(vc.version, "2.38");
    }

    #[test]
    fn test_parse_dependency_alternatives() {
        let dep = parse_dependency("libssl1.1 | libssl3").unwrap();
        assert_eq!(dep.name, "libssl1.1");
        assert_eq!(dep.alternatives.len(), 1);
        assert_eq!(dep.alternatives[0].name, "libssl3");
    }

    #[test]
    fn test_parse_dependency_with_arch_qualifier() {
        // Test :any (multi-arch: allows any architecture)
        let dep = parse_dependency("perl:any").unwrap();
        assert_eq!(dep.name, "perl");
        assert!(dep.version_constraint.is_none());

        // Test :native (same as build architecture)
        let dep = parse_dependency("build-essential:native").unwrap();
        assert_eq!(dep.name, "build-essential");

        // Test with version constraint
        let dep = parse_dependency("libc6:amd64 (>= 2.38)").unwrap();
        assert_eq!(dep.name, "libc6");
        assert!(dep.version_constraint.is_some());
    }

    #[test]
    fn test_version_constraint_parsing() {
        let vc = parse_version_constraint(">= 2.38").unwrap();
        assert_eq!(vc.op, VersionOp::Ge);
        assert_eq!(vc.version, "2.38");

        let vc = parse_version_constraint("<< 3.0").unwrap();
        assert_eq!(vc.op, VersionOp::Lt);
        assert_eq!(vc.version, "3.0");
    }
    #[test]
    fn suggestions_handle_multibyte_names_without_panicking() {
        // Regression: byte-slicing at name.len().min(3) panicked on
        // multi-byte UTF-8 input whose 3rd byte split a character.
        let mut resolver = DependencyResolver {
            available: HashMap::new(),
            installed: HashMap::new(),
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        };
        let error = resolver
            .add_package("éé")
            .expect_err("unknown package must error, not panic");
        assert!(error.to_string().contains("not found"), "{error}");
        assert_eq!(similarity_prefix("éé"), "éé");
        assert_eq!(similarity_prefix("ééabc"), "ééa");
    }

    #[test]
    fn sequential_resolution_orders_dependencies_before_dependents() {
        let mut resolver = DependencyResolver {
            available: HashMap::from([
                ("base".to_string(), test_package("base", "1.0", &[])),
                ("pkg-a".to_string(), test_package("pkg-a", "1.0", &["base"])),
            ]),
            installed: HashMap::new(),
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        };
        resolver.add_package("pkg-a").expect("known package");

        let resolution = resolver.resolve().expect("resolvable");

        assert_eq!(resolution.to_install, ["base", "pkg-a"]);
    }

    #[test]
    fn resolved_alternative_is_ordered_before_its_dependent() {
        for _ in 0..64 {
            let mut resolver = DependencyResolver {
                available: HashMap::from([
                    (
                        "mail-transport-agent".to_string(),
                        test_package("mail-transport-agent", "1.0", &[]),
                    ),
                    (
                        "pkg-a".to_string(),
                        test_package(
                            "pkg-a",
                            "1.0",
                            &["default-mta (>= 1) | mail-transport-agent"],
                        ),
                    ),
                ]),
                installed: HashMap::new(),
                selected: HashSet::new(),
                dep_graph: HashMap::new(),
            };
            resolver.add_package("pkg-a").expect("known package");

            let resolution = resolver.resolve().expect("alternative is resolvable");
            let dependency = resolution
                .to_install
                .iter()
                .position(|name| name == "mail-transport-agent")
                .expect("resolved alternative in install set");
            let dependent = resolution
                .to_install
                .iter()
                .position(|name| name == "pkg-a")
                .expect("dependent in install set");
            assert!(
                dependency < dependent,
                "resolved alternative must be installed first: {:?}",
                resolution.to_install
            );
        }
    }

    #[test]
    fn unchosen_alternative_does_not_create_a_false_dependency_cycle() {
        let mut resolver = DependencyResolver {
            available: HashMap::from([
                (
                    "pkg-a".to_string(),
                    test_package("pkg-a", "1.0", &["pkg-b | pkg-c"]),
                ),
                ("pkg-b".to_string(), test_package("pkg-b", "1.0", &[])),
                (
                    "pkg-c".to_string(),
                    test_package("pkg-c", "1.0", &["pkg-a"]),
                ),
            ]),
            installed: HashMap::new(),
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        };
        resolver.add_package("pkg-a").expect("known package");
        resolver.add_package("pkg-c").expect("known package");

        let resolution = resolver
            .resolve()
            .expect("the unchosen pkg-c alternative must not form a cycle");
        let position = |name: &str| {
            resolution
                .to_install
                .iter()
                .position(|candidate| candidate == name)
                .expect("package in install set")
        };
        assert!(position("pkg-b") < position("pkg-a"));
        assert!(position("pkg-a") < position("pkg-c"));
    }

    #[test]
    fn planned_upgrade_cannot_break_another_planned_dependency() {
        let mut resolver = DependencyResolver {
            available: HashMap::from([
                ("shared".to_string(), test_package("shared", "3.0", &[])),
                (
                    "needs-new".to_string(),
                    test_package("needs-new", "1.0", &["shared (>= 2.0)"]),
                ),
                (
                    "needs-old".to_string(),
                    test_package("needs-old", "1.0", &["shared (< 2.0)"]),
                ),
            ]),
            installed: HashMap::from([("shared".to_string(), "1.0".to_string())]),
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        };
        resolver.add_package("needs-new").expect("known package");
        resolver.add_package("needs-old").expect("known package");

        let error = resolver
            .resolve()
            .expect_err("the projected upgrade would violate needs-old");
        assert!(
            error.to_string().contains("needs-old") && error.to_string().contains("shared (< 2.0)"),
            "{error:#}"
        );
    }

    #[test]
    fn parallel_resolution_orders_dependencies_before_dependents() {
        // Regression: the parallel (>1 package) pass never populated
        // `dep_graph`, so `topological_sort` emitted arbitrary order.
        let mut resolver = DependencyResolver {
            available: HashMap::from([
                ("base".to_string(), test_package("base", "1.0", &[])),
                ("pkg-a".to_string(), test_package("pkg-a", "1.0", &["base"])),
                ("pkg-b".to_string(), test_package("pkg-b", "1.0", &["base"])),
            ]),
            installed: HashMap::new(),
            selected: HashSet::new(),
            dep_graph: HashMap::new(),
        };
        resolver.add_package("pkg-a").expect("known package");
        resolver.add_package("pkg-b").expect("known package");

        let resolution = resolver.resolve().expect("resolvable");

        let base_pos = resolution
            .to_install
            .iter()
            .position(|name| name == "base")
            .expect("base in install set");
        let a_pos = resolution
            .to_install
            .iter()
            .position(|name| name == "pkg-a")
            .expect("pkg-a in install set");
        let b_pos = resolution
            .to_install
            .iter()
            .position(|name| name == "pkg-b")
            .expect("pkg-b in install set");
        assert!(
            base_pos < a_pos && base_pos < b_pos,
            "dependency-first order violated: {:?}",
            resolution.to_install
        );
    }

    fn test_package(name: &str, version: &str, depends: &[&str]) -> DebianPackage {
        DebianPackage {
            name: name.to_string(),
            version: version.to_string(),
            description: String::new(),
            section: String::new(),
            priority: String::new(),
            installed_size: 0,
            maintainer: String::new(),
            architecture: "amd64".to_string(),
            depends: depends.iter().map(|d| (*d).to_string()).collect(),
            filename: String::new(),
            size: 0,
            sha256: String::new(),
            homepage: String::new(),
            component: "main".to_string(),
            suite: String::new(),
        }
    }

    #[test]
    fn numeric_version_segments_beyond_u64_still_compare() {
        // "1" < huge (previously parsed as 0 and mis-ordered)
        assert_eq!(
            compare_versions("1.0-1", "1.99999999999999999999999999-1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("1.99999999999999999999999999-1", "1.0-1"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("1.0001-1", "1.1-1"),
            std::cmp::Ordering::Equal
        );
    }
}
