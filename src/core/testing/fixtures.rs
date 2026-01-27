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

/// Builder for creating test version scenarios
#[derive(Debug, Clone)]
pub struct VersionFixture {
    major: u32,
    minor: u32,
    patch: u32,
    prefix: String,
    suffix: String,
}

impl VersionFixture {
    /// Create a new version fixture
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// Add a 'v' prefix to the version
    #[must_use]
    pub fn with_v_prefix(mut self) -> Self {
        self.prefix = "v".to_string();
        self
    }

    /// Add a build suffix
    #[must_use]
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.suffix = format!("+{}", build.into());
        self
    }

    /// Add a pre-release suffix
    #[must_use]
    pub fn with_prerelease(mut self, prerelease: impl Into<String>) -> Self {
        self.suffix = format!("-{}", prerelease.into());
        self
    }

    /// Build the version string
    #[must_use]
    pub fn build(self) -> String {
        format!(
            "{}{}.{}.{}{}",
            self.prefix, self.major, self.minor, self.patch, self.suffix
        )
    }

    /// Create a semver version
    #[must_use]
    pub fn semver(major: u32, minor: u32, patch: u32) -> String {
        Self::new(major, minor, patch).build()
    }

    /// Create an alpha version
    #[must_use]
    pub fn alpha(major: u32, minor: u32, patch: u32) -> String {
        Self::new(major, minor, patch)
            .with_prerelease("alpha")
            .build()
    }

    /// Create a beta version
    #[must_use]
    pub fn beta(major: u32, minor: u32, patch: u32) -> String {
        Self::new(major, minor, patch).with_prerelease("beta").build()
    }

    /// Create an rc version
    #[must_use]
    pub fn rc(major: u32, minor: u32, patch: u32) -> String {
        Self::new(major, minor, patch).with_prerelease("rc").build()
    }
}

impl Default for VersionFixture {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

/// Builder for creating test error scenarios
#[derive(Debug, Clone)]
pub struct ErrorScenarioFixture {
    scenario_type: ErrorScenarioType,
}

#[derive(Debug, Clone)]
enum ErrorScenarioType {
    NetworkTimeout,
    FileNotFound,
    PermissionDenied,
    InvalidInput,
    ParseError,
}

impl ErrorScenarioFixture {
    /// Create a network timeout scenario
    #[must_use]
    pub fn network_timeout() -> Self {
        Self {
            scenario_type: ErrorScenarioType::NetworkTimeout,
        }
    }

    /// Create a file not found scenario
    #[must_use]
    pub fn file_not_found() -> Self {
        Self {
            scenario_type: ErrorScenarioType::FileNotFound,
        }
    }

    /// Create a permission denied scenario
    #[must_use]
    pub fn permission_denied() -> Self {
        Self {
            scenario_type: ErrorScenarioType::PermissionDenied,
        }
    }

    /// Create an invalid input scenario
    #[must_use]
    pub fn invalid_input() -> Self {
        Self {
            scenario_type: ErrorScenarioType::InvalidInput,
        }
    }

    /// Create a parse error scenario
    #[must_use]
    pub fn parse_error() -> Self {
        Self {
            scenario_type: ErrorScenarioType::ParseError,
        }
    }

