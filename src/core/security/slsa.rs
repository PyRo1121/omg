//! SLSA (Supply-chain Levels for Software Artifacts) provenance verification
//!
//! Verifies build provenance evidence and classifies SLSA levels (L0-L3) per
//! SLSA v1.0 specification for supply chain security attestation.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::http::shared_client;

/// Failures when talking to Rekor or parsing a log entry.
#[derive(Debug, Error)]
pub enum RekorError {
    #[error("Rekor HTTP request failed with status {status}")]
    HttpFailed { status: u16 },
    #[error("Rekor entry {uuid} missing required field {field}")]
    EntryMissingField { uuid: String, field: &'static str },
    #[error("Rekor entry {uuid} field {field} is not a u64")]
    EntryInvalidField { uuid: String, field: &'static str },
    #[error("Rekor response missing entry {uuid}")]
    EntryNotFound { uuid: String },
    #[error("Invalid Rekor response: empty entry map")]
    EmptyEntryMap,
}

/// Failures hashing artifacts or talking to Rekor.
#[derive(Debug, Error)]
pub enum SlsaError {
    #[error(transparent)]
    Rekor(#[from] RekorError),
    #[error("Failed to query Rekor")]
    RekorRequest {
        #[source]
        source: reqwest::Error,
    },
    #[error("Invalid Rekor index JSON")]
    RekorIndexJson {
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to get Rekor entry")]
    RekorEntryRequest {
        #[source]
        source: reqwest::Error,
    },
    #[error("Invalid Rekor entry JSON")]
    RekorEntryJson {
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to read '{path}'")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Invalid local provenance JSON")]
    ProvenanceParse {
        #[source]
        source: serde_json::Error,
    },
}

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

/// Verification result with detailed information.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// True only after full cryptographic attestation verification; the
    /// current evidence-based pipeline always reports `false`.
    pub verified: bool,
    /// Classified SLSA build level for the artifact.
    pub slsa_level: SlsaLevel,
    /// Rekor entry UUID when a transparency-log hit was found.
    pub transparency_log_entry: Option<String>,
    /// Builder identity claimed by matched provenance, when present.
    pub builder_id: Option<String>,
    /// Build timestamp from provenance metadata, when present.
    pub build_timestamp: Option<String>,
    /// Human-readable reason the artifact is not verified.
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
        Self::new()
    }
}

/// Evidence available to [`classify_slsa_evidence`]. None of these are a
/// completed in-toto/SLSA verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlsaEvidence {
    None,
    /// Local JSON parsed as provenance, with no signature check.
    UnsignedLocalJson,
    /// Rekor returned an index hit for the artifact hash.
    RekorIndexHit,
}

/// Map raw evidence to a result. A Rekor UUID or unsigned JSON file is not
/// SLSA L1/L2 and must not be reported as verified.
#[must_use]
pub fn classify_slsa_evidence(evidence: SlsaEvidence) -> VerificationResult {
    match evidence {
        SlsaEvidence::None => slsa_unverified("No verified SLSA provenance"),
        SlsaEvidence::UnsignedLocalJson => {
            slsa_unverified("Local provenance JSON is not a verified attestation")
        }
        SlsaEvidence::RekorIndexHit => {
            slsa_unverified("Rekor log index hit is not in-toto/SLSA verification")
        }
    }
}

fn slsa_unverified(error: &str) -> VerificationResult {
    VerificationResult {
        verified: false,
        slsa_level: SlsaLevel::None,
        transparency_log_entry: None,
        builder_id: None,
        build_timestamp: None,
        error: Some(error.to_string()),
    }
}

fn rekor_http_must_succeed(status: reqwest::StatusCode) -> Result<(), RekorError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(RekorError::HttpFailed {
            status: status.as_u16(),
        })
    }
}

fn required_u64(value: &Value, uuid: &str, field: &'static str) -> Result<u64, RekorError> {
    match value.get(field) {
        None => Err(RekorError::EntryMissingField {
            uuid: uuid.to_string(),
            field,
        }),
        Some(v) => v.as_u64().ok_or_else(|| RekorError::EntryInvalidField {
            uuid: uuid.to_string(),
            field,
        }),
    }
}

