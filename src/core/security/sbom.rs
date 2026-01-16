use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::core::paths;

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

/// SBOM Generator for enterprise compliance
pub struct SbomGenerator {
    include_vulns: bool,
    include_deps: bool,
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
            include_deps: true,
        }
    }

    #[must_use]
    pub const fn with_vulnerabilities(mut self, include: bool) -> Self {
        self.include_vulns = include;
        self
    }

    #[must_use]
    pub const fn with_dependencies(mut self, include: bool) -> Self {
        self.include_deps = include;
        self
    }

    /// Generate SBOM for all installed packages
    pub async fn generate_system_sbom(&self) -> Result<Sbom> {
        let installed = crate::package_managers::list_installed_fast()
            .map_err(|e| anyhow::anyhow!("Failed to list packages: {e}"))?;

        let timestamp = jiff::Zoned::now()
            .strftime("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let serial_number = format!("urn:uuid:{}", uuid::Uuid::new_v4());

        let mut components = Vec::with_capacity(installed.len());
        let mut vulnerabilities = Vec::new();
        let mut dependencies = Vec::new();

        // Build component list
        for pkg in &installed {
            let bom_ref = format!("pkg:pacman/archlinux/{}@{}", pkg.name, pkg.version);

            let component = SbomComponent {
                component_type: "library".to_string(),
                mime_type: None,
                bom_ref: Some(bom_ref.clone()),
                name: pkg.name.clone(),
                version: pkg.version.to_string(),
                description: Some(pkg.description.clone()),
                purl: Some(format!("pkg:pacman/archlinux/{}@{}", pkg.name, pkg.version)),
                licenses: vec![],
                hashes: vec![],
                external_references: vec![SbomExternalRef {
                    ref_type: "website".to_string(),
                    url: format!("https://archlinux.org/packages/?name={}", pkg.name),
                }],
                properties: None,
            };

            components.push(component);

            // Add dependency info if enabled
            if self.include_deps {
                dependencies.push(SbomDependency {
                    dep_ref: bom_ref,
                    depends_on: vec![], // Would need to resolve actual deps
                });
            }
        }

        // Scan for vulnerabilities if enabled
        if self.include_vulns {
            let scanner = super::vulnerability::VulnerabilityScanner::new();
            if let Ok(issues) = scanner.fetch_alsa_issues().await {
                for issue in issues {
                    for pkg_name in &issue.packages {
                        if let Some(pkg) = installed.iter().find(|p| p.name == *pkg_name) {
                            let bom_ref =
                                format!("pkg:pacman/archlinux/{}@{}", pkg.name, pkg.version);

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
        }

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
                    bom_ref: Some("pkg:os/archlinux".to_string()),
                    name: "Arch Linux".to_string(),
                    version: "rolling".to_string(),
                    description: Some("Arch Linux system".to_string()),
                    purl: Some("pkg:os/archlinux".to_string()),
                    licenses: vec![],
                    hashes: vec![],
                    external_references: vec![],
                    properties: None,
                }),
                manufacture: None,
                supplier: Some(SbomOrganization {
                    name: "Arch Linux".to_string(),
                    url: Some(vec!["https://archlinux.org".to_string()]),
                }),
            },
            components,
            dependencies,
            vulnerabilities,
        })
    }

    /// Export SBOM to JSON file
    pub fn export_json<P: AsRef<Path>>(&self, sbom: &Sbom, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(sbom)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Export SBOM to default location
    pub fn export_default(&self, sbom: &Sbom) -> Result<std::path::PathBuf> {
        let sbom_dir = paths::data_dir().join("sbom");
        std::fs::create_dir_all(&sbom_dir)?;

        let timestamp = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string();
        let filename = format!("sbom-{timestamp}.json");
        let path = sbom_dir.join(&filename);

        self.export_json(sbom, &path)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbom_serialization() {
        let sbom = Sbom {
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
        };

        let json = serde_json::to_string(&sbom).unwrap();
        assert!(json.contains("CycloneDX"));
        assert!(json.contains("1.5"));
    }
}