    /// Get a sample error message for this scenario
    #[must_use]
    pub fn message(&self) -> String {
        match self.scenario_type {
            ErrorScenarioType::NetworkTimeout => "Network timeout after 30s".to_string(),
            ErrorScenarioType::FileNotFound => "File not found: /path/to/file".to_string(),
            ErrorScenarioType::PermissionDenied => "Permission denied".to_string(),
            ErrorScenarioType::InvalidInput => "Invalid input provided".to_string(),
            ErrorScenarioType::ParseError => "Failed to parse configuration".to_string(),
        }
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

    /// Create a standard Vim package
    #[must_use]
    pub fn vim() -> Self {
        Self::new()
            .name("vim")
            .version("9.0.0-1")
            .description("Vi IMproved - enhanced vi editor")
            .repo("extra")
            .installed(false)
    }

    /// Create a standard Rust package
    #[must_use]
    pub fn rust() -> Self {
        Self::new()
            .name("rust")
            .version("1.75.0-1")
            .description("Systems programming language")
            .repo("extra")
            .installed(false)
    }

    /// Create a standard Python package
    #[must_use]
    pub fn python() -> Self {
        Self::new()
            .name("python")
            .version("3.11.6-1")
            .description("High-level scripting language")
            .repo("core")
            .installed(true)
    }

    /// Create a standard Node.js package
    #[must_use]
    pub fn nodejs() -> Self {
        Self::new()
            .name("nodejs")
            .version("20.10.0-1")
            .description("JavaScript runtime")
            .repo("extra")
            .installed(false)
    }

    /// Create a standard Docker package
    #[must_use]
    pub fn docker() -> Self {
        Self::new()
            .name("docker")
            .version("24.0.7-1")
            .description("Container platform")
            .repo("extra")
            .installed(false)
    }

    /// Create a collection of development packages
    pub fn dev_tools() -> Vec<Package> {
        vec![
            Self::git().installed(true).build(),
            Self::python().installed(true).build(),
            Self::rust().installed(false).build(),
            Self::nodejs().installed(false).build(),
            Self::vim().installed(true).build(),
        ]
    }

    /// Create a minimal package for testing
    #[must_use]
    pub fn minimal() -> Self {
        Self::new()
            .name("minimal-pkg")
            .version("1.0.0")
            .description("Minimal test package")
            .installed(false)
    }

    /// Create a package with a very long name for testing
    #[must_use]
    pub fn long_name() -> Self {
        Self::new()
            .name("very-long-package-name-that-tests-display-limits-and-truncation")
            .version("1.0.0")
            .description("Package with an extremely long name")
            .installed(false)
    }

    /// Create a collection of packages with different states
    pub fn mixed_states() -> Vec<Package> {
        vec![
            Self::firefox().installed(false).build(),
            Self::git().installed(true).build(),
            Self::pacman().installed(true).build(),
            Self::vim().installed(false).build(),
            Self::rust().installed(false).build(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::implicit_clone)]
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
    #[allow(clippy::implicit_clone)]
    fn test_package_fixture_defaults() {
        let pkg = PackageFixture::new().build();
        assert_eq!(pkg.name, "test-package");
        assert_eq!(pkg.version.to_string(), "1.0.0");
    }

    #[test]
    #[allow(clippy::implicit_clone)]
    fn test_package_fixture_firefox_preset() {
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

    #[test]
    fn test_package_fixture_additional_presets() {
        // Arrange & Act
        let vim = PackageFixture::vim().build();
        let rust = PackageFixture::rust().build();
        let python = PackageFixture::python().build();
        let nodejs = PackageFixture::nodejs().build();

        // Assert
        assert_eq!(vim.name, "vim");
        assert_eq!(rust.name, "rust");
        assert_eq!(python.name, "python");
        assert_eq!(nodejs.name, "nodejs");
    }

    #[test]
    fn test_package_fixture_collections() {
        // Arrange & Act
        let dev_tools = PackageFixture::dev_tools();
        let mixed_states = PackageFixture::mixed_states();

        // Assert
        assert!(!dev_tools.is_empty());
        assert!(!mixed_states.is_empty());
        assert!(dev_tools.iter().any(|p| p.installed));
        assert!(mixed_states.iter().any(|p| p.installed));
        assert!(mixed_states.iter().any(|p| !p.installed));
    }

    #[test]
    fn test_version_fixture_basic() {
        // Arrange & Act
        let version = VersionFixture::semver(1, 2, 3);

        // Assert
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_version_fixture_with_prefix() {
        // Arrange & Act
        let version = VersionFixture::new(1, 2, 3).with_v_prefix().build();

        // Assert
        assert_eq!(version, "v1.2.3");
    }

    #[test]
    fn test_version_fixture_prerelease() {
        // Arrange & Act
        let alpha = VersionFixture::alpha(1, 0, 0);
        let beta = VersionFixture::beta(2, 0, 0);
        let rc = VersionFixture::rc(3, 0, 0);

        // Assert
        assert_eq!(alpha, "1.0.0-alpha");
        assert_eq!(beta, "2.0.0-beta");
        assert_eq!(rc, "3.0.0-rc");
    }

    #[test]
    fn test_error_scenario_fixture() {
        // Arrange & Act
        let timeout = ErrorScenarioFixture::network_timeout();
        let not_found = ErrorScenarioFixture::file_not_found();
        let permission = ErrorScenarioFixture::permission_denied();

        // Assert
        assert!(timeout.message().contains("timeout"));
        assert!(not_found.message().contains("not found"));
        assert!(permission.message().contains("Permission"));
    }
}
