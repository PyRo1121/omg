//! Security policy enforcement and package security grading
//!
//! Defines security policies for package approval/rejection based on
//! vulnerabilities, licenses, and trust levels with A-F grading.

use crate::package_managers::types::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

use crate::core::paths;

/// Failures from loading a security policy or checking a package against it.
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Failed to read security policy: {path}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse security policy: {path}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("Security Grade '{grade}' for '{name}' is below required minimum '{minimum}'")]
    GradeTooLow {
        name: String,
        grade: SecurityGrade,
        minimum: SecurityGrade,
    },
    #[error("Package '{name}' is banned by security policy")]
    Banned { name: String },
    #[error("Package '{name}' is from AUR, which is disabled by security policy")]
    AurDisabled { name: String },
    #[error("Package '{name}' is unsigned; security policy requires PGP or checksum verification")]
    PgpRequired { name: String },
    #[error("Package '{name}' has license '{license}' which is not in allowed list")]
    LicenseNotAllowed { name: String, license: String },
    #[error("Package '{name}' has unknown license, but allowed list is enforced")]
    LicenseUnknown { name: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecurityGrade {
    Risk = 0,      // Known vulnerabilities
    Community = 1, // AUR/Unsigned
    Verified = 2,  // PGP or Checksum
    Locked = 3,    // SLSA + PGP
}

impl std::fmt::Display for SecurityGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => write!(f, "LOCKED (SLSA + PGP)"),
            Self::Verified => write!(f, "VERIFIED (PGP/Checksum)"),
            Self::Community => write!(f, "COMMUNITY (AUR/Unsigned)"),
            Self::Risk => write!(f, "RISK (Vulnerabilities)"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SecurityPolicy {
    #[serde(default = "default_minimum_grade")]
    pub minimum_grade: SecurityGrade,
    #[serde(default = "default_true")]
    pub allow_aur: bool,
    #[serde(default)]
    pub require_pgp: bool,
    #[serde(default)]
    pub allowed_licenses: Vec<String>,
    #[serde(default)]
    pub banned_packages: Vec<String>,
}

const fn default_minimum_grade() -> SecurityGrade {
    SecurityGrade::Community
}

const fn default_true() -> bool {
    true
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            minimum_grade: SecurityGrade::Community,
            allow_aur: true,
            require_pgp: false,
            allowed_licenses: Vec::new(),
            banned_packages: Vec::new(),
        }
    }
}

static INHERITED_POLICY: std::sync::OnceLock<SecurityPolicy> = std::sync::OnceLock::new();
pub const POLICY_MARKER: &str = "__omg_policy=";

/// The privileged child receives the parent's policy as bounded argv data,
/// since sudo resets XDG configuration environment variables.
pub fn inherit_policy(argument: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::core::privilege::is_root(),
        "Only an elevated child can inherit policy"
    );
    let encoded = argument
        .strip_prefix(POLICY_MARKER)
        .ok_or_else(|| anyhow::anyhow!("Missing policy marker"))?;
    anyhow::ensure!(encoded.len() <= 65536, "Inherited policy exceeds limit");
    let policy = serde_json::from_slice(&hex::decode(encoded)?)?;
    INHERITED_POLICY
        .set(policy)
        .map_err(|_| anyhow::anyhow!("Duplicate inherited policy"))
}

pub fn explicit_policy_exists() -> bool {
    INHERITED_POLICY.get().is_some() || paths::config_dir().join("policy.toml").exists()
}

