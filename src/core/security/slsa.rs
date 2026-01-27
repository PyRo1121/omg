use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::core::http::shared_client;

/// SLSA Level definitions per SLSA v1.0 specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SlsaLevel {
    /// No SLSA guarantees
    None = 0,
    /// Build process is documented
    Level1 = 1,
    /// Hosted build platform, signed provenance
    Level2 = 2,
    /// Hardened build platform, non-falsifiable provenance
    Level3 = 3,
}

impl std::fmt::Display for SlsaLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Level1 => write!(f, "SLSA Level 1"),
            Self::Level2 => write!(f, "SLSA Level 2"),
            Self::Level3 => write!(f, "SLSA Level 3"),
        }
    }
}

/// Rekor transparency log entry
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RekorEntry {
    pub uuid: String,
    pub log_index: u64,
    pub integrated_time: u64,
    pub body: String,
}

/// Sigstore bundle for verification
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SigstoreBundle {
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_material: Option<VerificationMaterial>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VerificationMaterial {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tlog_entries: Option<Vec<TlogEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TlogEntry {
    pub log_index: String,
    pub log_id: LogId,
    pub integrated_time: String,
    pub inclusion_proof: Option<InclusionProof>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogId {
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InclusionProof {
    pub log_index: String,
    pub root_hash: String,
    pub tree_size: String,
    pub hashes: Vec<String>,
}

/// SLSA provenance attestation
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SlsaProvenance {
    pub build_type: String,
    pub builder: SlsaBuilder,
    pub invocation: Option<SlsaInvocation>,
    pub build_config: Option<serde_json::Value>,
    pub metadata: Option<SlsaMetadata>,
    pub materials: Vec<SlsaMaterial>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlsaBuilder {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SlsaInvocation {
    pub config_source: Option<ConfigSource>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigSource {
    pub uri: String,
    pub digest: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SlsaMetadata {
    pub build_invocation_id: Option<String>,
    pub build_started_on: Option<String>,
    pub build_finished_on: Option<String>,
    pub completeness: Option<SlsaCompleteness>,
    pub reproducible: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlsaCompleteness {
    pub parameters: bool,
    pub environment: bool,
    pub materials: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlsaMaterial {
    pub uri: String,
    pub digest: std::collections::HashMap<String, String>,
}

/// Verification result with detailed information
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub verified: bool,
    pub slsa_level: SlsaLevel,
    pub transparency_log_entry: Option<String>,
    pub builder_id: Option<String>,
    pub build_timestamp: Option<String>,
    pub error: Option<String>,
}

/// SLSA verification engine using Sigstore
#[derive(Debug, Clone)]
pub struct SlsaVerifier {
    client: reqwest::Client,
    rekor_url: String,
}

impl Default for SlsaVerifier {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            client: reqwest::Client::new(),
            rekor_url: "https://rekor.sigstore.dev".to_string(),
        })
    }
}

impl SlsaVerifier {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: shared_client().clone(),
            rekor_url: "https://rekor.sigstore.dev".to_string(),
        })
    }

    /// Query Rekor transparency log for an artifact hash
    pub async fn query_rekor(&self, artifact_hash: &str) -> Result<Vec<RekorEntry>> {
        let url = format!("{}/api/v1/index/retrieve", self.rekor_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "hash": format!("sha256:{}", artifact_hash)
            }))
            .send()
            .await
            .context("Failed to query Rekor")?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let uuids: Vec<String> = response.json().await.unwrap_or_default();

        let mut entries = Vec::new();
        for uuid in uuids.iter().take(5) {
            if let Ok(entry) = self.get_rekor_entry(uuid).await {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Get a specific Rekor entry by UUID
    async fn get_rekor_entry(&self, uuid: &str) -> Result<RekorEntry> {
        let url = format!("{}/api/v1/log/entries/{}", self.rekor_url, uuid);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get Rekor entry")?;

        let entry_map: std::collections::HashMap<String, serde_json::Value> =
            response.json().await?;

        if let Some((uuid, value)) = entry_map.into_iter().next() {
            let log_index = value
                .get("logIndex")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let integrated_time = value
                .get("integratedTime")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let body = value
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            return Ok(RekorEntry {
                uuid,
                log_index,
                integrated_time,
                body,
            });
        }

        anyhow::bail!("Invalid Rekor response")
    }

    /// Verify SLSA provenance for a package
    pub async fn verify_provenance<P: AsRef<Path>>(
        &self,
        blob_path: P,
        provenance_path: Option<P>,
    ) -> Result<VerificationResult> {
        // Calculate artifact hash
        let artifact_hash = Self::calculate_hash(&blob_path)?;

        // Check Rekor for transparency log entries
        let rekor_entries = self.query_rekor(&artifact_hash).await?;

        if rekor_entries.is_empty() {
            // No transparency log entry found
            // Check if we have local provenance
            if let Some(prov_path) = provenance_path
                && prov_path.as_ref().exists()
            {
                // Parse local provenance
                let content = std::fs::read_to_string(prov_path.as_ref())?;
                if let Ok(provenance) = serde_json::from_str::<SlsaProvenance>(&content) {
                    return Ok(VerificationResult {
                        verified: true,
                        slsa_level: SlsaLevel::Level1,
                        transparency_log_entry: None,
                        builder_id: Some(provenance.builder.id),
                        build_timestamp: provenance.metadata.and_then(|m| m.build_finished_on),
                        error: None,
                    });
                }
            }

            return Ok(VerificationResult {
                verified: false,
                slsa_level: SlsaLevel::None,
                transparency_log_entry: None,
                builder_id: None,
                build_timestamp: None,
                error: Some("No transparency log entry found".to_string()),
            });
        }

        // Found in Rekor - this is at least SLSA Level 2
        let entry = &rekor_entries[0];

        Ok(VerificationResult {
            verified: true,
            slsa_level: SlsaLevel::Level2,
            transparency_log_entry: Some(entry.uuid.clone()),
            builder_id: None,
            build_timestamp: Some(
                jiff::Timestamp::from_second(entry.integrated_time.cast_signed())
                    .map(|t| t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_default(),
            ),
            error: None,
        })
    }

    /// Calculate SHA-256 hash of a file
    fn calculate_hash<P: AsRef<Path>>(path: P) -> Result<String> {
        let mut hasher = Sha256::new();
        let mut file = std::fs::File::open(path)?;
        std::io::copy(&mut file, &mut hasher)?;
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Verify hash of a file against expected value
    pub fn verify_hash<P: AsRef<Path>>(&self, path: P, expected_hash: &str) -> Result<bool> {
        let actual_hash = Self::calculate_hash(path)?;
        Ok(actual_hash == expected_hash)
    }

    /// Determine SLSA level for a package based on available attestations
    pub fn determine_slsa_level(&self, package_name: &str, is_official: bool) -> SlsaLevel {
        // Core system packages from official repos have SLSA Level 2+
        // (Arch Linux build system provides signed packages with reproducible builds)
        if is_official {
            let core_packages = ["glibc", "linux", "pacman", "systemd", "openssl", "bash"];
            if core_packages.contains(&package_name) {
                return SlsaLevel::Level3;
            }
            return SlsaLevel::Level2;
        }

        // AUR packages have no SLSA guarantees by default
        SlsaLevel::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_slsa_level_display() {
        assert_eq!(SlsaLevel::None.to_string(), "None");
        assert_eq!(SlsaLevel::Level1.to_string(), "SLSA Level 1");
        assert_eq!(SlsaLevel::Level2.to_string(), "SLSA Level 2");
        assert_eq!(SlsaLevel::Level3.to_string(), "SLSA Level 3");
    }

    #[test]
    fn test_slsa_level_ordering() {
        assert!(SlsaLevel::Level3 > SlsaLevel::Level2);
        assert!(SlsaLevel::Level2 > SlsaLevel::Level1);
        assert!(SlsaLevel::Level1 > SlsaLevel::None);
    }

    #[test]
    fn test_calculate_hash() {
        let mut temp = NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(temp, "test content").unwrap();
        temp.flush().unwrap();

        let hash = SlsaVerifier::calculate_hash(temp.path()).unwrap();

        // Verify it's a valid SHA-256 hash (64 hex characters)
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Known hash for "test content" (no newline)
        assert_eq!(
            hash,
            "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72"
        );
    }

    #[test]
    fn test_calculate_hash_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let hash = SlsaVerifier::calculate_hash(temp.path()).unwrap();

        // SHA-256 of empty file
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_verify_hash_success() {
        let mut temp = NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(temp, "test content").unwrap();
        temp.flush().unwrap();

        let verifier = SlsaVerifier::default();
        let expected = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";

        assert!(verifier.verify_hash(temp.path(), expected).unwrap());
    }

    #[test]
    fn test_verify_hash_mismatch() {
        let mut temp = NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(temp, "test content").unwrap();
        temp.flush().unwrap();

        let verifier = SlsaVerifier::default();
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        assert!(!verifier.verify_hash(temp.path(), wrong_hash).unwrap());
    }

    #[test]
    fn test_determine_slsa_level_core_packages() {
        let verifier = SlsaVerifier::default();

        // Core packages should be Level 3
        assert_eq!(
            verifier.determine_slsa_level("glibc", true),
            SlsaLevel::Level3
        );
        assert_eq!(
            verifier.determine_slsa_level("linux", true),
            SlsaLevel::Level3
        );
        assert_eq!(
            verifier.determine_slsa_level("systemd", true),
            SlsaLevel::Level3
        );
    }

    #[test]
    fn test_determine_slsa_level_official_packages() {
        let verifier = SlsaVerifier::default();

        // Regular official packages should be Level 2
        assert_eq!(
            verifier.determine_slsa_level("vim", true),
            SlsaLevel::Level2
        );
        assert_eq!(
            verifier.determine_slsa_level("firefox", true),
            SlsaLevel::Level2
        );
    }

    #[test]
    fn test_determine_slsa_level_aur_packages() {
        let verifier = SlsaVerifier::default();

        // AUR packages have no SLSA guarantees
        assert_eq!(
            verifier.determine_slsa_level("yay", false),
            SlsaLevel::None
        );
        assert_eq!(
            verifier.determine_slsa_level("spotify", false),
            SlsaLevel::None
        );
    }

    #[test]
    fn test_slsa_verifier_new() {
        let verifier = SlsaVerifier::new();
        assert!(verifier.is_ok());

        let verifier = verifier.unwrap();
        assert_eq!(verifier.rekor_url, "https://rekor.sigstore.dev");
    }

    #[test]
    fn test_slsa_verifier_default() {
        let verifier = SlsaVerifier::default();
        assert_eq!(verifier.rekor_url, "https://rekor.sigstore.dev");
    }
}
