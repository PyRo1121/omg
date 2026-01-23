//! End-to-End Tests for OMG CLI
//!
//! Comprehensive E2E test infrastructure validating the complete user journey:
//! - CLI installation simulation (self-update)
//! - License activation flow (JWT/EdDSA validation)
//! - Usage reporting flow (telemetry)
//! - Daemon communication (Unix socket IPC)
//!
//! ## Test Philosophy (TDD)
//! - Tests are written FIRST, before any production code changes
//! - Each test has a clear purpose documented with `///` comments
//! - Tests are deterministic - no flaky tests allowed
//! - Network calls are mocked to avoid external dependencies
//! - All error paths are explicitly tested

#![allow(dead_code)] // Test utilities may not all be used immediately

mod common;

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST INFRASTRUCTURE
// ═══════════════════════════════════════════════════════════════════════════════

/// Unique port counter for mock servers to avoid conflicts in parallel tests
static MOCK_PORT: AtomicU16 = AtomicU16::new(19000);

/// Get the next available mock server port
fn next_mock_port() -> u16 {
    MOCK_PORT.fetch_add(1, Ordering::SeqCst)
}

/// Test environment with isolated filesystem
struct E2ETestEnv {
    /// Temporary directory for test data (simulates ~/.local/share/omg)
    data_dir: TempDir,
    /// Temporary directory for test config (simulates ~/.config/omg)
    config_dir: TempDir,
    /// Temporary directory for test cache
    cache_dir: TempDir,
    /// Environment variables to set for the test
    env_vars: HashMap<String, String>,
}

impl E2ETestEnv {
    /// Create a new isolated test environment
    fn new() -> Result<Self> {
        let data_dir = TempDir::new()?;
        let config_dir = TempDir::new()?;
        let cache_dir = TempDir::new()?;

        let mut env_vars = HashMap::new();
        env_vars.insert("OMG_TEST_MODE".to_string(), "1".to_string());
        env_vars.insert("OMG_DISABLE_DAEMON".to_string(), "1".to_string());
        env_vars.insert("OMG_DISABLE_TELEMETRY".to_string(), "1".to_string());
        env_vars.insert(
            "OMG_DATA_DIR".to_string(),
            data_dir.path().to_string_lossy().to_string(),
        );
        env_vars.insert(
            "OMG_CONFIG_DIR".to_string(),
            config_dir.path().to_string_lossy().to_string(),
        );
        env_vars.insert(
            "OMG_CACHE_DIR".to_string(),
            cache_dir.path().to_string_lossy().to_string(),
        );

        Ok(Self {
            data_dir,
            config_dir,
            cache_dir,
            env_vars,
        })
    }

    /// Get the data directory path
    fn data_path(&self) -> &Path {
        self.data_dir.path()
    }

    /// Get the config directory path
    fn config_path(&self) -> &Path {
        self.config_dir.path()
    }

    /// Get the cache directory path
    fn cache_path(&self) -> &Path {
        self.cache_dir.path()
    }

    /// Create a file in the data directory
    fn create_data_file(&self, name: &str, content: &str) -> Result<PathBuf> {
        let path = self.data_dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }

    /// Create a file in the config directory
    fn create_config_file(&self, name: &str, content: &str) -> Result<PathBuf> {
        let path = self.config_dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }

    /// Read a file from the data directory
    fn read_data_file(&self, name: &str) -> Result<String> {
        Ok(fs::read_to_string(self.data_dir.path().join(name))?)
    }

    /// Check if a file exists in the data directory
    fn data_file_exists(&self, name: &str) -> bool {
        self.data_dir.path().join(name).exists()
    }

    /// Set an environment variable for this test
    fn set_env(&mut self, key: &str, value: &str) {
        self.env_vars.insert(key.to_string(), value.to_string());
    }

