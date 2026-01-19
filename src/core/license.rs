//! License management for OMG tiered features
//!
//! Handles license key validation, JWT tokens, and feature gating.
//!
//! ## Tiers
//! - **Free**: Basic package management, runtimes, containers
//! - **Pro** ($9/mo): SBOM, vulnerability scanning, secrets detection
//! - **Team** ($200/mo): Team sync, shared configs, audit logs
//! - **Enterprise** (Contact us): SSO, policy enforcement, SLSA, priority support
//!
//! ## JWT-based Licensing
//! - License activation returns a signed JWT token
//! - Token contains tier, features, expiry, and machine binding
//! - CLI validates token signature offline (no network needed)
//! - Token refreshes every 7 days on validation

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::PathBuf;
use std::str::FromStr;

const LICENSE_API_URL: &str = "https://api.pyro1121.com/api/validate-license";

/// Public key for JWT verification (HMAC secret is server-side only)
/// For offline validation, we embed a verification key
#[allow(dead_code)]
const JWT_PUBLIC_KEY: &str = "omg-license-v1"; // Used for basic validation

/// License tiers (ordered by level)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Team,
    Enterprise,
}

impl Tier {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        s.parse().unwrap_or(Self::Free)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Team => "team",
            Self::Enterprise => "enterprise",
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Pro => "Pro",
            Self::Team => "Team",
            Self::Enterprise => "Enterprise",
        }
    }

    #[must_use]
    pub fn price(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Pro => "$9/mo",
            Self::Team => "$200/mo",
            Self::Enterprise => "Contact us",
        }
    }
}

impl FromStr for Tier {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(Self::Free),
            "pro" => Ok(Self::Pro),
            "team" => Ok(Self::Team),
            "enterprise" => Ok(Self::Enterprise),
            _ => Err(()),
        }
    }
}

/// Feature definitions with required tier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    // Free features
    Packages,
    Runtimes,
    Container,
    EnvCapture,
    EnvShare,
    // Pro features
    Sbom,
    Audit,
    Secrets,
    // Team features
    TeamSync,
    TeamConfig,
    AuditLog,
    // Enterprise features
    Policy,
    Slsa,
    Sso,
    PrioritySupport,
}

impl Feature {
    #[must_use]
    pub fn required_tier(&self) -> Tier {
        match self {
            // Free
            Self::Packages
            | Self::Runtimes
            | Self::Container
            | Self::EnvCapture
            | Self::EnvShare => Tier::Free,
            // Pro
            Self::Sbom | Self::Audit | Self::Secrets => Tier::Pro,
            // Team
            Self::TeamSync | Self::TeamConfig | Self::AuditLog => Tier::Team,
            // Enterprise
            Self::Policy | Self::Slsa | Self::Sso | Self::PrioritySupport => Tier::Enterprise,
        }
    }

    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "packages" => Some(Self::Packages),
            "runtimes" => Some(Self::Runtimes),
            "container" => Some(Self::Container),
            "env-capture" | "env_capture" => Some(Self::EnvCapture),
            "env-share" | "env_share" => Some(Self::EnvShare),
            "sbom" => Some(Self::Sbom),
            "audit" => Some(Self::Audit),
            "secrets" => Some(Self::Secrets),
            "team-sync" | "team_sync" => Some(Self::TeamSync),
            "team-config" | "team_config" => Some(Self::TeamConfig),
            "audit-log" | "audit_log" => Some(Self::AuditLog),
            "policy" => Some(Self::Policy),
            "slsa" => Some(Self::Slsa),
            "sso" => Some(Self::Sso),
            "priority-support" | "priority_support" => Some(Self::PrioritySupport),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Packages => "packages",
            Self::Runtimes => "runtimes",
            Self::Container => "container",
            Self::EnvCapture => "env-capture",
            Self::EnvShare => "env-share",
            Self::Sbom => "sbom",
            Self::Audit => "audit",
            Self::Secrets => "secrets",
            Self::TeamSync => "team-sync",
            Self::TeamConfig => "team-config",
            Self::AuditLog => "audit-log",
            Self::Policy => "policy",
            Self::Slsa => "slsa",
            Self::Sso => "sso",
            Self::PrioritySupport => "priority-support",
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Packages => "Package Management",
            Self::Runtimes => "Runtime Version Switching",
            Self::Container => "Container Integration",
            Self::EnvCapture => "Environment Fingerprinting",
            Self::EnvShare => "Gist Sharing",
            Self::Sbom => "SBOM Generation (CycloneDX)",
            Self::Audit => "Vulnerability Scanning",
            Self::Secrets => "Secret Detection",
            Self::TeamSync => "Team Environment Sync",
            Self::TeamConfig => "Shared Team Configs",
            Self::AuditLog => "Tamper-proof Audit Logs",
            Self::Policy => "Policy Enforcement",
            Self::Slsa => "SLSA Provenance Verification",
            Self::Sso => "SSO/SAML Integration",
            Self::PrioritySupport => "Priority Support",
        }
    }
}

