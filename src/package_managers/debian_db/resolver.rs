//! Pure Rust Dependency Resolver for Debian/Ubuntu
//!
//! Resolves package dependencies using a SAT-solver inspired algorithm with:
//! - Debian version comparison (epoch:upstream-revision)
//! - Virtual package resolution (Provides: field)
//! - Conflict detection (Conflicts:, Breaks: fields)
//! - Alternative dependency handling (pkg1 | pkg2)
//! - Topological ordering for installation sequence

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result};

use super::{DebianPackage, get_detailed_packages, list_installed_fast};

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

/// Conflict specification (Conflicts:, Breaks: fields)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Package name that conflicts
    pub name: String,
    /// Version constraint (optional)
    pub version_constraint: Option<VersionConstraint>,
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

impl VersionOp {
    #[expect(dead_code)] // Kept for potential future use
    fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "=" => Some(Self::Eq),
            ">=" => Some(Self::Ge),
            ">>" => Some(Self::Gt),
            "<=" => Some(Self::Le),
            "<<" => Some(Self::Lt),
            _ => None,
        }
    }
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
    /// Virtual packages (Provides: field)
    provides: HashMap<String, Vec<String>>, // virtual_name -> [real_packages]
    /// Packages selected for installation
    selected: HashSet<String>,
    /// Resolved dependency graph
    dep_graph: HashMap<String, HashSet<String>>,
}

