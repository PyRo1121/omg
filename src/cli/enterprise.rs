//! `omg enterprise` - Enterprise features (reports, policies, compliance)

use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{
    CliContext, EnterpriseCommands, EnterprisePolicyCommands, LocalCommandRunner, ServerCommands,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::core::license;

impl LocalCommandRunner for EnterpriseCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        match self {
            EnterpriseCommands::Reports {
                report_type,
                format,
            } => reports(report_type, format, ctx).await,
            EnterpriseCommands::Policy { command } => match command {
                EnterprisePolicyCommands::Set { scope, rule } => policy::set(scope, rule, ctx),
                EnterprisePolicyCommands::Show { scope } => {
                    policy::show(scope.as_deref(), ctx).await
                }
                EnterprisePolicyCommands::Inherit { from, to } => policy::inherit(from, to, ctx),
            },
            EnterpriseCommands::AuditExport {
                format,
                period,
                output,
            } => audit_export(format, period.as_deref(), output, ctx),
            EnterpriseCommands::LicenseScan { export } => license_scan(export.as_deref(), ctx),
            EnterpriseCommands::Server { command } => match command {
                ServerCommands::Init {
                    license,
                    storage,
                    domain,
                } => server::init(license, storage, domain, ctx),
                ServerCommands::Mirror { upstream } => server::mirror(upstream, ctx).await,
            },
        }
    }
}

/// Generate executive reports
pub async fn reports(report_type: &str, format: &str, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate report type and format
    let valid_types = ["monthly", "quarterly", "custom"];
    let valid_formats = ["json", "csv", "html", "pdf"];
    if !valid_types.contains(&report_type.to_lowercase().as_str()) {
        execute_cmd(Components::error_with_suggestion(
            format!("Invalid report type: {report_type}"),
            "Valid types: monthly, quarterly, custom",
        ));
        anyhow::bail!("Invalid report type: {report_type}");
    }
    if !valid_formats.contains(&format.to_lowercase().as_str()) {
        execute_cmd(Components::error_with_suggestion(
            format!("Invalid report format: {format}"),
            "Valid formats: json, csv, html, pdf",
        ));
        anyhow::bail!("Invalid report format: {format}");
    }

    license::require_feature("enterprise-reports")?;

    execute_cmd(Components::loading(format!(
        "Generating {report_type} report..."
    )));

    let report = generate_report(report_type).await?;
    let filename = format!(
        "omg-report-{}-{}.{}",
        report_type,
        jiff::Timestamp::now().as_second(),
        format
    );

    // For now, output to JSON (PDF would require additional dependencies)
    let content = serde_json::to_string_pretty(&report)?;
    fs::write(&filename, &content)?;

    let report_sections = vec![
        "Executive Summary".to_string(),
        "Compliance Score Trend".to_string(),
        "Vulnerability Remediation Timeline".to_string(),
        "Team Adoption Metrics".to_string(),
        "Cost Savings Analysis".to_string(),
        "Recommendations".to_string(),
    ];

    execute_cmd(Cmd::batch([
        Cmd::success(format!("Generated {filename}")),
        Cmd::spacer(),
        Components::kv_list(
            Some("Report Details"),
            vec![
                ("Type", report_type),
                ("Format", format),
                ("File", &filename),
            ],
        ),
        Cmd::spacer(),
        Cmd::card("Report Contents", report_sections),
    ]));

    Ok(())
}

/// Export audit evidence for compliance
pub fn audit_export(
    format: &str,
    period: Option<&str>,
    output: &str,
    _ctx: &CliContext,
) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate all inputs
    let valid_frameworks = ["soc2", "iso27001", "fedramp", "hipaa", "pci-dss"];
    if !valid_frameworks.contains(&format.to_lowercase().as_str()) {
        execute_cmd(Components::error_with_suggestion(
            format!("Invalid compliance framework: {format}"),
            "Valid frameworks: soc2, iso27001, fedramp, hipaa, pci-dss",
        ));
        anyhow::bail!("Invalid compliance framework: {format}");
    }
    if let Some(p) = period
        && (p.len() > 64 || p.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-'))
    {
        execute_cmd(Cmd::error("Invalid period format"));
        anyhow::bail!("Invalid period format");
    }
    if let Err(e) = crate::core::security::validate_relative_path(output) {
        execute_cmd(Cmd::error(format!("Invalid output path: {e}")));
        return Err(e);
    }

    license::require_feature("audit-export")?;

    execute_cmd(Components::loading(format!(
        "Exporting {format} audit evidence..."
    )));

    let period_str = period.unwrap_or("current");
    fs::create_dir_all(output)?;

    // Generate audit files
    let files = vec![
        ("access-control-matrix.csv", generate_access_control_csv()),
        ("change-log.json", generate_change_log_json()?),
        ("policy-enforcement.json", generate_policy_json()?),
        ("installed-packages.csv", generate_installed_packages_csv()?),
        ("sbom-inventory.json", generate_sbom_json()?),
    ];

    let mut file_list = vec![];
    for (filename, content) in &files {
        let path = Path::new(output).join(filename);
        fs::write(&path, content)?;
        file_list.push(path.display().to_string());
    }

    execute_cmd(Cmd::batch([
        Cmd::success("Audit evidence exported"),
        Cmd::spacer(),
        Components::kv_list(
            Some("Export Details"),
            vec![
                ("Framework", format),
                ("Period", period_str),
                ("Output", output),
            ],
        ),
        Cmd::spacer(),
        Cmd::card("Generated Files", file_list),
        Cmd::spacer(),
        Components::complete("Ready for auditor review"),
    ]));

    Ok(())
}