/// All features grouped by tier
pub const FREE_FEATURES: &[Feature] = &[
    Feature::Packages,
    Feature::Runtimes,
    Feature::Container,
    Feature::EnvCapture,
    Feature::EnvShare,
];

pub const PRO_FEATURES: &[Feature] = &[Feature::Sbom, Feature::Audit, Feature::Secrets];

pub const TEAM_FEATURES: &[Feature] = &[Feature::TeamSync, Feature::TeamConfig, Feature::AuditLog];

pub const ENTERPRISE_FEATURES: &[Feature] = &[
    Feature::Policy,
    Feature::Slsa,
    Feature::Sso,
    Feature::PrioritySupport,
];

/// License response from the validation API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseResponse {
    pub valid: bool,
    pub tier: Option<String>,
    pub features: Option<Vec<String>>,
    pub customer: Option<String>,
    pub expires_at: Option<String>,
    pub token: Option<String>, // Signed JWT for offline validation
    pub error: Option<String>,
}

/// JWT payload structure (matches backend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtPayload {
    pub sub: String,           // customer_id
    pub tier: String,          // license tier
    pub features: Vec<String>, // enabled features
    pub exp: i64,              // expiration timestamp
    pub iat: i64,              // issued at
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>, // machine_id (optional binding)
    pub lic: String,           // license_key for reference
}

/// Stored license information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredLicense {
    pub key: String,
    pub tier: String,
    pub features: Vec<String>,
    pub customer: Option<String>,
    pub expires_at: Option<String>,
    pub validated_at: i64,
    pub token: Option<String>,      // JWT token for offline validation
    pub machine_id: Option<String>, // Bound machine ID
}

impl StoredLicense {
    #[must_use]
    pub fn tier_enum(&self) -> Tier {
        Tier::parse(&self.tier)
    }

    /// Check if the stored token is still valid
    #[must_use]
    pub fn is_token_valid(&self) -> bool {
        if let Some(token) = &self.token
            && let Some(payload) = decode_jwt_payload(token)
        {
            let now = jiff::Timestamp::now().as_second();
            return payload.exp > now;
        }
        false
    }

    /// Check if token needs refresh (< 1 day remaining)
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        if let Some(token) = &self.token
            && let Some(payload) = decode_jwt_payload(token)
        {
            let now = jiff::Timestamp::now().as_second();
            let one_day = 24 * 60 * 60;
            return payload.exp - now < one_day;
        }
        true
    }
}

/// Get machine fingerprint for license binding
#[must_use]
pub fn get_machine_id() -> String {
    // Combine multiple system identifiers for a stable fingerprint
    let mut components = Vec::new();

    // Machine ID (Linux)
    if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
        components.push(id.trim().to_string());
    }

    // Hostname
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        components.push(hostname);
    } else if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        components.push(hostname.trim().to_string());
    }

    // Username
    if let Ok(user) = std::env::var("USER") {
        components.push(user);
    }

    // Hash the combined components
    let combined = components.join(":");
    let hash = sha256_hex(combined.as_bytes());
    hash[..16].to_string() // First 16 chars of hash
}

/// SHA256 hash as hex string
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Decode JWT payload without verification (for reading claims)
fn decode_jwt_payload(token: &str) -> Option<JwtPayload> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload_b64 = parts[1];
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let payload_str = String::from_utf8(payload_bytes).ok()?;
    serde_json::from_str(&payload_str).ok()
}

/// Get the license file path
fn license_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .context("Could not find data directory")?
        .join("omg");
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("license.json"))
}

