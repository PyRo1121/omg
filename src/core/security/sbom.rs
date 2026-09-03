//! Software Bill of Materials (SBOM) generation in `CycloneDX` format
//!
//! Generates industry-standard `CycloneDX` 1.5 SBOMs for compliance,
//! supply chain security, and vulnerability tracking.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::paths;
#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
use crate::package_managers::VersionDisplay;

use super::vulnerability::PackageSource;

/// `CycloneDX` SBOM format (industry standard for enterprise)
/// Compliant with `CycloneDX` 1.5 specification
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Sbom {
    pub bom_format: String,
    pub spec_version: String,
    pub serial_number: String,
    pub version: u32,
    pub metadata: SbomMetadata,
    pub components: Vec<SbomComponent>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<SbomDependency>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vulnerabilities: Vec<SbomVulnerability>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SbomMetadata {
    pub timestamp: String,
    pub tools: Vec<SbomTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<SbomComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacture: Option<SbomOrganization>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier: Option<SbomOrganization>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomTool {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomOrganization {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SbomComponent {
    #[serde(rename = "type")]
    pub component_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(rename = "bom-ref", skip_serializing_if = "Option::is_none")]
    pub bom_ref: Option<String>,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purl: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub licenses: Vec<SbomLicense>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub hashes: Vec<SbomHash>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub external_references: Vec<SbomExternalRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<SbomProperty>>,
}

/// Failures generating or exporting a CycloneDX SBOM.
#[derive(Debug, Error)]
pub enum SbomError {
    #[error("Failed to list installed packages")]
    ListPackages {
        #[source]
        source: PackageSource,
    },
    #[error("Failed to fetch vulnerability data")]
    FetchVulnerabilities {
        #[source]
        source: super::vulnerability::VulnerabilityError,
    },
    #[error("Failed to serialize SBOM")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("Failed to create SBOM directory '{path}'")]
    CreateDir {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to write SBOM '{path}'")]
    Write {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("SBOM generation is not available without an Arch or Debian package backend")]
    NoBackend,
    #[error(
        "Arch Linux Security Advisory data cannot be used to scan Debian packages; generate the SBOM without vulnerability matching"
    )]
    AlsaUnsupportedOnDebian,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomLicense {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<SbomLicenseInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomLicenseInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomHash {
    pub alg: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomExternalRef {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SbomDependency {
    #[serde(rename = "ref")]
    pub dep_ref: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SbomVulnerability {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SbomVulnSource>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ratings: Vec<SbomVulnRating>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub affects: Vec<SbomVulnAffects>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomVulnSource {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomVulnRating {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SbomVulnAffects {
    #[serde(rename = "ref")]
    pub affects_ref: String,
}

/// Whether an ALSA advisory applies to one installed package.
///
/// Advisories cover `[affected, fixed)`, so matching by name alone reports
/// historical advisories for fully patched packages (W5-B-01). This routes
/// through the same `version_is_affected` check `scan_system` uses; `None`
/// (unparseable version) skips the pair with a visible warning instead of
/// fabricating a comparison.
#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
fn advisory_applies(
    name: &str,
    installed_version: &str,
    affected: &str,
    fixed: Option<&str>,
) -> bool {
    if let Some(applies) =
        super::vulnerability::version_is_affected(installed_version, affected, fixed)
    {
        applies
    } else {
        tracing::warn!(
            "Skipping ALSA advisory match for package '{name}': unparseable \
             version (installed '{installed_version}', affected '{affected}')"
        );
        false
    }
}

#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
fn package_purl(name: &str, version: &str, debian_like: bool) -> String {
    if debian_like {
        format!("pkg:deb/debian/{name}@{version}")
    } else {
        format!("pkg:pacman/archlinux/{name}@{version}")
    }
}

#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
fn package_website(name: &str, debian_like: bool) -> String {
    if debian_like {
        format!("https://packages.debian.org/{name}")
    } else {
        format!("https://archlinux.org/packages/?name={name}")
    }
}

#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
struct OsSbomIdentity {
    name: &'static str,
    purl: &'static str,
    version: &'static str,
    description: &'static str,
    supplier: &'static str,
    supplier_url: &'static str,
}

#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
fn os_sbom_identity(debian_like: bool) -> OsSbomIdentity {
    if debian_like {
        OsSbomIdentity {
            name: "Debian",
            purl: "pkg:os/debian",
            // Release is not known here; do not invent Arch's "rolling".
            version: "",
            description: "Debian-like system",
            supplier: "Debian",
            supplier_url: "https://www.debian.org",
        }
    } else {
        OsSbomIdentity {
            name: "Arch Linux",
            purl: "pkg:os/archlinux",
            version: "rolling",
            description: "Arch Linux system",
            supplier: "Arch Linux",
            supplier_url: "https://archlinux.org",
        }
    }
}

/// SBOM Generator for enterprise compliance
pub struct SbomGenerator {
    include_vulns: bool,
}

impl Default for SbomGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SbomGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_vulns: true,
        }
    }

    /// Include vulnerability matching from Arch Linux security advisory data.
    #[must_use]
    pub const fn with_vulnerabilities(mut self, include: bool) -> Self {
        self.include_vulns = include;
        self
    }

    /// Generate SBOM for all installed packages
    #[allow(
        clippy::needless_return,
        clippy::unused_async,
        reason = "backend builds await vulnerability data while backend-free builds fail directly"
    )]
    pub async fn generate_system_sbom(&self) -> Result<Sbom, SbomError> {
        #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
        {
            let _ = self;
            return Err(SbomError::NoBackend);
        }

        #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
        {
            #[cfg(feature = "arch")]
            let installed = crate::package_managers::list_installed_fast().map_err(|source| {
                SbomError::ListPackages {
                    source: PackageSource(source),
                }
            })?;
            #[cfg(all(
                any(feature = "debian", feature = "debian-pure"),
                not(feature = "arch")
            ))]
            let installed =
                crate::package_managers::apt_list_installed_fast().map_err(|source| {
                    SbomError::ListPackages {
                        source: PackageSource(source),
                    }
                })?;

            let timestamp = jiff::Timestamp::now()
                .strftime("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            let serial_number = format!("urn:uuid:{}", uuid::Uuid::new_v4());

            let debian_like = cfg!(any(feature = "debian", feature = "debian-pure"))
                && crate::core::env::distro::is_debian_like();

            let mut components = Vec::with_capacity(installed.len());
            let mut vulnerabilities = Vec::new();

            // Build component list
            for pkg in &installed {
                let version = pkg.version.version_string();
                let bom_ref = package_purl(&pkg.name, &version, debian_like);

                let component = SbomComponent {
                    component_type: "library".to_string(),
                    mime_type: None,
                    bom_ref: Some(bom_ref.clone()),
                    name: pkg.name.clone(),
                    version,
                    description: Some(pkg.description.clone()),
                    purl: Some(bom_ref.clone()),
                    licenses: pkg
                        .licenses
                        .iter()
                        .map(|license| SbomLicense {
                            license: Some(SbomLicenseInfo {
                                id: Some(license.clone()),
                                name: None,
                            }),
                            expression: None,
                        })
                        .collect(),
                    hashes: vec![],
                    external_references: vec![SbomExternalRef {
                        ref_type: "website".to_string(),
                        url: package_website(&pkg.name, debian_like),
                    }],
                    properties: None,
                };

                components.push(component);
            }
            // Dependency edges are intentionally not emitted: real
            // `dependsOn` resolution does not exist yet, and a CycloneDX
            // document full of empty dependency entries misstates the
            // system, so the `dependencies` array stays empty.

            // Scan for vulnerabilities if enabled. A failed fetch must not look like
            // a clean bill of materials. ALSA is Arch-specific; matching it against
            // dpkg names would report zero issues and look clean.
            if self.include_vulns {
                if debian_like {
                    return Err(SbomError::AlsaUnsupportedOnDebian);
                }
                let scanner = super::vulnerability::VulnerabilityScanner::new();
                let issues = scanner
                    .fetch_alsa_issues()
                    .await
                    .map_err(|source| SbomError::FetchVulnerabilities { source })?;
                for issue in issues {
                    for pkg_name in &issue.packages {
                        let Some(pkg) = installed.iter().find(|p| p.name == *pkg_name) else {
                            continue;
                        };
                        // Match the installed version against the advisory range
                        // exactly like `scan_system` does (W5-B-01): name-only
                        // matching listed every historical advisory for installed
                        // package names on fully patched systems.
                        if !advisory_applies(
                            &pkg.name,
                            &pkg.version.version_string(),
                            &issue.affected,
                            issue.fixed.as_deref(),
                        ) {
                            continue;
                        }
                        {
                            let bom_ref =
                                package_purl(&pkg.name, &pkg.version.version_string(), debian_like);

                            let severity = match issue.severity.to_lowercase().as_str() {
                                "critical" => Some("critical".to_string()),
                                "high" => Some("high".to_string()),
                                "medium" => Some("medium".to_string()),
                                "low" => Some("low".to_string()),
                                _ => None,
                            };

                            vulnerabilities.push(SbomVulnerability {
                                id: issue.name.clone(),
                                source: Some(SbomVulnSource {
                                    name: "Arch Linux Security Advisory".to_string(),
                                    url: Some("https://security.archlinux.org".to_string()),
                                }),
                                ratings: vec![SbomVulnRating {
                                    score: None,
                                    severity,
                                    method: Some("other".to_string()),
                                }],
                                description: Some(format!("Affected: {}", issue.affected)),
                                affects: vec![SbomVulnAffects {
                                    affects_ref: bom_ref,
                                }],
                            });
                        }
                    }
                }
            }

            let os = os_sbom_identity(debian_like);

            Ok(Sbom {
                bom_format: "CycloneDX".to_string(),
                spec_version: "1.5".to_string(),
                serial_number,
                version: 1,
                metadata: SbomMetadata {
                    timestamp,
                    tools: vec![SbomTool {
                        vendor: "OMG".to_string(),
                        name: "omg".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    }],
                    component: Some(SbomComponent {
                        component_type: "operating-system".to_string(),
                        mime_type: None,
                        bom_ref: Some(os.purl.to_string()),
                        name: os.name.to_string(),
                        version: os.version.to_string(),
                        description: Some(os.description.to_string()),
                        purl: Some(os.purl.to_string()),
                        licenses: vec![],
                        hashes: vec![],
                        external_references: vec![],
                        properties: None,
                    }),
                    manufacture: None,
                    supplier: Some(SbomOrganization {
                        name: os.supplier.to_string(),
                        url: Some(vec![os.supplier_url.to_string()]),
                    }),
                },
                components,
                dependencies: Vec::new(),
                vulnerabilities,
            })
        }
    }

    /// Export SBOM to JSON file (atomic replace, so a crash mid-write can
    /// never leave a truncated artifact)
    pub fn export_json<P: AsRef<Path>>(&self, sbom: &Sbom, path: P) -> Result<(), SbomError> {
        let path_str = path.as_ref().display().to_string();
        let json =
            serde_json::to_string_pretty(sbom).map_err(|source| SbomError::Serialize { source })?;
        crate::core::safe_ops::atomic_write_file_sync(path.as_ref(), json.as_bytes()).map_err(
            |error| SbomError::Write {
                path: path_str,
                source: io::Error::other(error),
            },
        )?;
        Ok(())
    }

    /// Export SBOM to default location
    pub fn export_default(&self, sbom: &Sbom) -> Result<std::path::PathBuf, SbomError> {
        let sbom_dir = paths::data_dir().join("sbom");
        std::fs::create_dir_all(&sbom_dir).map_err(|source| SbomError::CreateDir {
            path: sbom_dir.display().to_string(),
            source,
        })?;

        let timestamp = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string();
        let filename = format!("sbom-{timestamp}.json");
        let path = sbom_dir.join(&filename);

        self.export_json(sbom, &path)?;
        Ok(path)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    #[test]
    fn test_sbom_serialization() {
        let json = serde_json::to_string(&sample_sbom()).unwrap();
        assert!(json.contains("CycloneDX"));
        assert!(json.contains("1.5"));
    }

    fn sample_sbom() -> Sbom {
        Sbom {
            bom_format: "CycloneDX".to_string(),
            spec_version: "1.5".to_string(),
            serial_number: "urn:uuid:test".to_string(),
            version: 1,
            metadata: SbomMetadata {
                timestamp: "2026-01-16T00:00:00Z".to_string(),
                tools: vec![SbomTool {
                    vendor: "OMG".to_string(),
                    name: "omg".to_string(),
                    version: "0.1.0".to_string(),
                }],
                component: None,
                manufacture: None,
                supplier: None,
            },
            components: vec![],
            dependencies: vec![],
            vulnerabilities: vec![],
        }
    }

    #[test]
    fn export_json_fails_closed_when_path_is_a_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let error = SbomGenerator::new()
            .export_json(&sample_sbom(), temp.path())
            .expect_err("writing an SBOM over a directory must fail");
        assert!(matches!(error, SbomError::Write { .. }), "got: {error}");
    }

    #[test]
    fn sbom_without_backend_is_typed() {
        let error = SbomError::NoBackend;
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend"),
            "got: {error}"
        );
    }

    #[test]
    fn alsa_unsupported_on_debian_is_typed() {
        let error = SbomError::AlsaUnsupportedOnDebian;
        assert!(
            error
                .to_string()
                .contains("cannot be used to scan Debian packages"),
            "got: {error}"
        );
    }

    #[test]
    #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
    fn sbom_advisory_matching_respects_installed_version() {
        // Regression test for W5-B-01: SBOM advisory matching used the package
        // name only, so a fully patched system listed every historical advisory
        // for installed package names. A patched version must be excluded.
        // Historical ALSA advisory: affects from 1.1.1-1, fixed in 3.0.0-1.
        let affected = "1.1.1-1";
        let fixed = Some("3.0.0-1");

        assert!(
            advisory_applies("openssl", "1.1.1-1", affected, fixed),
            "version inside [affected, fixed) must be reported"
        );
        assert!(
            !advisory_applies("openssl", "3.2.1-1", affected, fixed),
            "patched version outside [affected, fixed) must not be reported"
        );

        // Missing fixed version means every release from `affected` onward
        // remains vulnerable, matching the scan_system precedent.
        assert!(advisory_applies("openssl", "4.0.0-1", affected, None));

        // ARCH-R14: unparseable advisory strings skip the pair instead of
        // fabricating a match. Non-Arch `parse_version` is infallible, so this
        // branch exists only on Arch (same gate as scan_system's tests).
        #[cfg(feature = "arch")]
        {
            assert!(!advisory_applies(
                "openssl",
                "1.1.1-1",
                "not a version",
                fixed
            ));
            assert!(!advisory_applies(
                "openssl",
                "1.1.1-1",
                affected,
                Some("not a version")
            ));
        }
    }

    #[test]
    #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
    fn debian_like_packages_use_deb_purl_and_debian_urls() {
        assert_eq!(
            package_purl("apt", "2.6.1", true),
            "pkg:deb/debian/apt@2.6.1"
        );
        assert_eq!(
            package_website("apt", true),
            "https://packages.debian.org/apt"
        );
    }

    #[test]
    #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
    fn arch_packages_keep_pacman_purl_and_arch_urls() {
        assert_eq!(
            package_purl("pacman", "7.0.0", false),
            "pkg:pacman/archlinux/pacman@7.0.0"
        );
        assert_eq!(
            package_website("pacman", false),
            "https://archlinux.org/packages/?name=pacman"
        );
    }

    #[test]
    #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
    fn debian_like_os_identity_is_not_arch() {
        let os = os_sbom_identity(true);
        assert_eq!(os.name, "Debian");
        assert_eq!(os.purl, "pkg:os/debian");
        assert_ne!(os.version, "rolling");
        assert_eq!(os.supplier, "Debian");
        assert_eq!(os.supplier_url, "https://www.debian.org");
    }

    #[test]
    #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
    fn arch_os_identity_stays_arch() {
        let os = os_sbom_identity(false);
        assert_eq!(os.name, "Arch Linux");
        assert_eq!(os.purl, "pkg:os/archlinux");
        assert_eq!(os.version, "rolling");
    }
}