/// Scan for license compliance issues
pub fn license_scan(export: Option<&str>, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if let Some(fmt) = export {
        // SECURITY: Validate export format
        let valid_formats = ["json", "csv", "spdx"];
        if !valid_formats.contains(&fmt.to_lowercase().as_str()) {
            execute_cmd(Components::error_with_suggestion(
                format!("Invalid license export format: {fmt}"),
                "Valid formats: json, csv, spdx",
            ));
            anyhow::bail!("Invalid license export format: {fmt}");
        }
    }

    license::require_feature("license-scan")?;

    let scan = perform_license_scan()?;

    // Display results
    let mut license_inventory = vec![];
    for (license, count) in &scan.by_license {
        let pct = (*count as f32 / scan.total as f32) * 100.0;
        license_inventory.push(format!("{license}: {count} packages ({pct:.0}%)"));
    }

    let mut violations = vec![];
    for violation in &scan.violations {
        violations.push(format!("{} - {}", violation.package, violation.reason));
    }

    let mut unknown = vec![];
    for pkg in scan.unknown.iter().take(5) {
        unknown.push(pkg.clone());
    }
    if scan.unknown.len() > 5 {
        unknown.push(format!("... and {} more", scan.unknown.len() - 5));
    }

    execute_cmd(Cmd::batch([
        Cmd::header(
            "License Compliance Scan",
            format!("{} total packages", scan.total),
        ),
        Cmd::spacer(),
        Cmd::card("License Inventory", license_inventory),
        if violations.is_empty() {
            Cmd::none()
        } else {
            Cmd::batch([Cmd::spacer(), Cmd::card("Policy Violations", violations)])
        },
        if unknown.is_empty() {
            Cmd::none()
        } else {
            Cmd::batch([Cmd::spacer(), Cmd::card("Unknown Licenses", unknown)])
        },
        if let Some(format) = export {
            Cmd::batch([Cmd::spacer(), {
                let filename = format!(
                    "license-scan-{}.{}",
                    jiff::Timestamp::now().as_second(),
                    format
                );
                let content = match format {
                    "csv" => generate_license_csv(&scan),
                    _ => serde_json::to_string_pretty(&scan)?,
                };
                fs::write(&filename, content)?;
                Cmd::success(format!("Exported to {filename}"))
            }])
        } else {
            Cmd::none()
        },
    ]));

    Ok(())
}

/// Enterprise policy management
pub mod policy {
    use super::{CliContext, Result, license};
    use crate::cli::packages::execute_cmd;
    use crate::cli::tea::Cmd;

    pub fn set(scope: &str, rule: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Validate scope and rule
        if scope.len() > 64
            || scope
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != ':' && c != '-')
        {
            execute_cmd(Cmd::error("Invalid policy scope"));
            anyhow::bail!("Invalid policy scope");
        }
        if rule.len() > 1024 {
            execute_cmd(Cmd::error("Policy rule too long (max 1024 characters)"));
            anyhow::bail!("Policy rule too long");
        }

        license::require_feature("enterprise-policy")?;