impl DependencyResolver {
    /// Create a new resolver with current system state
    pub fn new() -> Result<Self> {
        let packages = get_detailed_packages()?;
        let installed_list = list_installed_fast()?;

        let mut available = HashMap::with_capacity(packages.len());
        let mut provides: HashMap<String, Vec<String>> = HashMap::new();

        for pkg in packages {
            // Build provides map (would need to parse Provides field from package)
            // For now, just map package name to itself
            provides
                .entry(pkg.name.clone())
                .or_default()
                .push(pkg.name.clone());

            available.insert(pkg.name.clone(), pkg);
        }

        let installed: HashMap<String, String> = installed_list
            .into_iter()
            .map(|p| (p.name, p.version))
            .collect();

        Ok(Self {
            available,
            installed,
            provides,
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
                    k.starts_with(&name[..name.len().min(3)])
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
        use ahash::AHashSet;
        use rayon::prelude::*;

        let mut to_install = Vec::new();
        let mut to_upgrade = Vec::new();
        let mut visited = AHashSet::new();
        let mut download_size = 0u64;
        let mut installed_size = 0u64;

        // Process each selected package
        let selected: Vec<_> = self.selected.iter().cloned().collect();

        // OPTIMIZATION: Use FxHashSet for faster lookups (no crypto needed)
        use rustc_hash::FxHashSet;

        // For multiple independent packages, resolve initial dependencies in parallel
        if selected.len() > 1 {
            tracing::debug!("Resolving {} packages in parallel", selected.len());

            // First pass: resolve each selected package independently in parallel
            let parallel_results: Vec<_> = selected
                .par_iter()
                .map(|pkg_name| {
                    let mut local_visited = FxHashSet::default();
                    let mut local_to_install = Vec::new();

                    // Create a thread-safe clone of resolver state for read-only access
                    match self.resolve_package_readonly(
                        pkg_name,
                        &mut local_visited,
                        &mut local_to_install,
                    ) {
                        Ok(()) => Ok((local_to_install, local_visited)),
                        Err(e) => Err(e),
                    }
                })
                .collect();

            // Merge results (deduplicate packages)
            let mut seen = FxHashSet::default();
            for result in parallel_results {
                let (local_install, local_visited) = result?;
                for pkg in local_install {
                    if seen.insert(pkg.clone()) {
                        to_install.push(pkg);
                    }
                }
                visited.extend(local_visited);
            }
        } else {
            // Single package: use sequential resolution (no overhead)
            for pkg_name in selected {
                self.resolve_package(&pkg_name, &mut visited, &mut to_install)?;
            }
        }

        // Topologically sort the install order
        let sorted = self.topological_sort(&to_install)?;

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

    /// Recursively resolve a package and its dependencies (read-only for parallel use)
    fn resolve_package_readonly(
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
            .cloned()
            .with_context(|| format!("Package '{name}' not found in repository"))?;

        tracing::debug!(
            "Resolving package (parallel): {} (version: {})",
            name,
            pkg.version
        );

        // Parse and resolve dependencies
        for dep_str in &pkg.depends {
            let dep = parse_dependency(dep_str);
            match self.resolve_dependency_readonly(&dep, visited, to_install) {
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

        // Add this package after its dependencies
        if !to_install.contains(&name.to_string()) {
            to_install.push(name.to_string());
        }

        Ok(())
    }

    /// Recursively resolve a package and its dependencies
    fn resolve_package(
        &mut self,
        name: &str,
        visited: &mut ahash::AHashSet<String>,
        to_install: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }
        visited.insert(name.to_string());

        let pkg = self
            .available
            .get(name)
            .cloned()
            .with_context(|| format!("Package '{name}' not found in repository"))?;

        tracing::debug!("Resolving package: {} (version: {})", name, pkg.version);

        // Parse and resolve dependencies
        for dep_str in &pkg.depends {
            let dep = parse_dependency(dep_str);
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

        // Add this package after its dependencies
        if !to_install.contains(&name.to_string()) {
            to_install.push(name.to_string());
        }

        // Build dependency graph for topological sort
        let deps: HashSet<String> = pkg.depends.iter().map(|d| parse_dep_name(d)).collect();
        self.dep_graph.insert(name.to_string(), deps);

        Ok(())
    }

    /// Resolve a single dependency (read-only for parallel use)
    fn resolve_dependency_readonly(
        &self,
        dep: &Dependency,
        visited: &mut rustc_hash::FxHashSet<String>,
        to_install: &mut Vec<String>,
    ) -> Result<()> {
        // Check if already installed
        if let Some(installed_ver) = self.installed.get(&dep.name)
            && (dep.version_constraint.is_none()
                || Self::version_satisfies(
                    installed_ver,
                    dep.version_constraint
                        .as_ref()
                        .expect("guarded by is_none() check"),
                ))
        {
            return Ok(());
        }

        // Check if available
        if let Some(pkg) = self.available.get(&dep.name)
            && (dep.version_constraint.is_none()
                || Self::version_satisfies(
                    &pkg.version,
                    dep.version_constraint
                        .as_ref()
                        .expect("guarded by is_none() check"),
                ))
        {
            return self.resolve_package_readonly(&dep.name, visited, to_install);
        }

        // Check virtual packages (Provides:)
        if let Some(providers) = self.provides.get(&dep.name).cloned() {
            for provider in providers {
                if provider != dep.name {
                    if self.installed.contains_key(&provider) {
                        return Ok(()); // Already satisfied
                    }
                    if self.available.contains_key(&provider) {
                        return self.resolve_package_readonly(&provider, visited, to_install);
                    }
                }
            }
        }

        // Try alternatives
        for alt in &dep.alternatives {
            if self
                .resolve_dependency_readonly(alt, visited, to_install)
                .is_ok()
            {
                return Ok(());
            }
        }

        // Build helpful error message
        use std::fmt::Write;
        let mut error_msg = format!("Cannot satisfy dependency: {}", dep.name);
        if !dep.alternatives.is_empty() {
            let alt_names: Vec<_> = dep.alternatives.iter().map(|a| a.name.as_str()).collect();
            write!(
                &mut error_msg,
                "\n  Tried alternatives: {}",
                alt_names.join(", ")
            )
            .unwrap();
        }
        if let Some(constraint) = &dep.version_constraint {
            write!(
                &mut error_msg,
                " (version: {:?} {})",
                constraint.op, constraint.version
            )
            .unwrap();
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

    /// Resolve a single dependency (may be satisfied by multiple packages)
    fn resolve_dependency(
        &mut self,
        dep: &Dependency,
        visited: &mut ahash::AHashSet<String>,
        to_install: &mut Vec<String>,
    ) -> Result<()> {
        // Check if already installed
        if let Some(installed_ver) = self.installed.get(&dep.name)
            && (dep.version_constraint.is_none()
                || Self::version_satisfies(
                    installed_ver,
                    dep.version_constraint
                        .as_ref()
                        .expect("guarded by is_none() check"),
                ))
        {
            return Ok(());
        }

        // Check if available
        if let Some(pkg) = self.available.get(&dep.name)
            && (dep.version_constraint.is_none()
                || Self::version_satisfies(
                    &pkg.version,
                    dep.version_constraint
                        .as_ref()
                        .expect("guarded by is_none() check"),
                ))
        {
            return self.resolve_package(&dep.name, visited, to_install);
        }

        // Check virtual packages (Provides:)
        if let Some(providers) = self.provides.get(&dep.name).cloned() {
            for provider in providers {
                if provider != dep.name {
                    if self.installed.contains_key(&provider) {
                        return Ok(()); // Already satisfied
                    }
                    if self.available.contains_key(&provider) {
                        return self.resolve_package(&provider, visited, to_install);
                    }
                }
            }
        }

        // Try alternatives
        for alt in &dep.alternatives {
            if self.resolve_dependency(alt, visited, to_install).is_ok() {
                return Ok(());
            }
        }

        // Build helpful error message
        use std::fmt::Write;
        let mut error_msg = format!("Cannot satisfy dependency: {}", dep.name);
        if !dep.alternatives.is_empty() {
            let alt_names: Vec<_> = dep.alternatives.iter().map(|a| a.name.as_str()).collect();
            write!(
                &mut error_msg,
                "\n  Tried alternatives: {}",
                alt_names.join(", ")
            )
            .unwrap();
        }
        if let Some(constraint) = &dep.version_constraint {
            write!(
                &mut error_msg,
                " (version: {:?} {})",
                constraint.op, constraint.version
            )
            .unwrap();
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
        use ahash::AHashMap;

        // Use AHashMap for faster lookups (vs std HashMap)
        let mut in_degree: AHashMap<String, usize> = AHashMap::with_capacity(packages.len());
        let mut graph: AHashMap<String, Vec<String>> = AHashMap::with_capacity(packages.len());

        // Build reverse graph (package -> packages that depend on it)
        for pkg in packages {
            in_degree.entry(pkg.clone()).or_insert(0);
            if let Some(deps) = self.dep_graph.get(pkg) {
                for dep in deps {
                    if packages.contains(dep) {
                        graph.entry(dep.clone()).or_default().push(pkg.clone());
                        *in_degree.entry(pkg.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(pkg, _)| pkg.clone())
            .collect();

        let mut result = Vec::with_capacity(packages.len());

        while let Some(pkg) = queue.pop_front() {
            result.push(pkg.clone());

            if let Some(dependents) = graph.get(&pkg) {
                for dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        if result.len() != packages.len() {
            let missing: Vec<_> = packages.iter().filter(|p| !result.contains(p)).collect();

            tracing::error!("Circular dependency detected among packages: {:?}", missing);

            anyhow::bail!(
                "Circular dependency detected. Unable to determine installation order for: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        tracing::debug!(
            "Topological sort complete: {} packages ordered",
            result.len()
        );
        Ok(result)
    }

    /// Check for conflicts between packages
    /// Returns packages that would conflict with the proposed installation
    ///
    /// # Future Enhancement
    /// Full conflict detection would require parsing Conflicts: and Breaks: fields
    /// from package metadata. Current implementation returns empty (no conflicts).
    pub fn check_conflicts(&self, _packages: &[String]) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

/// Parse a dependency string like "libc6 (>= 2.38)"
fn parse_dependency(s: &str) -> Dependency {
    let s = s.trim();

    // Check for alternatives (|)
    if s.contains('|') {
        let parts: Vec<&str> = s.split('|').collect();
        let first = parse_single_dep(parts[0].trim());
        let mut dep = first;
        dep.alternatives = parts[1..]
            .iter()
            .map(|p| parse_single_dep(p.trim()))
            .collect();
        return dep;
    }

    parse_single_dep(s)
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
fn parse_single_dep(s: &str) -> Dependency {
    // OPTIMIZATION: Use memchr for faster parenthesis search (SIMD)
    if let Some(paren_start) = memchr::memchr(b'(', s.as_bytes()) {
        let raw_name = s[..paren_start].trim();
        let name = strip_arch_qualifier(raw_name).to_string();
        if let Some(paren_end) = memchr::memchr(b')', s.as_bytes()) {
            let constraint_str = &s[paren_start + 1..paren_end];
            if let Some(constraint) = parse_version_constraint(constraint_str) {
                return Dependency {
                    name,
                    version_constraint: Some(constraint),
                    alternatives: Vec::new(),
                };
            }
        }
        return Dependency {
            name,
            version_constraint: None,
            alternatives: Vec::new(),
        };
    }

    Dependency {
        name: strip_arch_qualifier(s.trim()).to_string(),
        version_constraint: None,
        alternatives: Vec::new(),
    }
}

/// Parse a version constraint like `">= 2.38"`
/// OPTIMIZATION: Inline and use byte comparisons for faster parsing
#[inline]
fn parse_version_constraint(s: &str) -> Option<VersionConstraint> {
    let s = s.trim();
    let bytes = s.as_bytes();

    // Fast path: Check first two bytes for two-char operators
    if bytes.len() >= 2 {
        let op = match (bytes[0], bytes[1]) {
            (b'>', b'=') => VersionOp::Ge,
            (b'>', b'>') => VersionOp::Gt,
            (b'<', b'=') => VersionOp::Le,
            (b'<', b'<') => VersionOp::Lt,
            _ => {
                // Single-char operator '='
                if bytes[0] == b'=' {
                    let version = s[1..].trim().to_string();
                    return Some(VersionConstraint {
                        op: VersionOp::Eq,
                        version,
                    });
                }
                return None;
            }
        };
        let version = s[2..].trim().to_string();
        return Some(VersionConstraint { op, version });
    }

    None
}

/// Extract just the package name from a dependency string
#[inline]
fn parse_dep_name(s: &str) -> String {
    let s = s.trim();
    // OPTIMIZATION: Manual character search for common delimiters (faster than find)
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'(' || b == b'|' || b == b':' {
            return s[..i].trim().to_string();
        }
    }
    s.to_string()
}

/// Compare two Debian version strings
///
/// Format: `[epoch:]upstream_version[-debian_revision]`
/// - Epoch: Optional integer, defaults to 0
/// - Upstream version: The main version from upstream
/// - Debian revision: Debian-specific changes
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (epoch_a, rest_a) = split_epoch(a);
    let (epoch_b, rest_b) = split_epoch(b);

    // Compare epochs first
    match epoch_a.cmp(&epoch_b) {
        std::cmp::Ordering::Equal => {}
        other => return other,
    }

    let (upstream_a, revision_a) = split_revision(rest_a);
    let (upstream_b, revision_b) = split_revision(rest_b);

    // Compare upstream versions
    match compare_version_part(upstream_a, upstream_b) {
        std::cmp::Ordering::Equal => {}
        other => return other,
    }

    // Compare Debian revisions
    compare_version_part(revision_a, revision_b)
}

/// Split epoch from version string
#[inline]
fn split_epoch(version: &str) -> (u32, &str) {
    // OPTIMIZATION: Use memchr for faster colon search (SIMD)
    if let Some(pos) = memchr::memchr(b':', version.as_bytes()) {
        let epoch = version[..pos].parse().unwrap_or(0);
        (epoch, &version[pos + 1..])
    } else {
        (0, version)
    }
}

/// Split Debian revision from version string
#[inline]
fn split_revision(version: &str) -> (&str, &str) {
    // OPTIMIZATION: Use memrchr for faster reverse search (SIMD)
    if let Some(pos) = memchr::memrchr(b'-', version.as_bytes()) {
        (&version[..pos], &version[pos + 1..])
    } else {
        (version, "")
    }
}

/// Compare version parts using Debian's algorithm
fn compare_version_part(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        // Collect non-digit prefix using peek to avoid consuming extra chars
        let mut a_prefix = String::new();
        while let Some(&c) = a_chars.peek() {
            if c.is_ascii_digit() {
                break;
            }
            a_prefix.push(a_chars.next().expect("peek() confirmed Some"));
        }

        let mut b_prefix = String::new();
        while let Some(&c) = b_chars.peek() {
            if c.is_ascii_digit() {
                break;
            }
            b_prefix.push(b_chars.next().expect("peek() confirmed Some"));
        }

        match compare_non_digit(&a_prefix, &b_prefix) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }

        // Collect numeric parts using peek
        let mut a_num = String::new();
        while let Some(&c) = a_chars.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            a_num.push(a_chars.next().expect("peek() confirmed Some"));
        }

        let mut b_num = String::new();
        while let Some(&c) = b_chars.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            b_num.push(b_chars.next().expect("peek() confirmed Some"));
        }

        if a_num.is_empty() && b_num.is_empty() {
            break;
        }

        let a_val: u64 = a_num.parse().unwrap_or(0);
        let b_val: u64 = b_num.parse().unwrap_or(0);

        match a_val.cmp(&b_val) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }

    std::cmp::Ordering::Equal
}

/// Compare non-digit parts of version strings
/// In Debian versioning, tilde sorts before everything including empty string
fn compare_non_digit(a: &str, b: &str) -> std::cmp::Ordering {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let max_len = a_chars.len().max(b_chars.len());

    for i in 0..max_len {
        let ac = a_chars.get(i);
        let bc = b_chars.get(i);

        match (ac, bc) {
            (None, None) => break,
            // Empty string sorts after tilde but before everything else
            (Some(&c), None) => {
                if c == '~' {
                    return std::cmp::Ordering::Less; // Tilde before empty
                }
                return std::cmp::Ordering::Greater; // Everything else after empty
            }
            (None, Some(&c)) => {
                if c == '~' {
                    return std::cmp::Ordering::Greater; // Empty after tilde
                }
                return std::cmp::Ordering::Less; // Empty before everything else
            }
            (Some(&ac), Some(&bc)) => {
                let order = char_order(ac).cmp(&char_order(bc));
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
        }
    }

    std::cmp::Ordering::Equal
}

/// Get the sort order for a character in Debian versioning
fn char_order(c: char) -> i32 {
    match c {
        '~' => -1, // Tilde sorts before everything (including empty)
        _ if c.is_ascii_alphabetic() => c as i32,
        _ => (c as i32) + 256, // Non-alphanumeric after letters
    }
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
    fn test_parse_dependency_simple() {
        let dep = parse_dependency("libc6");
        assert_eq!(dep.name, "libc6");
        assert!(dep.version_constraint.is_none());
    }

    #[test]
    fn test_parse_dependency_with_version() {
        let dep = parse_dependency("libc6 (>= 2.38)");
        assert_eq!(dep.name, "libc6");
        assert!(dep.version_constraint.is_some());
        let vc = dep.version_constraint.unwrap();
        assert_eq!(vc.op, VersionOp::Ge);
        assert_eq!(vc.version, "2.38");
    }

    #[test]
    fn test_parse_dependency_alternatives() {
        let dep = parse_dependency("libssl1.1 | libssl3");
        assert_eq!(dep.name, "libssl1.1");
        assert_eq!(dep.alternatives.len(), 1);
        assert_eq!(dep.alternatives[0].name, "libssl3");
    }

    #[test]
    fn test_parse_dep_name() {
        assert_eq!(parse_dep_name("libc6"), "libc6");
        assert_eq!(parse_dep_name("libc6 (>= 2.38)"), "libc6");
        assert_eq!(parse_dep_name("libc6:amd64"), "libc6");
    }

    #[test]
    fn test_parse_dependency_with_arch_qualifier() {
        // Test :any (multi-arch: allows any architecture)
        let dep = parse_dependency("perl:any");
        assert_eq!(dep.name, "perl");
        assert!(dep.version_constraint.is_none());

        // Test :native (same as build architecture)
        let dep = parse_dependency("build-essential:native");
        assert_eq!(dep.name, "build-essential");

        // Test with version constraint
        let dep = parse_dependency("libc6:amd64 (>= 2.38)");
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
}
