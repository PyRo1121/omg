#![cfg(all(
    any(feature = "debian", feature = "debian-pure"),
    feature = "debian-resolvo"
))]

use anyhow::Result;

use super::resolver::{DependencyResolver, ResolutionResult};

pub struct ResolvoAdapter;

impl ResolvoAdapter {
    pub fn is_available() -> bool {
        true
    }

    pub fn resolve_packages(packages: &[String]) -> Result<ResolutionResult> {
        let mut resolver = DependencyResolver::new()?;
        for package in packages {
            resolver.add_package(package)?;
        }
        resolver.resolve()
    }
}