fn parse_rekor_entry(
    requested_uuid: &str,
    mut entry_map: HashMap<String, Value>,
) -> Result<RekorEntry, RekorError> {
    if entry_map.is_empty() {
        return Err(RekorError::EmptyEntryMap);
    }
    let value = entry_map
        .remove(requested_uuid)
        .ok_or_else(|| RekorError::EntryNotFound {
            uuid: requested_uuid.to_string(),
        })?;
    let log_index = required_u64(&value, requested_uuid, "logIndex")?;
    let integrated_time = required_u64(&value, requested_uuid, "integratedTime")?;
    let body = value
        .get("body")
        .and_then(Value::as_str)
        .ok_or_else(|| RekorError::EntryMissingField {
            uuid: requested_uuid.to_string(),
            field: "body",
        })?
        .to_string();
    Ok(RekorEntry {
        uuid: requested_uuid.to_string(),
        log_index,
        integrated_time,
        body,
    })
}

impl SlsaVerifier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: shared_client().clone(),
            rekor_url: "https://rekor.sigstore.dev".to_string(),
        }
    }

    /// Query Rekor transparency log for an artifact hash
    pub async fn query_rekor(&self, artifact_hash: &str) -> Result<Vec<RekorEntry>, SlsaError> {
        let url = format!("{}/api/v1/index/retrieve", self.rekor_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "hash": format!("sha256:{}", artifact_hash)
            }))
            .send()
            .await
            .map_err(|source| SlsaError::RekorRequest { source })?;

        rekor_http_must_succeed(response.status())?;

        let uuids: Vec<String> = response
            .json()
            .await
            .map_err(|source| SlsaError::RekorIndexJson { source })?;

        let mut entries = Vec::new();
        for uuid in uuids.iter().take(5) {
            entries.push(self.get_rekor_entry(uuid).await?);
        }

        Ok(entries)
    }

    /// Get a specific Rekor entry by UUID
    async fn get_rekor_entry(&self, uuid: &str) -> Result<RekorEntry, SlsaError> {
        let url = format!("{}/api/v1/log/entries/{}", self.rekor_url, uuid);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|source| SlsaError::RekorEntryRequest { source })?;

        rekor_http_must_succeed(response.status())?;

        let entry_map: HashMap<String, Value> = response
            .json()
            .await
            .map_err(|source| SlsaError::RekorEntryJson { source })?;
        Ok(parse_rekor_entry(uuid, entry_map)?)
    }

    /// Verify SLSA provenance for a package
    /// Gather provenance *evidence* for an artifact and classify it.
    ///
    /// This queries the Rekor transparency log and optionally parses a local
    /// provenance file, but neither is cryptographic verification: the
    /// returned [`VerificationResult`] currently always reports
    /// `verified: false` until full in-toto/SLSA verification lands. Callers
    /// must not treat an `Ok` result as a verified attestation.
    pub async fn verify_provenance(
        &self,
        blob_path: impl AsRef<Path>,
        provenance_path: Option<impl AsRef<Path>>,
    ) -> Result<VerificationResult, SlsaError> {
        // Calculate artifact hash
        let artifact_hash = Self::calculate_hash(&blob_path)?;

        // Check Rekor for transparency log entries
        let rekor_entries = self.query_rekor(&artifact_hash).await?;

        if let Some(entry) = rekor_entries.first() {
            let mut result = classify_slsa_evidence(SlsaEvidence::RekorIndexHit);
            result.transparency_log_entry = Some(entry.uuid.clone());
            return Ok(result);
        }

        if let Some(prov_path) = provenance_path
            && prov_path.as_ref().exists()
        {
            let path_str = prov_path.as_ref().display().to_string();
            let content =
                std::fs::read_to_string(prov_path.as_ref()).map_err(|source| SlsaError::Read {
                    path: path_str,
                    source,
                })?;
            let _: SlsaProvenance = serde_json::from_str(&content)
                .map_err(|source| SlsaError::ProvenanceParse { source })?;
            return Ok(classify_slsa_evidence(SlsaEvidence::UnsignedLocalJson));
        }

        Ok(classify_slsa_evidence(SlsaEvidence::None))
    }

    /// Calculate SHA-256 hash of a file
    fn calculate_hash<P: AsRef<Path>>(path: P) -> Result<String, SlsaError> {
        use std::io::Read as _;

        let path_str = path.as_ref().display().to_string();
        let mut hasher = Sha256::new();
        let mut file = std::fs::File::open(&path).map_err(|source| SlsaError::Read {
            path: path_str.clone(),
            source,
        })?;
        let mut buffer = [0u8; 8192];
        loop {
            let read = file.read(&mut buffer).map_err(|source| SlsaError::Read {
                path: path_str.clone(),
                source,
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /// Verify the SHA-256 hash of a file against an expected hex digest.
    ///
    /// The comparison accepts either hex case so uppercase digests from
    /// external attestations cannot silently mismatch.
    pub fn verify_hash<P: AsRef<Path>>(
        &self,
        path: P,
        expected_hash: &str,
    ) -> Result<bool, SlsaError> {
        let actual_hash = Self::calculate_hash(path)?;
        Ok(actual_hash.eq_ignore_ascii_case(expected_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
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
    fn calculate_hash_fails_closed_when_file_is_missing() {
        let err = SlsaVerifier::calculate_hash("/no/such/artifact.bin")
            .expect_err("a missing artifact must not produce a hash");
        assert!(matches!(err, SlsaError::Read { .. }), "got: {err}");
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
    fn test_slsa_verifier_default() {
        let verifier = SlsaVerifier::default();
        assert_eq!(verifier.rekor_url, "https://rekor.sigstore.dev");
    }

    #[test]
    fn rekor_index_hit_is_not_slsa_level2() {
        let result = classify_slsa_evidence(SlsaEvidence::RekorIndexHit);
        assert!(
            !result.verified,
            "a Rekor UUID is not a verified attestation"
        );
        assert_eq!(result.slsa_level, SlsaLevel::None);
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|e| e.contains("not in-toto")),
            "got: {:?}",
            result.error
        );
    }

    #[test]
    fn unsigned_local_json_is_not_slsa_level1() {
        let result = classify_slsa_evidence(SlsaEvidence::UnsignedLocalJson);
        assert!(!result.verified, "parsed JSON is not a signature check");
        assert_eq!(result.slsa_level, SlsaLevel::None);
    }

    #[test]
    fn missing_evidence_is_unverified() {
        let result = classify_slsa_evidence(SlsaEvidence::None);
        assert!(!result.verified);
        assert_eq!(result.slsa_level, SlsaLevel::None);
    }

    #[test]
    fn rekor_http_failure_is_an_error() {
        let err = rekor_http_must_succeed(reqwest::StatusCode::INTERNAL_SERVER_ERROR)
            .expect_err("HTTP failure must not look like no entry");
        assert!(
            matches!(err, RekorError::HttpFailed { status: 500 }),
            "got: {err}"
        );
        assert!(rekor_http_must_succeed(reqwest::StatusCode::OK).is_ok());
    }

    #[test]
    fn rekor_entry_missing_fields_is_an_error() {
        let mut incomplete = HashMap::new();
        incomplete.insert(
            "abc".to_string(),
            serde_json::json!({ "logIndex": 1, "body": "x" }),
        );
        let err =
            parse_rekor_entry("abc", incomplete).expect_err("missing integratedTime must fail");
        assert!(
            matches!(
                err,
                RekorError::EntryMissingField {
                    field: "integratedTime",
                    ..
                }
            ),
            "got: {err}"
        );
    }

    #[test]
    fn rekor_entry_non_u64_field_is_an_error() {
        let mut invalid = HashMap::new();
        invalid.insert(
            "abc".to_string(),
            serde_json::json!({ "logIndex": "1", "integratedTime": 2, "body": "x" }),
        );
        let err = parse_rekor_entry("abc", invalid).expect_err("string logIndex must fail");
        assert!(
            matches!(
                err,
                RekorError::EntryInvalidField {
                    field: "logIndex",
                    ..
                }
            ),
            "got: {err}"
        );
    }

    #[test]
    fn rekor_entry_wrong_uuid_is_an_error() {
        let mut other = HashMap::new();
        other.insert(
            "other".to_string(),
            serde_json::json!({ "logIndex": 1, "integratedTime": 2, "body": "x" }),
        );
        let err = parse_rekor_entry("wanted", other).expect_err("wrong uuid must fail");
        assert!(
            matches!(err, RekorError::EntryNotFound { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn empty_rekor_entry_map_is_an_error() {
        let err = parse_rekor_entry("abc", HashMap::new()).expect_err("empty map must fail");
        assert!(matches!(err, RekorError::EmptyEntryMap), "got: {err}");
    }
}