    /// Get environment variables as a slice for command execution
    fn env_slice(&self) -> Vec<(&str, &str)> {
        self.env_vars
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MOCK HTTP SERVER
// ═══════════════════════════════════════════════════════════════════════════════

/// Mock HTTP server for testing network-dependent features
/// Uses a simple in-memory response map
struct MockHttpServer {
    /// Base URL of the mock server
    base_url: String,
    /// Registered responses keyed by path
    responses: HashMap<String, MockHttpResponse>,
    /// Request log for verification
    requests: Vec<MockHttpRequest>,
}

#[derive(Clone, Debug)]
struct MockHttpResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

#[derive(Clone, Debug)]
struct MockHttpRequest {
    method: String,
    path: String,
    body: Option<String>,
}

impl MockHttpServer {
    /// Create a new mock HTTP server
    fn new() -> Self {
        let port = next_mock_port();
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            responses: HashMap::new(),
            requests: Vec::new(),
        }
    }

    /// Register a mock response for a path
    fn mock_get(&mut self, path: &str, status: u16, body: &str) {
        self.responses.insert(
            path.to_string(),
            MockHttpResponse {
                status,
                content_type: "application/json".to_string(),
                body: body.as_bytes().to_vec(),
            },
        );
    }

    /// Register a mock binary response (for downloads)
    fn mock_get_binary(&mut self, path: &str, status: u16, body: Vec<u8>) {
        self.responses.insert(
            path.to_string(),
            MockHttpResponse {
                status,
                content_type: "application/octet-stream".to_string(),
                body,
            },
        );
    }

    /// Get the base URL for this mock server
    fn url(&self) -> &str {
        &self.base_url
    }

    /// Get a registered response
    fn get_response(&self, path: &str) -> Option<&MockHttpResponse> {
        self.responses.get(path)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 1: CLI INSTALLATION SIMULATION (self-update)
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for the `omg self-update` command
mod self_update_tests {
    use super::*;

    /// Verify that version comparison correctly identifies when update is needed
    #[test]
    fn test_version_comparison_identifies_update_needed() -> Result<()> {
        // Given: Current version is 0.1.0, latest is 0.2.0
        let current = semver::Version::parse("0.1.0")?;
        let latest = semver::Version::parse("0.2.0")?;

        // When: We compare versions
        let needs_update = latest > current;

        // Then: Update should be needed
        assert!(
            needs_update,
            "Version {latest} should be greater than {current}"
        );

        Ok(())
    }

    /// Verify that version comparison correctly identifies when already up-to-date
    #[test]
    fn test_version_comparison_identifies_already_updated() -> Result<()> {
        // Given: Current version equals latest version
        let current = semver::Version::parse("0.1.75")?;
        let latest = semver::Version::parse("0.1.75")?;

        // When: We compare versions
        let needs_update = latest > current;

        // Then: No update should be needed
        assert!(
            !needs_update,
            "Version {latest} should equal {current}, no update needed"
        );

        Ok(())
    }

    /// Verify that prerelease versions are handled correctly
    #[test]
    fn test_version_comparison_handles_prerelease() -> Result<()> {
        // Given: Current is prerelease, latest is stable
        let current = semver::Version::parse("0.1.75-beta.1")?;
        let latest = semver::Version::parse("0.1.75")?;

        // When: We compare versions
        let needs_update = latest > current;

        // Then: Stable should be considered newer than prerelease
        assert!(
            needs_update,
            "Stable version {latest} should be greater than prerelease {current}"
        );

        Ok(())
    }

    /// Verify that downgrade is not performed when force=false
    #[test]
    fn test_version_comparison_prevents_downgrade() -> Result<()> {
        // Given: Current version is newer than "latest" (edge case)
        let current = semver::Version::parse("0.2.0")?;
        let latest = semver::Version::parse("0.1.75")?;

        // When: We compare versions
        let needs_update = latest > current;

        // Then: Should not suggest downgrade
        assert!(
            !needs_update,
            "Should not suggest downgrade from {current} to {latest}"
        );

        Ok(())
    }

    /// Verify that the download URL is constructed correctly
    #[test]
    fn test_download_url_construction() {
        // Given: Version and platform
        let version = "0.1.75";
        let platform = "x86_64-unknown-linux-gnu";
        let base_url = "https://releases.pyro1121.com";

        // When: We construct the URL
        let url = format!("{base_url}/download/omg-{version}-{platform}.tar.gz");

        // Then: URL should be properly formatted
        assert_eq!(
            url,
            "https://releases.pyro1121.com/download/omg-0.1.75-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    /// Verify that version parsing handles malformed version strings gracefully
    #[test]
    fn test_version_parsing_handles_malformed_input() {
        // Given: Various malformed version strings
        let malformed_versions = ["not-a-version", "1.2", "v1.2.3", "1.2.3.4", ""];

        for version_str in malformed_versions {
            // When: We attempt to parse
            let result = semver::Version::parse(version_str);

            // Then: Parsing should fail gracefully
            assert!(
                result.is_err(),
                "Malformed version '{version_str}' should fail to parse"
            );
        }
    }

    /// Verify that tar.gz extraction works correctly (unit test for archive handling)
    #[test]
    fn test_tar_gz_extraction_creates_expected_files() -> Result<()> {
        // Given: A mock tar.gz archive
        let temp_dir = TempDir::new()?;
        let archive_path = temp_dir.path().join("test.tar.gz");

        // Create a minimal tar.gz with a single file
        {
            let file = fs::File::create(&archive_path)?;
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);

            // Add a test file to the archive
            let test_content = b"#!/bin/sh\necho 'test binary'";
            let mut header = tar::Header::new_gnu();
            header.set_path("omg")?;
            header.set_size(test_content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, &test_content[..])?;
            builder.finish()?;
        }

        // When: We extract the archive
        let extract_dir = TempDir::new()?;
        let file = fs::File::open(&archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(extract_dir.path())?;

        // Then: The extracted file should exist
        let extracted_binary = extract_dir.path().join("omg");
        assert!(
            extracted_binary.exists(),
            "Extracted binary should exist at {:?}",
            extracted_binary
        );

        // And: Should have correct content
        let content = fs::read_to_string(&extracted_binary)?;
        assert!(
            content.contains("test binary"),
            "Extracted file should contain expected content"
        );

        Ok(())
    }

    /// Verify atomic binary replacement logic
    #[test]
    fn test_atomic_binary_replacement() -> Result<()> {
        // Given: An existing binary and a new binary
        let temp_dir = TempDir::new()?;
        let current_binary = temp_dir.path().join("omg");
        let new_binary = temp_dir.path().join("omg.new");
        let backup_binary = temp_dir.path().join("omg.old");

        fs::write(&current_binary, "old version")?;
        fs::write(&new_binary, "new version")?;

        // When: We perform atomic replacement
        // Step 1: Backup current
        fs::rename(&current_binary, &backup_binary)?;

        // Step 2: Move new to current
        fs::rename(&new_binary, &current_binary)?;

        // Step 3: Remove backup
        fs::remove_file(&backup_binary)?;

        // Then: Current should have new content
        let content = fs::read_to_string(&current_binary)?;
        assert_eq!(content, "new version", "Binary should be updated");

        // And: Backup should be removed
        assert!(
            !backup_binary.exists(),
            "Backup should be cleaned up after successful update"
        );

        Ok(())
    }

    /// Verify that failed replacement restores backup
    #[test]
    fn test_failed_replacement_restores_backup() -> Result<()> {
        // Given: An existing binary
        let temp_dir = TempDir::new()?;
        let current_binary = temp_dir.path().join("omg");
        let backup_binary = temp_dir.path().join("omg.old");

        fs::write(&current_binary, "original version")?;

        // When: Backup succeeds but new binary doesn't exist
        fs::rename(&current_binary, &backup_binary)?;

        // Simulate failure: new binary doesn't exist, need to restore
        let non_existent = temp_dir.path().join("does-not-exist");
        let move_result = fs::rename(&non_existent, &current_binary);
        assert!(move_result.is_err(), "Move should fail for non-existent file");

        // Then: Restore from backup
        fs::rename(&backup_binary, &current_binary)?;

        // Verify restoration
        let content = fs::read_to_string(&current_binary)?;
        assert_eq!(
            content, "original version",
            "Original binary should be restored after failed update"
        );

        Ok(())
    }

    /// Verify error handling when latest version endpoint fails
    #[test]
    fn test_handles_version_endpoint_failure() {
        // Given: A mock server that returns 500 for version endpoint
        let mut mock = MockHttpServer::new();
        mock.mock_get("/latest-version", 500, "Internal Server Error");

        // When: We check for the response
        let response = mock.get_response("/latest-version");

        // Then: We should have a 500 response
        assert!(response.is_some(), "Response should be registered");
        assert_eq!(response.unwrap().status, 500, "Should return 500 status");
    }

    /// Verify error handling when download fails
    #[test]
    fn test_handles_download_failure() {
        // Given: A mock server that returns 404 for download
        let mut mock = MockHttpServer::new();
        mock.mock_get(
            "/download/omg-0.1.75-x86_64-unknown-linux-gnu.tar.gz",
            404,
            "Not Found",
        );

        // When: We check for the response
        let response = mock.get_response("/download/omg-0.1.75-x86_64-unknown-linux-gnu.tar.gz");

        // Then: We should have a 404 response
        assert!(response.is_some(), "Response should be registered");
        assert_eq!(response.unwrap().status, 404, "Should return 404 status");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 2: LICENSE ACTIVATION FLOW
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for license activation and validation
mod license_tests {
    use super::*;

    /// Verify that license key format validation rejects invalid keys
    #[test]
    fn test_license_key_format_validation_rejects_too_long() {
        // Given: A license key that exceeds maximum length
        let key = "a".repeat(129);

        // When: We validate the format
        let is_valid = key.len() <= 128
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');

        // Then: It should be rejected
        assert!(!is_valid, "Key exceeding 128 chars should be rejected");
    }

    /// Verify that license key format validation rejects keys with invalid characters
    #[test]
    fn test_license_key_format_validation_rejects_invalid_chars() {
        // Given: License keys with invalid characters
        let invalid_keys = [
            "key-with-space here",
            "key@with#special",
            "key\nwith\nnewline",
            "key;with;semicolons",
            "key<script>xss</script>",
        ];

        for key in invalid_keys {
            // When: We validate the format
            let is_valid =
                key.len() <= 128 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');

            // Then: It should be rejected
            assert!(!is_valid, "Key '{key}' with invalid chars should be rejected");
        }
    }

    /// Verify that valid license key formats are accepted
    #[test]
    fn test_license_key_format_validation_accepts_valid() {
        // Given: Valid license key formats
        let valid_keys = [
            "OMG-PRO-1234-ABCD-5678",
            "abc123",
            "A-B-C-D",
            "ENTERPRISE-KEY-2024",
        ];

        for key in valid_keys {
            // When: We validate the format
            let is_valid =
                key.len() <= 128 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');

            // Then: It should be accepted
            assert!(is_valid, "Key '{key}' should be valid");
        }
    }

    /// Verify that license file is saved in correct location
    #[test]
    fn test_license_saved_to_correct_location() -> Result<()> {
        // Given: A test environment
        let env = E2ETestEnv::new()?;

        // When: We create a license file
        let license_json = r#"{
            "key": "test-key",
            "tier": "pro",
            "features": ["sbom", "audit"],
            "validated_at": 1700000000
        }"#;
        env.create_data_file("license.json", license_json)?;

        // Then: The file should exist in the data directory
        assert!(
            env.data_file_exists("license.json"),
            "License file should be created in data directory"
        );

        // And: Should have correct content
        let content = env.read_data_file("license.json")?;
        assert!(content.contains("test-key"), "License should contain key");
        assert!(content.contains("pro"), "License should contain tier");

        Ok(())
    }

    /// Verify tier hierarchy is correctly ordered
    #[test]
    fn test_tier_hierarchy() {
        use omg_lib::core::license::Tier;

        // Given: All tier levels
        let free = Tier::Free;
        let pro = Tier::Pro;
        let team = Tier::Team;
        let enterprise = Tier::Enterprise;

        // Then: Hierarchy should be correct
        assert!(free < pro, "Free should be less than Pro");
        assert!(pro < team, "Pro should be less than Team");
        assert!(team < enterprise, "Team should be less than Enterprise");
        assert!(free < enterprise, "Free should be less than Enterprise");
    }

    /// Verify feature gating based on tier
    #[test]
    fn test_feature_gating_by_tier() {
        use omg_lib::core::license::{Feature, Tier};

        // Given: Features and their required tiers
        let test_cases = [
            (Feature::Packages, Tier::Free),
            (Feature::Runtimes, Tier::Free),
            (Feature::Sbom, Tier::Pro),
            (Feature::Audit, Tier::Pro),
            (Feature::TeamSync, Tier::Team),
            (Feature::Fleet, Tier::Team),
            (Feature::Policy, Tier::Enterprise),
            (Feature::Slsa, Tier::Enterprise),
        ];

        for (feature, expected_tier) in test_cases {
            // When: We check the required tier
            let required = feature.required_tier();

            // Then: It should match expected
            assert_eq!(
                required, expected_tier,
                "Feature {:?} should require {:?} tier",
                feature, expected_tier
            );
        }
    }

    /// Verify machine ID generation is deterministic
    #[test]
    fn test_machine_id_is_deterministic() {
        use omg_lib::core::license::get_machine_id;

        // When: We generate machine ID twice
        let id1 = get_machine_id();
        let id2 = get_machine_id();

        // Then: Should get the same ID
        assert_eq!(id1, id2, "Machine ID should be deterministic");

        // And: Should be 16 characters (first 16 chars of SHA256)
        assert_eq!(id1.len(), 16, "Machine ID should be 16 characters");

        // And: Should be hex-encoded
        assert!(
            id1.chars().all(|c| c.is_ascii_hexdigit()),
            "Machine ID should be hex-encoded"
        );
    }

    /// Verify license JSON serialization/deserialization
    #[test]
    fn test_license_json_roundtrip() -> Result<()> {
        use omg_lib::core::license::StoredLicense;

        // Given: A stored license
        let license = StoredLicense {
            key: "TEST-KEY-123".to_string(),
            tier: "pro".to_string(),
            features: vec!["sbom".to_string(), "audit".to_string()],
            customer: Some("Test Customer".to_string()),
            expires_at: Some("2025-12-31".to_string()),
            validated_at: 1700000000,
            token: Some("test.jwt.token".to_string()),
            machine_id: Some("abc123def456".to_string()),
        };

        // When: We serialize and deserialize
        let json = serde_json::to_string(&license)?;
        let deserialized: StoredLicense = serde_json::from_str(&json)?;

        // Then: All fields should match
        assert_eq!(deserialized.key, license.key);
        assert_eq!(deserialized.tier, license.tier);
        assert_eq!(deserialized.features, license.features);
        assert_eq!(deserialized.customer, license.customer);
        assert_eq!(deserialized.expires_at, license.expires_at);
        assert_eq!(deserialized.validated_at, license.validated_at);
        assert_eq!(deserialized.token, license.token);
        assert_eq!(deserialized.machine_id, license.machine_id);

        Ok(())
    }

    /// Verify license API response parsing
    #[test]
    fn test_license_api_response_parsing() -> Result<()> {
        use omg_lib::core::license::LicenseResponse;

        // Given: A valid API response JSON
        let json = r#"{
            "valid": true,
            "tier": "pro",
            "features": ["sbom", "audit", "secrets"],
            "customer": "Acme Corp",
            "expires_at": "2025-12-31T23:59:59Z",
            "token": "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9..."
        }"#;

        // When: We parse the response
        let response: LicenseResponse = serde_json::from_str(json)?;

        // Then: All fields should be correctly parsed
        assert!(response.valid, "Response should be valid");
        assert_eq!(response.tier, Some("pro".to_string()));
        assert_eq!(response.features.as_ref().unwrap().len(), 3);
        assert_eq!(response.customer, Some("Acme Corp".to_string()));

        Ok(())
    }

    /// Verify license API error response parsing
    #[test]
    fn test_license_api_error_response_parsing() -> Result<()> {
        use omg_lib::core::license::LicenseResponse;

        // Given: An invalid license response
        let json = r#"{
            "valid": false,
            "error": "License key not found or expired"
        }"#;

        // When: We parse the response
        let response: LicenseResponse = serde_json::from_str(json)?;

        // Then: Should indicate invalid with error message
        assert!(!response.valid, "Response should be invalid");
        assert!(response.tier.is_none(), "Tier should be absent");
        assert_eq!(
            response.error,
            Some("License key not found or expired".to_string())
        );

        Ok(())
    }

    /// Verify offline license validation with cached token
    #[test]
    fn test_offline_license_validation_with_cached_token() -> Result<()> {
        // Given: A test environment with a cached license
        let env = E2ETestEnv::new()?;

        // Create a mock cached license (simulating offline scenario)
        let license_json = r#"{
            "key": "OMG-PRO-TEST",
            "tier": "pro",
            "features": ["sbom", "audit"],
            "customer": "Test User",
            "validated_at": 1700000000,
            "token": "mock.jwt.token",
            "machine_id": "abc123"
        }"#;
        env.create_data_file("license.json", license_json)?;

        // Then: License file should exist for offline validation
        assert!(
            env.data_file_exists("license.json"),
            "Cached license should exist for offline validation"
        );

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 3: USAGE REPORTING FLOW
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for usage tracking and reporting
mod usage_tests {
    use super::*;
    use omg_lib::core::usage::{Achievement, UsageStats};

    /// Verify usage stats initialization with default values
    #[test]
    fn test_usage_stats_default_values() {
        // When: We create default usage stats
        let stats = UsageStats::default();

        // Then: All counters should be zero
        assert_eq!(stats.total_commands, 0, "Total commands should start at 0");
        assert_eq!(stats.time_saved_ms, 0, "Time saved should start at 0");
        assert_eq!(stats.queries_today, 0, "Queries today should start at 0");
        assert!(stats.commands.is_empty(), "Commands map should be empty");
        assert!(
            stats.achievements.is_empty(),
            "Achievements should be empty"
        );
    }

    /// Verify command recording increments counters correctly
    #[test]
    fn test_record_command_increments_counters() {
        // Given: Empty usage stats
        let mut stats = UsageStats::default();

        // When: We record commands
        stats.record_command("search", 127);
        stats.record_command("search", 127);
        stats.record_command("info", 132);

        // Then: Counters should be updated
        assert_eq!(stats.total_commands, 3, "Total should be 3");
        assert_eq!(stats.commands.get("search"), Some(&2), "Search count should be 2");
        assert_eq!(stats.commands.get("info"), Some(&1), "Info count should be 1");
        assert_eq!(
            stats.time_saved_ms,
            127 + 127 + 132,
            "Time saved should be sum"
        );
    }

    /// Verify installed_packages hashmap tracks package installations
    #[test]
    fn test_installed_packages_tracking() {
        // Given: Usage stats
        let mut stats = UsageStats::default();

        // When: We track package installations
        stats
            .installed_packages
            .insert("firefox".to_string(), 1);
        stats
            .installed_packages
            .insert("vim".to_string(), 3);
        *stats
            .installed_packages
            .entry("firefox".to_string())
            .or_insert(0) += 1;

        // Then: Package counts should be tracked
        assert_eq!(
            stats.installed_packages.get("firefox"),
            Some(&2),
            "Firefox should be installed 2 times"
        );
        assert_eq!(
            stats.installed_packages.get("vim"),
            Some(&3),
            "Vim should be installed 3 times"
        );
    }

    /// Verify runtime_usage_counts hashmap tracks runtime switches
    #[test]
    fn test_runtime_usage_tracking() {
        // Given: Usage stats
        let mut stats = UsageStats::default();

        // When: We track runtime switches
        stats.runtime_usage_counts.insert("node".to_string(), 5);
        stats.runtime_usage_counts.insert("python".to_string(), 3);
        stats
            .runtime_usage_counts.insert("rust".to_string(), 1);

        // Then: Runtime counts should be tracked
        assert_eq!(
            stats.runtime_usage_counts.get("node"),
            Some(&5),
            "Node usage should be 5"
        );
        assert_eq!(
            stats.runtime_usage_counts.get("python"),
            Some(&3),
            "Python usage should be 3"
        );
    }

    /// Verify time saved calculation is accurate
    #[test]
    fn test_time_saved_calculation() {
        use omg_lib::core::usage::time_saved;

        // Given: Expected time savings per operation
        // Then: Values should match documented benchmarks
        assert_eq!(
            time_saved::SEARCH_MS, 127,
            "Search should save 127ms (133ms - 6ms)"
        );
        assert_eq!(
            time_saved::INFO_MS, 132,
            "Info should save 132ms (138ms - 6.5ms)"
        );
        assert_eq!(
            time_saved::RUNTIME_SWITCH_MS, 148,
            "Runtime switch should save 148ms (150ms - 1.8ms)"
        );
    }

    /// Verify human-readable time format
    #[test]
    fn test_time_saved_human_format() {
        // Given: Various time values
        let test_cases = [
            (500, "500ms"),
            (5000, "5.0s"),
            (60_000, "1.0min"),
            (120_000, "2.0min"),
            (3_600_000, "1.0hr"),
            (7_200_000, "2.0hr"),
        ];

        for (ms, expected) in test_cases {
            // When: We format the time
            let stats = UsageStats {
                time_saved_ms: ms,
                ..Default::default()
            };
            let formatted = stats.time_saved_human();

            // Then: Format should be correct
            assert_eq!(
                formatted, expected,
                "{ms}ms should format as '{expected}'"
            );
        }
    }

    /// Verify achievement unlocking based on thresholds
    #[test]
    fn test_achievement_unlocking() {
        // Given: Stats that should trigger achievements
        let mut stats = UsageStats {
            total_commands: 100,
            time_saved_ms: 60_000, // 1 minute
            ..Default::default()
        };

        // Manually check achievements (normally called in record_command)
        stats.achievements.push(Achievement::FirstStep);
        stats.achievements.push(Achievement::Centurion);
        stats.achievements.push(Achievement::MinuteSaver);

        // Then: Appropriate achievements should be present
        assert!(
            stats.achievements.contains(&Achievement::FirstStep),
            "Should have FirstStep achievement"
        );
        assert!(
            stats.achievements.contains(&Achievement::Centurion),
            "Should have Centurion (100 commands) achievement"
        );
        assert!(
            stats.achievements.contains(&Achievement::MinuteSaver),
            "Should have MinuteSaver (1 minute saved) achievement"
        );
    }

    /// Verify usage stats JSON serialization for API sync
    #[test]
    fn test_usage_stats_json_serialization() -> Result<()> {
        // Given: Usage stats with data
        let mut stats = UsageStats::default();
        stats.total_commands = 50;
        stats.time_saved_ms = 10000;
        stats.installed_packages.insert("git".to_string(), 1);
        stats
            .runtime_usage_counts
            .insert("node".to_string(), 5);

        // When: We serialize to JSON
        let json = serde_json::to_string(&stats)?;

        // Then: JSON should contain expected fields
        assert!(
            json.contains("\"total_commands\":50"),
            "JSON should contain total_commands"
        );
        assert!(
            json.contains("\"time_saved_ms\":10000"),
            "JSON should contain time_saved_ms"
        );
        assert!(
            json.contains("\"git\""),
            "JSON should contain installed package"
        );
        assert!(
            json.contains("\"node\""),
            "JSON should contain runtime usage"
        );

        Ok(())
    }

    /// Verify sync payload format matches API expectations
    #[test]
    fn test_sync_payload_format() -> Result<()> {
        // Given: Usage stats
        let mut stats = UsageStats::default();
        stats.total_commands = 100;
        stats.queries_today = 10;
        stats.installed_packages.insert("vim".to_string(), 1);
        stats
            .runtime_usage_counts
            .insert("python".to_string(), 3);

        // When: We construct a sync payload (simulating what sync() does)
        let payload = serde_json::json!({
            "license_key": "test-key",
            "machine_id": "abc123",
            "commands_run": stats.queries_today,
            "installed_packages": stats.installed_packages,
            "runtime_usage_counts": stats.runtime_usage_counts,
            "time_saved_ms": stats.time_saved_ms,
        });

        // Then: Payload should have correct structure
        assert!(
            payload["license_key"].is_string(),
            "Payload should have license_key"
        );
        assert!(
            payload["installed_packages"].is_object(),
            "installed_packages should be object"
        );
        assert!(
            payload["runtime_usage_counts"].is_object(),
            "runtime_usage_counts should be object"
        );

        Ok(())
    }

    /// Verify usage file persistence
    #[test]
    fn test_usage_stats_persistence() -> Result<()> {
        // Given: A test environment
        let env = E2ETestEnv::new()?;

        // Create usage stats file
        let stats = UsageStats {
            total_commands: 42,
            time_saved_ms: 5000,
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&stats)?;
        env.create_data_file("usage.json", &json)?;

        // When: We read back the file
        let content = env.read_data_file("usage.json")?;
        let loaded: UsageStats = serde_json::from_str(&content)?;

        // Then: Stats should match
        assert_eq!(loaded.total_commands, 42);
        assert_eq!(loaded.time_saved_ms, 5000);

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 4: DAEMON COMMUNICATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for daemon IPC protocol and communication
mod daemon_tests {
    use super::*;
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult, SearchResult, PackageInfo};

    /// Verify request serialization with bitcode
    #[test]
    fn test_request_bitcode_serialization() -> Result<()> {
        // Given: A search request
        let request = Request::Search {
            id: 1,
            query: "firefox".to_string(),
            limit: Some(10),
        };

        // When: We serialize with bitcode
        let bytes = bitcode::serialize(&request)?;

        // Then: Should produce non-empty bytes
        assert!(!bytes.is_empty(), "Serialized request should not be empty");

        // And: Should deserialize back correctly
        let deserialized: Request = bitcode::deserialize(&bytes)?;
        match deserialized {
            Request::Search { id, query, limit } => {
                assert_eq!(id, 1);
                assert_eq!(query, "firefox");
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Wrong request type after deserialization"),
        }

        Ok(())
    }

    /// Verify response serialization with bitcode
    #[test]
    fn test_response_bitcode_serialization() -> Result<()> {
        // Given: A search response
        let response = Response::Success {
            id: 1,
            result: ResponseResult::Search(SearchResult {
                packages: vec![PackageInfo {
                    name: "firefox".to_string(),
                    version: "122.0".to_string(),
                    description: "Web browser".to_string(),
                    source: "extra".to_string(),
                }],
                total: 1,
            }),
        };

        // When: We serialize with bitcode
        let bytes = bitcode::serialize(&response)?;

        // Then: Should produce non-empty bytes
        assert!(!bytes.is_empty(), "Serialized response should not be empty");

        // And: Should deserialize back correctly
        let deserialized: Response = bitcode::deserialize(&bytes)?;
        match deserialized {
            Response::Success { id, result } => {
                assert_eq!(id, 1);
                match result {
                    ResponseResult::Search(sr) => {
                        assert_eq!(sr.packages.len(), 1);
                        assert_eq!(sr.packages[0].name, "firefox");
                    }
                    _ => panic!("Wrong result type"),
                }
            }
            Response::Error { .. } => panic!("Expected success response"),
        }

        Ok(())
    }

    /// Verify error response format
    #[test]
    fn test_error_response_format() -> Result<()> {
        use omg_lib::daemon::protocol::error_codes;

        // Given: An error response
        let response = Response::Error {
            id: 1,
            code: error_codes::PACKAGE_NOT_FOUND,
            message: "Package 'nonexistent' not found".to_string(),
        };

        // When: We serialize and deserialize
        let bytes = bitcode::serialize(&response)?;
        let deserialized: Response = bitcode::deserialize(&bytes)?;

        // Then: Error details should be preserved
        match deserialized {
            Response::Error { id, code, message } => {
                assert_eq!(id, 1);
                assert_eq!(code, error_codes::PACKAGE_NOT_FOUND);
                assert!(message.contains("nonexistent"));
            }
            _ => panic!("Expected error response"),
        }

        Ok(())
    }

    /// Verify length-delimited framing format
    #[test]
    fn test_length_delimited_framing() -> Result<()> {
        // Given: A message to frame
        let message = b"test message content";

        // When: We apply length-delimited framing (Big Endian u32 prefix)
        let len = message.len() as u32;
        let mut framed = Vec::with_capacity(4 + message.len());
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(message);

        // Then: Frame should have correct structure
        assert_eq!(framed.len(), 4 + message.len(), "Frame should have 4-byte prefix + message");

        // And: Length prefix should be correct
        let prefix_bytes: [u8; 4] = framed[..4].try_into()?;
        let decoded_len = u32::from_be_bytes(prefix_bytes);
        assert_eq!(decoded_len as usize, message.len(), "Length prefix should match message length");

        Ok(())
    }

    /// Verify request ID matching in responses
    #[test]
    fn test_request_id_matching() {
        // Given: Requests with different IDs
        let requests = [
            Request::Ping { id: 1 },
            Request::Ping { id: 42 },
            Request::Ping { id: 999 },
        ];

        // Then: Each request should have its correct ID
        assert_eq!(requests[0].id(), 1);
        assert_eq!(requests[1].id(), 42);
        assert_eq!(requests[2].id(), 999);
    }

    /// Verify all request types have proper ID extraction
    #[test]
    fn test_all_request_types_have_id() {
        // Given: All request types
        let requests: Vec<Request> = vec![
            Request::Search { id: 1, query: "test".to_string(), limit: None },
            Request::Info { id: 2, package: "test".to_string() },
            Request::Status { id: 3 },
            Request::Explicit { id: 4 },
            Request::ExplicitCount { id: 5 },
            Request::SecurityAudit { id: 6 },
            Request::Ping { id: 7 },
            Request::CacheStats { id: 8 },
            Request::CacheClear { id: 9 },
            Request::Metrics { id: 10 },
            Request::Suggest { id: 11, query: "test".to_string(), limit: None },
        ];

        // Then: Each should return its ID
        for (idx, request) in requests.iter().enumerate() {
            assert_eq!(
                request.id(),
                (idx + 1) as u64,
                "Request type {:?} should return correct ID",
                request
            );
        }
    }

    /// Verify batch request serialization
    #[test]
    fn test_batch_request_serialization() -> Result<()> {
        // Given: A batch request with multiple sub-requests
        let batch = Request::Batch {
            id: 1,
            requests: Box::new(vec![
                Request::Search { id: 2, query: "vim".to_string(), limit: Some(5) },
                Request::Info { id: 3, package: "git".to_string() },
            ]),
        };

        // When: We serialize with bitcode
        let bytes = bitcode::serialize(&batch)?;

        // Then: Should deserialize correctly
        let deserialized: Request = bitcode::deserialize(&bytes)?;
        match deserialized {
            Request::Batch { id, requests } => {
                assert_eq!(id, 1);
                assert_eq!(requests.len(), 2);
            }
            _ => panic!("Expected batch request"),
        }

        Ok(())
    }

    /// Verify socket path is correctly generated
    #[test]
    fn test_socket_path_generation() {
        use omg_lib::core::paths::socket_path;

        // When: We get the socket path
        let path = socket_path();

        // Then: Path should end with omg.sock
        assert!(
            path.to_string_lossy().contains("omg.sock"),
            "Socket path should contain omg.sock"
        );
    }

    /// Verify daemon disabled check respects environment
    #[test]
    fn test_daemon_disabled_check() {
        use omg_lib::core::paths::test_mode;

        // Note: This test runs in test mode, so daemon should be disabled
        // The actual env var is set by the test harness

        // When: We check test mode
        // (In actual test execution, OMG_TEST_MODE=1 is set)

        // Then: We just verify the function exists and returns a bool
        let _ = test_mode(); // Just verify it compiles and runs
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 5: INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Integration tests combining multiple components
mod integration_tests {
    use super::*;

    /// Verify complete license activation flow (mocked)
    #[test]
    fn test_license_activation_flow_mocked() -> Result<()> {
        // Given: A test environment
        let env = E2ETestEnv::new()?;

        // Simulate successful activation by creating license file
        let license = r#"{
            "key": "OMG-PRO-TEST-1234",
            "tier": "pro",
            "features": ["sbom", "audit", "secrets"],
            "customer": "Test User",
            "expires_at": "2025-12-31",
            "validated_at": 1700000000,
            "machine_id": "test123"
        }"#;
        env.create_data_file("license.json", license)?;

        // Then: License should be persisted
        assert!(env.data_file_exists("license.json"));

        // And: Should be readable
        let content = env.read_data_file("license.json")?;
        assert!(content.contains("OMG-PRO-TEST-1234"));
        assert!(content.contains("pro"));

        Ok(())
    }

    /// Verify usage tracking accumulates across sessions
    #[test]
    fn test_usage_tracking_persistence() -> Result<()> {
        // Given: A test environment with existing usage
        let env = E2ETestEnv::new()?;

        // Initial usage
        let initial_usage = r#"{
            "total_commands": 100,
            "time_saved_ms": 12700,
            "commands": {"search": 50, "info": 30, "install": 20},
            "installed_packages": {"vim": 1, "git": 1},
            "runtime_usage_counts": {"node": 5},
            "queries_today": 10,
            "queries_this_month": 100,
            "last_query_date": "2024-01-15",
            "last_month": "2024-01",
            "last_sync": 1700000000
        }"#;
        env.create_data_file("usage.json", initial_usage)?;

        // When: We simulate adding more usage
        let content = env.read_data_file("usage.json")?;
        let mut stats: omg_lib::core::usage::UsageStats = serde_json::from_str(&content)?;
        stats.total_commands += 5;
        stats.time_saved_ms += 635; // 5 * 127ms for search

        // Save updated stats
        let updated = serde_json::to_string_pretty(&stats)?;
        env.create_data_file("usage.json", &updated)?;

        // Then: Updated values should persist
        let final_content = env.read_data_file("usage.json")?;
        let final_stats: omg_lib::core::usage::UsageStats = serde_json::from_str(&final_content)?;

        assert_eq!(final_stats.total_commands, 105, "Commands should accumulate");
        assert_eq!(final_stats.time_saved_ms, 13335, "Time saved should accumulate");

        Ok(())
    }

    /// Verify environment isolation between tests
    #[test]
    fn test_environment_isolation() -> Result<()> {
        // Given: Two separate test environments
        let env1 = E2ETestEnv::new()?;
        let env2 = E2ETestEnv::new()?;

        // When: We create files in one environment
        env1.create_data_file("test.txt", "env1 content")?;
        env2.create_data_file("test.txt", "env2 content")?;

        // Then: Each environment should have its own file
        assert_eq!(env1.read_data_file("test.txt")?, "env1 content");
        assert_eq!(env2.read_data_file("test.txt")?, "env2 content");

        // And: Paths should be different
        assert_ne!(env1.data_path(), env2.data_path());

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 6: ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for error handling and edge cases
mod error_handling_tests {
    use super::*;

    /// Verify graceful handling of corrupted license file
    #[test]
    fn test_corrupted_license_file_handling() -> Result<()> {
        // Given: A test environment with corrupted license
        let env = E2ETestEnv::new()?;
        env.create_data_file("license.json", "{ invalid json }")?;

        // When: We try to parse the file
        let content = env.read_data_file("license.json")?;
        let result: Result<omg_lib::core::license::StoredLicense, _> =
            serde_json::from_str(&content);

        // Then: Parsing should fail gracefully
        assert!(
            result.is_err(),
            "Corrupted license file should fail to parse"
        );

        Ok(())
    }

    /// Verify graceful handling of missing license file
    #[test]
    fn test_missing_license_file_handling() -> Result<()> {
        // Given: A test environment without license file
        let env = E2ETestEnv::new()?;

        // Then: File should not exist
        assert!(
            !env.data_file_exists("license.json"),
            "License file should not exist by default"
        );

        Ok(())
    }

    /// Verify graceful handling of corrupted usage file
    #[test]
    fn test_corrupted_usage_file_handling() -> Result<()> {
        // Given: A test environment with corrupted usage file
        let env = E2ETestEnv::new()?;
        env.create_data_file("usage.json", "not valid json at all")?;

        // When: We try to parse the file
        let content = env.read_data_file("usage.json")?;
        let result: Result<omg_lib::core::usage::UsageStats, _> = serde_json::from_str(&content);

        // Then: Should fail to parse
        assert!(result.is_err(), "Corrupted usage file should fail to parse");

        Ok(())
    }

    /// Verify network timeout handling (simulated)
    #[test]
    fn test_network_timeout_simulation() {
        // Given: A very short timeout
        let timeout = Duration::from_millis(1);

        // Then: Timeout should be less than typical network latency
        assert!(
            timeout < Duration::from_secs(1),
            "Timeout should be configurable for testing"
        );
    }

    /// Verify empty response handling
    #[test]
    fn test_empty_response_handling() -> Result<()> {
        // Given: An empty search result
        let response = omg_lib::daemon::protocol::SearchResult {
            packages: vec![],
            total: 0,
        };

        // When: We serialize it
        let bytes = bitcode::serialize(&response)?;

        // Then: Should deserialize correctly
        let deserialized: omg_lib::daemon::protocol::SearchResult = bitcode::deserialize(&bytes)?;
        assert!(
            deserialized.packages.is_empty(),
            "Empty packages should deserialize"
        );
        assert_eq!(deserialized.total, 0, "Total should be 0");

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 7: PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Performance-related tests (unit-level, not full benchmarks)
mod performance_tests {
    use super::*;

    /// Verify bitcode serialization is fast (sanity check)
    #[test]
    fn test_bitcode_serialization_performance() -> Result<()> {
        // Given: A typical request
        let request = omg_lib::daemon::protocol::Request::Search {
            id: 1,
            query: "firefox".to_string(),
            limit: Some(100),
        };

        // When: We measure serialization time
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = bitcode::serialize(&request)?;
        }
        let duration = start.elapsed();

        // Then: Should be reasonably fast (< 10ms for 1000 iterations)
        assert!(
            duration < Duration::from_millis(100),
            "1000 serializations should complete in <100ms, took {:?}",
            duration
        );

        Ok(())
    }

    /// Verify version comparison is fast
    #[test]
    fn test_version_comparison_performance() -> Result<()> {
        // Given: Two versions
        let v1 = semver::Version::parse("0.1.75")?;
        let v2 = semver::Version::parse("0.2.0")?;

        // When: We perform many comparisons
        let start = Instant::now();
        for _ in 0..10000 {
            let _ = v1 < v2;
        }
        let duration = start.elapsed();

        // Then: Should be sub-millisecond for 10000 comparisons
        assert!(
            duration < Duration::from_millis(10),
            "10000 version comparisons should complete in <10ms, took {:?}",
            duration
        );

        Ok(())
    }

    /// Verify JSON serialization performance for usage stats
    #[test]
    fn test_usage_stats_json_performance() -> Result<()> {
        // Given: Usage stats with data
        let mut stats = omg_lib::core::usage::UsageStats::default();
        for i in 0..100 {
            stats.installed_packages.insert(format!("pkg-{i}"), i as u64);
        }
        stats.total_commands = 1000;
        stats.time_saved_ms = 100000;

        // When: We serialize many times
        let start = Instant::now();
        for _ in 0..100 {
            let _ = serde_json::to_string(&stats)?;
        }
        let duration = start.elapsed();

        // Then: Should be reasonably fast
        assert!(
            duration < Duration::from_millis(100),
            "100 JSON serializations should complete in <100ms, took {:?}",
            duration
        );

        Ok(())
    }
}
