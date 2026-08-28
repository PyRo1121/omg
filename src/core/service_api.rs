//! Canonical OMG service endpoints shared by licensing and telemetry clients.

/// Version of the language-neutral CLI service contract.
pub const CONTRACT_VERSION: u64 = 1;
/// Production API origin for the OMG licensing and telemetry service.
pub const ORIGIN: &str = "https://omg-api.latham.cloud";
/// Validate and activate a license key.
pub const VALIDATE_LICENSE: &str = "https://omg-api.latham.cloud/api/validate-license";
/// Report aggregated licensed usage.
pub const REPORT_USAGE: &str = "https://omg-api.latham.cloud/api/report-usage";
/// Record an anonymous installation ping.
pub const INSTALL_PING: &str = "https://omg-api.latham.cloud/api/install-ping";
/// Upload a bounded batch of CLI telemetry events.
pub const CLI_BATCH: &str = "https://omg-api.latham.cloud/api/cli/batch";
/// Read the machine roster for a team license.
pub const TEAM_MEMBERS: &str = "https://omg-api.latham.cloud/api/license/members";
/// Read policy rules for an enterprise license.
pub const TEAM_POLICIES: &str = "https://omg-api.latham.cloud/api/license/policies";
/// Read the customer audit trail for a team license.
pub const TEAM_AUDIT_LOG: &str = "https://omg-api.latham.cloud/api/license/audit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_endpoints_match_the_language_neutral_contract() {
        let contract: serde_json::Value =
            serde_json::from_str(include_str!("../../contracts/service-api-v1.json"))
                .expect("service contract must be valid JSON");
        assert_eq!(contract["schemaVersion"].as_u64(), Some(CONTRACT_VERSION));
        assert_eq!(contract["origin"].as_str(), Some(ORIGIN));

        let expected = [
            ("validateLicense", "POST", "none", VALIDATE_LICENSE),
            ("reportUsage", "POST", "none", REPORT_USAGE),
            ("installPing", "POST", "none", INSTALL_PING),
            ("cliBatch", "POST", "none", CLI_BATCH),
            ("teamMembers", "GET", "license-key", TEAM_MEMBERS),
            ("teamPolicies", "GET", "license-key", TEAM_POLICIES),
            ("teamAuditLog", "GET", "license-key", TEAM_AUDIT_LOG),
        ];
        let endpoints = contract["cliEndpoints"]
            .as_object()
            .expect("contract must define cliEndpoints");
        assert_eq!(endpoints.len(), expected.len());

        for (id, method, authentication, rust_endpoint) in expected {
            let endpoint = &endpoints[id];
            assert_eq!(endpoint["method"].as_str(), Some(method), "method for {id}");
            assert_eq!(
                endpoint["authentication"].as_str(),
                Some(authentication),
                "authentication for {id}"
            );
            let path = endpoint["path"].as_str().unwrap_or_default();
            assert!(!path.is_empty(), "missing path for {id}");
            assert_eq!(rust_endpoint, format!("{ORIGIN}{path}"), "URL for {id}");
        }
    }
}
