//! Canonical OMG service endpoints shared by licensing and telemetry clients.

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
    fn every_service_endpoint_uses_the_production_origin() {
        for endpoint in [
            VALIDATE_LICENSE,
            REPORT_USAGE,
            INSTALL_PING,
            CLI_BATCH,
            TEAM_MEMBERS,
            TEAM_POLICIES,
            TEAM_AUDIT_LOG,
        ] {
            assert!(
                endpoint.starts_with(ORIGIN),
                "unexpected endpoint: {endpoint}"
            );
        }
    }
}