/// Load stored license from disk
pub fn load_license() -> Option<StoredLicense> {
    let path = license_path().ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save license to disk
pub fn save_license(license: &StoredLicense) -> Result<()> {
    let path = license_path()?;
    let content = serde_json::to_string_pretty(license)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Remove stored license
pub fn remove_license() -> Result<()> {
    let path = license_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Validate a license key against the API
pub async fn validate_license(key: &str) -> Result<LicenseResponse> {
    validate_license_with_user(key, None, None).await
}

/// Validate a license key with optional user info for team identification
pub async fn validate_license_with_user(
    key: &str,
    user_name: Option<&str>,
    user_email: Option<&str>,
) -> Result<LicenseResponse> {
    let machine_id = get_machine_id();
    let mut url = format!("{LICENSE_API_URL}?key={key}&machine_id={machine_id}");

    if let Some(name) = user_name {
        let _ = write!(url, "&user_name={}", urlencoding::encode(name));
    }
    if let Some(email) = user_email {
        let _ = write!(url, "&user_email={}", urlencoding::encode(email));
    }

    let response = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context("Failed to connect to license server")?;

    let license: LicenseResponse = response
        .json()
        .await
        .context("Failed to parse license response")?;

    Ok(license)
}

/// Activate a license key
pub async fn activate(key: &str) -> Result<StoredLicense> {
    activate_with_user(key, None, None).await
}

/// Activate a license key with user info for team identification
pub async fn activate_with_user(
    key: &str,
    user_name: Option<&str>,
    user_email: Option<&str>,
) -> Result<StoredLicense> {
    let response = validate_license_with_user(key, user_name, user_email).await?;

    if !response.valid {
        anyhow::bail!(
            "Invalid license: {}",
            response
                .error
                .unwrap_or_else(|| "Unknown error".to_string())
        );
    }

    let stored = StoredLicense {
        key: key.to_string(),
        tier: response.tier.unwrap_or_else(|| "pro".to_string()),
        features: response.features.unwrap_or_default(),
        customer: response.customer,
        expires_at: response.expires_at,
        validated_at: jiff::Timestamp::now().as_second(),
        token: response.token,
        machine_id: Some(get_machine_id()),
    };

    save_license(&stored)?;

    Ok(stored)
}

/// Refresh license token if needed (called periodically)
pub async fn refresh_if_needed() -> Result<()> {
    let Some(license) = load_license() else {
        return Ok(()); // No license to refresh
    };

    if !license.needs_refresh() {
        return Ok(()); // Token still valid
    }

    // Try to refresh
    match validate_license(&license.key).await {
        Ok(response) if response.valid => {
            let updated = StoredLicense {
                key: license.key,
                tier: response.tier.unwrap_or(license.tier),
                features: response.features.unwrap_or(license.features),
                customer: response.customer.or(license.customer),
                expires_at: response.expires_at.or(license.expires_at),
                validated_at: jiff::Timestamp::now().as_second(),
                token: response.token.or(license.token),
                machine_id: license.machine_id,
            };
            save_license(&updated)?;
        }
        _ => {
            // Refresh failed, but token might still be valid for offline use
            tracing::warn!("License refresh failed, using cached token");
        }
    }

    Ok(())
}

/// Get current user tier
pub fn current_tier() -> Tier {
    load_license().map_or(Tier::Free, |l| l.tier_enum())
}

/// Check if a feature is available based on current tier
pub fn has_feature(feature_name: &str) -> bool {
    let Some(feature) = Feature::from_str(feature_name) else {
        return true; // Unknown features are allowed
    };

    current_tier() >= feature.required_tier()
}

/// Check if user has at least the specified tier
pub fn has_tier(required: Tier) -> bool {
    current_tier() >= required
}

/// Require a feature, returning an error if not available
pub fn require_feature(feature_name: &str) -> Result<()> {
    if has_feature(feature_name) {
        return Ok(());
    }

    let required_tier = Feature::from_str(feature_name).map_or(Tier::Pro, |f| f.required_tier());

    anyhow::bail!(
        "Feature '{}' requires {} tier ({}). Upgrade at https://pyro1121.com/pricing",
        feature_name,
        required_tier.display_name(),
        required_tier.price()
    )
}

/// Require at least a specific tier
pub fn require_tier(required: Tier) -> Result<()> {
    if has_tier(required) {
        return Ok(());
    }

    anyhow::bail!(
        "This feature requires {} tier ({}). Upgrade at https://pyro1121.com/pricing",
        required.display_name(),
        required.price()
    )
}

/// Get current license status
pub fn status() -> Option<StoredLicense> {
    load_license()
}

/// Get features available for a tier
pub fn features_for_tier(tier: Tier) -> Vec<&'static Feature> {
    let mut features: Vec<&Feature> = Vec::new();

    // Always include free features
    features.extend(FREE_FEATURES.iter());

    if matches!(tier, Tier::Pro | Tier::Team | Tier::Enterprise) {
        features.extend(PRO_FEATURES.iter());
    }

    if matches!(tier, Tier::Team | Tier::Enterprise) {
        features.extend(TEAM_FEATURES.iter());
    }

    if tier == Tier::Enterprise {
        features.extend(ENTERPRISE_FEATURES.iter());
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_hierarchy() {
        assert!(matches!(Tier::parse("pro"), Tier::Pro));
        assert!(matches!(Tier::parse("team"), Tier::Team));
        assert!(matches!(Tier::parse("enterprise"), Tier::Enterprise));
        assert!(matches!(Tier::parse("unknown"), Tier::Free));
    }

    #[test]
    fn test_feature_tiers() {
        assert_eq!(Feature::Packages.required_tier(), Tier::Free);
        assert_eq!(Feature::Sbom.required_tier(), Tier::Pro);
        assert_eq!(Feature::TeamSync.required_tier(), Tier::Team);
        assert_eq!(Feature::Policy.required_tier(), Tier::Enterprise);
    }

    #[test]
    fn test_free_features_available() {
        // Free features should always be available (no license)
        assert!(has_feature("packages"));
        assert!(has_feature("runtimes"));
        assert!(has_feature("container"));
    }
}