pub fn policy_handoff() -> anyhow::Result<Option<String>> {
    if !explicit_policy_exists() {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(&SecurityPolicy::load_default()?)?;
    anyhow::ensure!(
        bytes.len() <= 32768,
        "Security policy exceeds elevation handoff limit"
    );
    Ok(Some(format!("{POLICY_MARKER}{}", hex::encode(bytes))))
}

impl SecurityPolicy {
    /// Load policy from file. A missing file is not handled here; callers that
    /// want defaults for an absent policy should use [`Self::load_optional`].
    ///
    /// # Errors
    /// Returns [`PolicyError::Read`] for unreadable files and
    /// [`PolicyError::Parse`] for malformed TOML.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PolicyError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| PolicyError::Read {
            path: path.display().to_string(),
            source,
        })?;
        toml::from_str(&content).map_err(|source| PolicyError::Parse {
            path: path.display().to_string(),
            source,
        })
    }

    /// Load a policy file, using the built-in default only when the file is absent.
    pub fn load_optional(path: impl AsRef<Path>) -> Result<Self, PolicyError> {
        match Self::load(&path) {
            Ok(policy) => Ok(policy),
            Err(PolicyError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                // Missing optional configuration is normal. Keep the fallback
                // visible once to verbose diagnostics without polluting routine
                // commands or machine-readable output.
                static MISSING_POLICY_LOGGED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !MISSING_POLICY_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::debug!(
                        path = %path.as_ref().display(),
                        "No policy file; using permissive built-in default (AUR allowed)"
                    );
                }
                Ok(Self::default())
            }
            Err(error) => Err(error),
        }
    }

    /// Load from default location (~/.config/omg/policy.toml).
    /// A missing file uses the built-in default; a corrupt or unreadable file fails closed.
    pub fn load_default() -> Result<Self, PolicyError> {
        if let Some(policy) = INHERITED_POLICY.get() {
            return Ok(policy.clone());
        }
        Self::load_optional(paths::config_dir().join("policy.toml"))
    }

    /// Assign a security grade to a package based on metadata
    pub async fn assign_grade(
        &self,
        scanner: &dyn super::vulnerability::VulnerabilitySource,
        name: &str,
        version: &Version,
        is_official: bool,
    ) -> Result<SecurityGrade, super::vulnerability::VulnerabilityError> {
        // An unavailable evidence source must not be treated as a clean package.
        if !scanner.scan_package(name, version).await?.is_empty() {
            return Ok(SecurityGrade::Risk);
        }

        // Official packages are Verified (signed repository metadata). A Locked
        // grade requires provenance evidence, which this function does not have.
        if is_official {
            return Ok(SecurityGrade::Verified);
        }

        Ok(SecurityGrade::Community)
    }

    /// Check a package using the trust grade supplied by its source.
    ///
    /// Official repository metadata is treated as `Verified`; AUR and local
    /// inputs remain `Community` until a dedicated verification result exists.
    pub fn check_source(
        &self,
        name: &str,
        is_aur: bool,
        license: Option<&str>,
    ) -> Result<(), PolicyError> {
        let grade = if is_aur {
            SecurityGrade::Community
        } else {
            SecurityGrade::Verified
        };
        self.check_package(name, is_aur, license, grade)
    }

    /// Check if a package is allowed by policy
    pub fn check_package(
        &self,
        name: &str,
        is_aur: bool,
        license: Option<&str>,
        grade: SecurityGrade,
    ) -> Result<(), PolicyError> {
        if grade < self.minimum_grade {
            return Err(PolicyError::GradeTooLow {
                name: name.to_string(),
                grade,
                minimum: self.minimum_grade,
            });
        }

        if self.banned_packages.iter().any(|banned| banned == name) {
            return Err(PolicyError::Banned {
                name: name.to_string(),
            });
        }

        if is_aur && !self.allow_aur {
            return Err(PolicyError::AurDisabled {
                name: name.to_string(),
            });
        }

        if self.require_pgp && grade < SecurityGrade::Verified {
            return Err(PolicyError::PgpRequired {
                name: name.to_string(),
            });
        }

        if !self.allowed_licenses.is_empty() {
            match license {
                Some(lic) if license_matches_allowlist(lic, &self.allowed_licenses) => {}
                Some(lic) => {
                    return Err(PolicyError::LicenseNotAllowed {
                        name: name.to_string(),
                        license: lic.to_string(),
                    });
                }
                None => {
                    return Err(PolicyError::LicenseUnknown {
                        name: name.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Lowercase SPDX-ish tokens from a license expression.
pub(crate) fn spdx_license_tokens(license: &str) -> Vec<String> {
    license
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')))
        .filter(|token| !token.is_empty())
        .filter(|token| {
            !["AND", "OR", "WITH", "TO"]
                .iter()
                .any(|operator| token.eq_ignore_ascii_case(operator))
        })
        .map(str::to_ascii_lowercase)
        .collect()
}

/// True when `license` contains an allowed SPDX identifier as a whole token.
pub(crate) fn license_matches_allowlist(license: &str, allowed: &[String]) -> bool {
    let tokens = spdx_license_tokens(license);
    allowed.iter().any(|allowed| {
        tokens.iter().any(|token| {
            token.eq_ignore_ascii_case(allowed)
                || token
                    .strip_suffix('+')
                    .is_some_and(|token| token.eq_ignore_ascii_case(allowed))
                || allowed
                    .strip_suffix('+')
                    .is_some_and(|allowed| token.eq_ignore_ascii_case(allowed))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security::vulnerability::VulnerabilityError;

    #[test]
    fn test_grade_ordering() {
        assert!(SecurityGrade::Locked > SecurityGrade::Verified);
        assert!(SecurityGrade::Verified > SecurityGrade::Community);
        assert!(SecurityGrade::Community > SecurityGrade::Risk);
    }

    #[test]
    fn source_policy_assigns_verified_grade_only_to_official_packages() {
        let policy = SecurityPolicy {
            minimum_grade: SecurityGrade::Verified,
            ..SecurityPolicy::default()
        };
        assert!(policy.check_source("system", false, None).is_ok());
        assert!(matches!(
            policy.check_source("community", true, None),
            Err(PolicyError::GradeTooLow { .. })
        ));
    }

    #[test]
    fn test_policy_check_grade() {
        let policy = SecurityPolicy {
            minimum_grade: SecurityGrade::Verified,
            ..SecurityPolicy::default()
        };

        // Verified is allowed
        assert!(
            policy
                .check_package("test", false, None, SecurityGrade::Verified)
                .is_ok()
        );

        // Locked is allowed
        assert!(
            policy
                .check_package("test", false, None, SecurityGrade::Locked)
                .is_ok()
        );

        // Community is blocked
        let err = policy
            .check_package("test", true, None, SecurityGrade::Community)
            .expect_err("Community is below Verified");
        assert!(
            matches!(err, PolicyError::GradeTooLow { .. }),
            "grade failures must be typed, got: {err}"
        );
    }

    #[test]
    fn load_optional_uses_defaults_when_policy_is_missing() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let policy = SecurityPolicy::load_optional(temp.path().join("policy.toml"))
            .expect("missing policy should use defaults");
        assert_eq!(policy.minimum_grade, SecurityGrade::Community);
        assert!(policy.allow_aur);
    }

    #[test]
    fn load_optional_rejects_corrupt_policy() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let path = temp.path().join("policy.toml");
        fs::write(&path, "minimum_grade = [").expect("write corrupt policy");
        let error = SecurityPolicy::load_optional(&path).expect_err("corrupt policy must fail");
        assert!(
            error
                .to_string()
                .contains("Failed to parse security policy")
        );
    }

    struct EmptyVulns;

    impl super::super::vulnerability::VulnerabilitySource for EmptyVulns {
        fn scan_package<'a>(
            &'a self,
            _name: &'a str,
            _version: &'a crate::package_managers::types::Version,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<crate::core::security::vulnerability::VulnerabilityReport>,
                            VulnerabilityError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    struct FailingVulns;

    impl super::super::vulnerability::VulnerabilitySource for FailingVulns {
        fn scan_package<'a>(
            &'a self,
            _name: &'a str,
            _version: &'a crate::package_managers::types::Version,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<crate::core::security::vulnerability::VulnerabilityReport>,
                            VulnerabilityError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Err(VulnerabilityError::Unavailable {
                    reason: "osv unavailable".to_string(),
                })
            })
        }
    }

    #[tokio::test]
    async fn prepared_plan_checks_dependencies_and_actual_licenses() {
        let mut policy = SecurityPolicy {
            allowed_licenses: vec!["MIT".to_owned()],
            ..SecurityPolicy::default()
        };
        let package = |name: &str, license: &str| {
            (
                name.to_owned(),
                crate::package_managers::parse_version_or_zero("1.0"),
                false,
                Some(license.to_owned()),
            )
        };
        super::check_prepared_with_source(
            &policy,
            vec![package("app", "MIT"), package("dependency", "MIT")],
            &EmptyVulns,
        )
        .await
        .unwrap();
        policy.banned_packages.push("dependency".to_owned());
        assert!(
            super::check_prepared_with_source(
                &policy,
                vec![package("app", "MIT"), package("dependency", "MIT")],
                &EmptyVulns
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("dependency")
        );
        policy.banned_packages.clear();
        assert!(
            super::check_prepared_with_source(
                &policy,
                vec![package("app", "GPL-3.0")],
                &EmptyVulns
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn official_packages_are_verified_not_locked_by_name() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("2.40");
        let grade = policy
            .assign_grade(&EmptyVulns, "glibc", &version, true)
            .await
            .expect("empty vuln source");
        assert_eq!(
            grade,
            SecurityGrade::Verified,
            "package names must not mint a Locked SLSA grade"
        );
    }

    #[tokio::test]
    async fn aur_packages_are_community() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("1.0");
        let grade = policy
            .assign_grade(&EmptyVulns, "yay", &version, false)
            .await
            .expect("empty vuln source");
        assert_eq!(grade, SecurityGrade::Community);
    }

    #[tokio::test]
    async fn unavailable_vuln_source_fails_closed() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("1.0");
        let error = policy
            .assign_grade(&FailingVulns, "vim", &version, true)
            .await
            .expect_err("missing evidence must not look like a clean package");
        assert!(
            matches!(
                error,
                VulnerabilityError::Unavailable { ref reason } if reason == "osv unavailable"
            ),
            "scanner error must be preserved, got: {error}"
        );
    }

    #[test]
    fn require_pgp_rejects_unsigned_packages() {
        let policy = SecurityPolicy {
            require_pgp: true,
            ..SecurityPolicy::default()
        };
        let err = policy
            .check_package("yay", true, Some("MIT"), SecurityGrade::Community)
            .expect_err("AUR community packages are unsigned");
        assert!(
            matches!(err, PolicyError::PgpRequired { .. }),
            "require_pgp must be a typed unsigned-package error, got: {err}"
        );
        assert!(
            policy
                .check_package("vim", false, Some("MIT"), SecurityGrade::Verified)
                .is_ok()
        );
    }

    #[test]
    fn allowed_license_matches_spdx_tokens_not_substrings() {
        let policy = SecurityPolicy {
            allowed_licenses: vec!["MIT".to_string()],
            ..SecurityPolicy::default()
        };
        let err = policy
            .check_package("foo", false, Some("LIMITED"), SecurityGrade::Verified)
            .expect_err("LIMITED must not match MIT");
        assert!(
            matches!(err, PolicyError::LicenseNotAllowed { .. }),
            "allowlist mismatches must be typed, got: {err}"
        );
        assert!(
            policy
                .check_package(
                    "foo",
                    false,
                    Some("MIT OR Apache-2.0"),
                    SecurityGrade::Verified
                )
                .is_ok()
        );
    }
}

/// Native backends that cannot expose and bind their final dependency plan
/// must not silently bypass an explicitly installed OMG policy.
pub fn require_native_plan_support(backend: &str) -> anyhow::Result<()> {
    let policy = SecurityPolicy::load_default()?;
    anyhow::ensure!(
        !explicit_policy_exists() && policy == SecurityPolicy::default(),
        "{backend} cannot enforce an explicit OMG policy on its final dependency transaction; use a backend with prepared-plan policy enforcement"
    );
    Ok(())
}

/// Evaluate all additions after resolution, using actual archive/repository identities.
pub fn check_prepared_packages(
    packages: Vec<(String, Version, bool, Option<String>)>,
) -> anyhow::Result<()> {
    let policy = SecurityPolicy::load_default()?;
    if !explicit_policy_exists() {
        for (name, _, community, license) in packages {
            policy.check_source(&name, community, license.as_deref())?;
        }
        return Ok(());
    }
    // ALPM's synchronous callback may run inside a Tokio current-thread runtime.
    // A separate bounded worker owns its runtime rather than nesting block_on.
    std::thread::spawn(move || -> anyhow::Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let scanner = super::vulnerability::VulnerabilityScanner::new();
            check_prepared_with_source(&policy, packages, &scanner).await
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("Prepared transaction policy worker failed"))?
}

async fn check_prepared_with_source(
    policy: &SecurityPolicy,
    packages: Vec<(String, Version, bool, Option<String>)>,
    scanner: &dyn super::vulnerability::VulnerabilitySource,
) -> anyhow::Result<()> {
    for (name, version, community, license) in packages {
        policy.check_source(&name, community, license.as_deref())?;
        let grade = policy
            .assign_grade(scanner, &name, &version, !community)
            .await?;
        policy.check_package(&name, community, license.as_deref(), grade)?;
    }
    Ok(())
}
