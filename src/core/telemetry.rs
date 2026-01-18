//! Telemetry and install tracking
//!
//! Privacy-first telemetry that tracks install counts for GitHub badge display.
//! - Opt-out via `OMG_TELEMETRY=0` environment variable
//! - Anonymous data collection (no personal information)
//! - One-time ping on first install only
//! - Silent failure if network unavailable

use anyhow::Result;
use serde::{Deserialize, Serialize};

const TELEMETRY_API_URL: &str = "https://api.pyro1121.com/api/install-ping";

/// Install telemetry payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPayload {
    /// Anonymous install ID (UUID v4)
    pub install_id: String,
    /// Install timestamp (ISO 8601)
    pub timestamp: String,
    /// OMG version
    pub version: String,
    /// Platform (e.g., "linux-x86_64")
    pub platform: String,
    /// Package manager backend (arch/debian)
    pub backend: String,
}

/// Marker file content
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallMarker {
    install_id: String,
    timestamp: String,
    version: String,
}

/// Check if telemetry is opted out
#[must_use]
pub fn is_telemetry_opt_out() -> bool {
    matches!(
        std::env::var("OMG_TELEMETRY").as_deref(),
        Ok("0" | "false" | "FALSE" | "off" | "OFF")
    )
}

/// Check if this is the first run
pub fn is_first_run() -> bool {
    let marker_path = super::paths::installed_marker_path();
    !marker_path.exists()
}

/// Generate or load install ID
fn generate_or_load_id() -> Result<String> {
    let marker_path = super::paths::installed_marker_path();

    if marker_path.exists() {
        // Load existing ID
        let content = std::fs::read_to_string(&marker_path)?;
        let marker: InstallMarker = serde_json::from_str(&content)?;
        Ok(marker.install_id)
    } else {
        // Generate new ID
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

/// Get platform string (e.g., "linux-x86_64")
fn get_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Get package manager backend
fn get_backend() -> String {
    #[cfg(feature = "arch")]
    return "arch".to_string();

    #[cfg(feature = "debian")]
    return "debian".to_string();

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    return "unknown".to_string();
}

/// Create install marker file
fn create_marker(install_id: &str) -> Result<()> {
    let marker_path = super::paths::installed_marker_path();

    // Ensure parent directory exists
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let marker = InstallMarker {
        install_id: install_id.to_string(),
        timestamp: jiff::Timestamp::now().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let content = serde_json::to_string_pretty(&marker)?;
    std::fs::write(marker_path, content)?;

    Ok(())
}

/// Ping install telemetry endpoint
pub async fn ping_install() -> Result<()> {
    // Generate or load install ID
    let install_id = generate_or_load_id()?;

    // Create payload
    let payload = InstallPayload {
        install_id: install_id.clone(),
        timestamp: jiff::Timestamp::now().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: get_platform(),
        backend: get_backend(),
    };

    // Send ping with timeout
    let client = reqwest::Client::new();
    let response = client
        .post(TELEMETRY_API_URL)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    // Create marker file on success or network error (don't block on failure)
    match response {
        Ok(_) => {
            tracing::debug!("Install telemetry ping successful");
            create_marker(&install_id)?;
        }
        Err(e) => {
            // Silent fail - create marker anyway to avoid retrying
            tracing::debug!("Install telemetry ping failed (silent): {}", e);
            create_marker(&install_id)?;
        }
    }

    Ok(())
}