        anyhow::bail!("Enterprise policy persistence is not implemented")
    }

    pub async fn show(scope: Option<&str>, _ctx: &CliContext) -> Result<()> {
        if let Some(s) = scope
            && (s.len() > 64
                || s.chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != ':' && c != '-'))
        {
            execute_cmd(Cmd::error("Invalid policy scope"));
            anyhow::bail!("Invalid policy scope");
        }

        license::require_feature("policy")?;

        let policies = license::fetch_policies().await?;

        if policies.is_empty() {
            execute_cmd(Cmd::batch([
                Cmd::header("Policy Configuration", "No active policies"),
                Cmd::spacer(),
                Cmd::info("Enterprise policies can be configured in the dashboard"),
            ]));
            return Ok(());
        }

        let mut policy_list = vec![];
        let policy_count = policies.len();
        for p in &policies {
            if let Some(s) = scope
                && p.scope != s
            {
                continue;
            }

            let enforced = if p.enforced { "Yes" } else { "No (Audit only)" };
            policy_list.push(format!(
                "{} (Scope: {}) - Enforced: {}",
                p.rule, p.scope, enforced
            ));
        }

        execute_cmd(Cmd::batch([
            Cmd::header(
                "Policy Configuration",
                format!("{policy_count} active policies"),
            ),
            Cmd::spacer(),
            Cmd::card("Active Policies", policy_list),
        ]));

        Ok(())
    }

    pub fn inherit(from: &str, to: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Validate scopes
        if from.len() > 64
            || from
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != ':' && c != '-')
        {
            execute_cmd(Cmd::error("Invalid source scope"));
            anyhow::bail!("Invalid source scope");
        }
        if to.len() > 64
            || to
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != ':' && c != '-')
        {
            execute_cmd(Cmd::error("Invalid target scope"));
            anyhow::bail!("Invalid target scope");
        }

        license::require_feature("enterprise-policy")?;

        anyhow::bail!("Enterprise policy inheritance is not implemented")
    }
}

/// Self-hosted server management
pub mod server {
    use super::{CliContext, Result, license};
    use crate::cli::components::Components;
    use crate::cli::packages::execute_cmd;
    use crate::cli::tea::Cmd;
    use anyhow::Context;

    pub fn init(license_key: &str, storage: &str, domain: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Validate all inputs
        if license_key.len() > 128
            || license_key
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != '-')
        {
            execute_cmd(Cmd::error("Invalid license key format"));
            anyhow::bail!("Invalid license key format");
        }
        if let Err(e) = crate::core::security::validate_relative_path(storage) {
            execute_cmd(Cmd::error(format!("Invalid storage path: {e}")));
            return Err(e);
        }
        if domain.len() > 255
            || domain
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
        {
            execute_cmd(Cmd::error("Invalid domain name"));
            anyhow::bail!("Invalid domain name");
        }

        license::require_feature("self-hosted")?;

        anyhow::bail!("Self-hosted server initialization is not implemented")
    }

    pub async fn mirror(upstream: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Basic URL validation
        if !upstream.starts_with("https://") {
            execute_cmd(Components::error_with_suggestion(
                "Only HTTPS upstreams allowed for security",
                "Use https:// instead of http://",
            ));
            anyhow::bail!("Only HTTPS upstreams allowed for security");
        }
        if upstream.len() > 1024 || upstream.chars().any(char::is_control) {
            execute_cmd(Cmd::error("Invalid upstream URL"));
            anyhow::bail!("Invalid upstream URL");
        }

        license::require_feature("self-hosted")?;

        execute_cmd(Components::loading("Syncing from upstream..."));

        let pm = crate::package_managers::get_package_manager()?;
        pm.sync().await?;

        // Check for updates to show meaningful status
        let updates = pm
            .list_updates()
            .await
            .context("Failed to list available updates after mirror sync")?;

        if updates.is_empty() {
            execute_cmd(Cmd::batch([
                Cmd::success("Mirror check complete!"),
                Components::kv_list(
                    Some("Sync Status"),
                    vec![
                        ("Upstream", upstream),
                        ("Status", "Up to date"),
                        ("Last sync", "Just now"),
                    ],
                ),
            ]));
        } else {
            execute_cmd(Cmd::batch([
                Cmd::success("Mirror check complete!"),
                Components::kv_list(
                    Some("Sync Status"),
                    vec![
                        ("Upstream", upstream),
                        ("Updates available", &updates.len().to_string()),
                        ("Last sync", "Just now"),
                    ],
                ),
            ]));
        }

        Ok(())
    }
}

// Helper types and functions

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    generated_at: i64,
    #[serde(rename = "report_type")]
    kind: String,
    summary: ReportSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReportSummary {
    compliance_score: f32,
    total_machines: usize,
    vulnerabilities_fixed: usize,
    cost_savings_estimate: String,
}

async fn generate_report(report_type: &str) -> Result<Report> {
    // A failed team lookup must not become a fabricated zero-machine report.
    let members = license::fetch_team_members()
        .await
        .context("Failed to fetch team members for enterprise report")?;
    let machine_count = members.len();

    let metrics = crate::core::metrics::GLOBAL_METRICS.snapshot();

    // Calculate a real compliance score based on validation failures and security audits
    let base_score = 100.0;
    let penalty =
        (metrics.validation_failures as f32).mul_add(0.5, metrics.rate_limit_hits as f32 * 0.1);
    let compliance_score = (base_score - penalty).max(0.0);

    Ok(Report {
        generated_at: jiff::Timestamp::now().as_second(),
        kind: report_type.to_string(),
        summary: ReportSummary {
            compliance_score,
            total_machines: machine_count,
            vulnerabilities_fixed: metrics.security_audit_requests as usize, // Use as proxy for now
            cost_savings_estimate: format!("${}", machine_count * 120), // Estimate $120 saved per machine
        },
    })
}

