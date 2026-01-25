//! Test fixtures and builders for common test scenarios

use crate::core::security::SecurityPolicy;
use crate::core::{Package, PackageSource};
use crate::package_managers::parse_version_or_zero;
use crate::package_managers::types::UpdateInfo;

/// Builder for creating test packages
#[derive(Debug, Clone, Default)]
pub struct PackageFixture {
    name: String,
    version: String,
    description: String,
    repo: String,
    installed: bool,
}

impl PackageFixture {
    /// Create a new package fixture builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the package name
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the package version
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set the package description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the package repo
    #[must_use]
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = repo.into();
        self
    }

    /// Set whether the package is installed
    #[must_use]
    pub fn installed(mut self, installed: bool) -> Self {
        self.installed = installed;
        self
    }

    /// Build the package
    #[must_use]
    pub fn build(self) -> Package {
        Package {
            name: if self.name.is_empty() {
                "test-package".to_string()
            } else {
                self.name
            },
            version: parse_version_or_zero(&if self.version.is_empty() {
                "1.0.0".to_string()
            } else {
                self.version
            }),
            description: if self.description.is_empty() {
                "Test package description".to_string()
            } else {
                self.description
            },
            source: PackageSource::Official,
            installed: self.installed,
        }
    }

    /// Create a standard Firefox package
    #[must_use]
    pub fn firefox() -> Self {
        Self::new()
            .name("firefox")
            .version("122.0-1")
            .description("Fast, Private & Safe Web Browser")
            .repo("extra")
            .installed(false)
    }

    /// Create a standard Git package
    #[must_use]
    pub fn git() -> Self {
        Self::new()
            .name("git")
            .version("2.43.0-1")
            .description("The fast distributed version control system")
            .repo("extra")
            .installed(false)
    }

    /// Create a standard Pacman package
    #[must_use]
    pub fn pacman() -> Self {
        Self::new()
            .name("pacman")
            .version("6.0.2-1")
            .description("A library-based package manager")
            .repo("core")
            .installed(true)
    }
}

/// Builder for creating update scenarios
#[derive(Debug, Clone)]
pub struct UpdateFixture {
    updates: Vec<UpdateInfo>,
}

impl UpdateFixture {
    /// Create a new update fixture builder
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
        }
    }

    /// Add an update to the fixture
    #[must_use]
    pub fn add_update(
        mut self,
        name: impl Into<String>,
        old: impl Into<String>,
        new: impl Into<String>,
    ) -> Self {
        self.updates.push(UpdateInfo {
            name: name.into(),
            old_version: old.into(),
            new_version: new.into(),
            repo: "extra".to_string(),
        });
        self
    }

    /// Add a major version update
    #[must_use]
    pub fn add_major(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.updates.push(UpdateInfo {
            name: name.clone(),
            old_version: "1.0.0-1".to_string(),
            new_version: "2.0.0-1".to_string(),
            repo: "extra".to_string(),
        });
        self
    }

    /// Add a minor version update
    #[must_use]
    pub fn add_minor(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.updates.push(UpdateInfo {
            name: name.clone(),
            old_version: "1.0.0-1".to_string(),
            new_version: "1.1.0-1".to_string(),
            repo: "extra".to_string(),
        });
        self
    }

    /// Add a patch version update
    #[must_use]
    pub fn add_patch(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.updates.push(UpdateInfo {
            name: name.clone(),
            old_version: "1.0.0-1".to_string(),
            new_version: "1.0.1-1".to_string(),
            repo: "extra".to_string(),
        });
        self
    }

    /// Build the update list
    #[must_use]
    pub fn build(self) -> Vec<UpdateInfo> {
        self.updates
    }

    /// Create a typical system update scenario
    #[must_use]
    pub fn typical_system() -> Vec<UpdateInfo> {
        Self::new()
            .add_patch("firefox")
            .add_minor("git")
            .add_major("kernel")
            .build()
    }
}

impl Default for UpdateFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating test security policies
#[derive(Debug, Clone)]
pub struct SecurityPolicyFixture {
    policy: SecurityPolicy,
}

impl SecurityPolicyFixture {
    /// Create a new security policy fixture
    pub fn new() -> Self {
        Self {
            policy: SecurityPolicy::default(),
        }
    }

    /// Set the policy to permissive mode (allow all)
    #[must_use]
    pub fn permissive(mut self) -> Self {
        self.policy = SecurityPolicy::default();
        self
    }

    /// Set the policy to strict mode (require verification)
    #[must_use]
    pub fn strict(mut self) -> Self {
        // In a real implementation, this would configure strict policies
        self.policy = SecurityPolicy::default();
        self
    }

    /// Build the policy
    #[must_use]
    pub fn build(self) -> SecurityPolicy {
        self.policy
    }
}

impl Default for SecurityPolicyFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// Common test scenarios
impl PackageFixture {
    /// Create a collection of packages for a search test
    pub fn search_results() -> Vec<Package> {
        vec![
            Self::firefox().installed(false).build(),
            Self::git().installed(true).build(),
            Self::pacman().installed(true).build(),
        ]
    }

    /// Create a collection of installed packages
    pub fn installed_packages() -> Vec<Package> {
        vec![
            Self::pacman().installed(true).build(),
            Self::git().installed(true).build(),
        ]
    }

    /// Create a collection of available packages
    pub fn available_packages() -> Vec<Package> {
        vec![
            Self::firefox().installed(false).build(),
            Self::new()
                .name("vim")
                .version("9.0.0-1")
                .description("Vi IMproved")
                .repo("extra")
                .installed(false)
                .build(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_fixture_builder() {
        let pkg = PackageFixture::new()
            .name("test")
            .version("1.0.0")
            .description("Test")
            .repo("repo")
            .installed(true)
            .build();

        assert_eq!(pkg.name, "test");
        assert_eq!(pkg.version.to_string(), "1.0.0");
        assert!(pkg.installed);
    }

    #[test]
    fn test_package_fixture_defaults() {
        let pkg = PackageFixture::new().build();
        assert_eq!(pkg.name, "test-package");
        assert_eq!(pkg.version.to_string(), "1.0.0");
    }

    #[test]
    fn test_package_fixture_presets() {
        let firefox = PackageFixture::firefox().build();
        assert_eq!(firefox.name, "firefox");
        assert_eq!(firefox.version.to_string(), "122.0-1");
    }

    #[test]
    fn test_update_fixture() {
        let updates = UpdateFixture::new()
            .add_patch("pkg1")
            .add_minor("pkg2")
            .add_major("pkg3")
            .build();

        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].name, "pkg1");
        assert_eq!(updates[1].name, "pkg2");
        assert_eq!(updates[2].name, "pkg3");
    }

    #[test]
    fn test_update_fixture_typical_system() {
        let updates = UpdateFixture::typical_system();
        assert_eq!(updates.len(), 3);
    }
}