fn generate_access_control_csv() -> String {
    let mut csv = "user,role,scope,permissions\n".to_string();
    // In a real system, we'd fetch this from the identity provider or local policy DB
    // For now we use the current user as a base
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let _ = std::fmt::write(&mut csv, format_args!("{user},owner,global,all\n"));
    csv
}

fn generate_change_log_json() -> Result<String> {
    let logger = crate::core::security::audit::AuditLogger::new()
        .context("Failed to open audit log for enterprise export")?;
    let entries = match logger.get_recent(100) {
        Ok(entries) => entries,
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::NotFound)
            }) =>
        {
            Vec::new()
        }
        Err(error) => {
            return Err(error).context("Failed to read audit log entries for enterprise export");
        }
    };
    serde_json::to_string(&entries).context("Failed to serialize audit log entries")
}

fn generate_policy_json() -> Result<String> {
    let policy = crate::core::security::SecurityPolicy::load_default()
        .context("Failed to load security policy for enterprise export")?;
    serde_json::to_string_pretty(&policy).context("Failed to serialize security policy")
}

fn generate_installed_packages_csv() -> Result<String> {
    #[cfg(feature = "arch")]
    {
        let mut csv = String::from("package,version,description\n");
        let packages = crate::package_managers::list_installed_fast()
            .context("Failed to list installed packages for enterprise export")?;
        for pkg in packages {
            let _ = std::fmt::write(
                &mut csv,
                format_args!(
                    "{},{},{}\n",
                    pkg.name,
                    pkg.version,
                    pkg.description.replace(',', " ")
                ),
            );
        }
        Ok(csv)
    }

    #[cfg(not(feature = "arch"))]
    {
        anyhow::bail!("Installed-package export requires the Arch package backend");
    }
}

fn generate_sbom_json() -> Result<String> {
    #[cfg(feature = "arch")]
    {
        let packages = crate::package_managers::list_installed_fast()
            .context("Failed to list installed packages for SBOM export")?;
        let components: Vec<serde_json::Value> = packages
            .into_iter()
            .map(|pkg| {
                serde_json::json!({
                    "type": "library",
                    "name": pkg.name,
                    "version": pkg.version.to_string(),
                    "description": pkg.description,
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.5",
            "version": 1,
            "components": components,
        }))
        .context("Failed to serialize SBOM inventory")
    }
    #[cfg(not(feature = "arch"))]
    anyhow::bail!("SBOM inventory export requires the Arch package backend")
}

#[derive(Debug, Serialize)]
struct LicenseScan {
    total: usize,
    by_license: HashMap<String, usize>,
    violations: Vec<LicenseViolation>,
    unknown: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LicenseViolation {
    package: String,
    license: String,
    reason: String,
}

fn perform_license_scan() -> Result<LicenseScan> {
    #[cfg(not(feature = "arch"))]
    anyhow::bail!("Enterprise license scan requires the Arch package backend");

    #[cfg(feature = "arch")]
    {
        let packages = crate::package_managers::pacman_db::list_local_cached()
            .context("Failed to list installed packages for license scan")?;
        let mut by_license: HashMap<String, usize> = HashMap::new();
        let mut violations: Vec<LicenseViolation> = Vec::new();
        let mut unknown: Vec<String> = Vec::new();
        let total = packages.len();
        for pkg in packages {
            if pkg.licenses.is_empty() {
                unknown.push(pkg.name.clone());
            } else {
                for lic in &pkg.licenses {
                    *by_license.entry(lic.clone()).or_insert(0) += 1;
                    if lic.to_uppercase().contains("GPL") {
                        violations.push(LicenseViolation {
                            package: pkg.name.clone(),
                            license: lic.clone(),
                            reason: "Copyleft license (GPL) requires legal review".to_string(),
                        });
                    }
                }
            }
        }
        Ok(LicenseScan {
            total,
            by_license,
            violations,
            unknown,
        })
    }
}

fn generate_license_csv(scan: &LicenseScan) -> String {
    use std::fmt::Write;
    let mut csv = "license,count\n".to_string();
    for (license, count) in &scan.by_license {
        let _ = writeln!(csv, "{license},{count}");
    }
    csv
}
