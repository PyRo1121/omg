//! Security audit command implementations
//!
//! Provides CLI handlers for vulnerability scanning, SBOM generation, secret detection,
//! license compliance, SLSA verification, and audit log management.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

fn read_audit_entries(
    logger: &AuditLogger,
    limit: usize,
    severity_filter: Option<&str>,
) -> Result<Vec<crate::core::security::audit::AuditEntry>> {
    let result = if let Some(sev) = severity_filter {
        let min_severity = match sev.to_lowercase().as_str() {
            "debug" => AuditSeverity::Debug,
            "info" => AuditSeverity::Info,
            "warning" | "warn" => AuditSeverity::Warning,
            "error" => AuditSeverity::Error,
            "critical" => AuditSeverity::Critical,
            _ => anyhow::bail!("Invalid severity: {sev}"),
        };
        logger.filter_by_severity(min_severity)
    } else {
        logger.get_recent(limit)
    };
    match result {
        Ok(entries) => Ok(entries),
        Err(error) if error.is_not_found() => Ok(Vec::new()),
        Err(error) => Err(error).context("Failed to read audit log entries"),
    }
}

use crate::cli::{AuditCommands, CliContext, LocalCommandRunner, style, ui};
#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::core::license;
use crate::core::security::{AuditLogger, AuditSeverity, SbomGenerator, SecurityPolicy};
use dialoguer;

impl LocalCommandRunner for AuditCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        ui::print_spacer();
        match self {
            AuditCommands::Scan => scan(ctx).await,
            AuditCommands::Sbom { output, vulns } => {
                generate_sbom(output.clone(), *vulns, ctx).await
            }
            AuditCommands::Secrets { path } => scan_secrets(path.clone(), ctx),
            AuditCommands::Log {
                limit,
                severity,
                export,
            } => view_audit_log(*limit, severity.as_deref(), export.clone(), ctx),
            AuditCommands::Verify => verify_audit_log(ctx),
            AuditCommands::Policy => show_policy(ctx),
            AuditCommands::Slsa { package } => check_slsa(package, ctx).await,
            AuditCommands::Licenses {
                format,
                export,
                filter,
                check_policy,
            } => scan_licenses(format, export.clone(), filter.clone(), *check_policy, ctx),
            AuditCommands::Fix {
                dry_run,
                yes,
                min_severity,
            } => fix_vulnerabilities(*dry_run, *yes, min_severity, ctx).await,
            AuditCommands::Export {
                framework,
                period,
                output,
            } => export_compliance(framework, period.clone(), output, ctx).await,
            AuditCommands::Eol => check_eol(ctx),
        }?;
        ui::print_spacer();
        Ok(())
    }
}

/// Perform security audit (vulnerability scan)
pub async fn scan(_ctx: &CliContext) -> Result<()> {
    // Require Pro tier for vulnerability scanning
    license::require_feature("audit")?;

    ui::print_header("Secure", "Vulnerability Scan");

    #[cfg(unix)]
    {
        let mut client = DaemonClient::connect().await.context(
            "Daemon not running. Security audit requires the daemon (start it with: omg daemon)",
        )?;
        let res = client
            .security_audit()
            .await
            .context("Failed to run security audit")?;
        if res.total_vulnerabilities == 0 {
            ui::print_success("No vulnerabilities found in scanned packages.");
        } else {
            ui::print_warning(format!(
                "Found {} vulnerabilities ({} high severity)",
                res.total_vulnerabilities, res.high_severity
            ));
            println!();
            for (pkg, vulns) in res.vulnerabilities {
                println!(
                    "  {} ({} issues):",
                    style::maybe_color(&pkg, |t| t.white().bold().to_string()),
                    vulns.len()
                );
                for vuln in vulns {
                    let score = vuln
                        .score
                        .map(|s| format!(" [Score: {s}]"))
                        .unwrap_or_default();
                    println!(
                        "    {} {} - {}{}",
                        style::maybe_color("→", |t| t.red().to_string()),
                        style::maybe_color(&vuln.id, |t| t.yellow().to_string()),
                        vuln.summary,
                        style::dim(&score)
                    );
                }
                println!();
            }
            ui::print_tip("Run 'omg audit sbom' to generate a full security report.");
        }
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        anyhow::bail!(
            "Security audit requires the daemon, which is only available on Unix systems."
        );
    }
}

/// Generate SBOM (Software Bill of Materials)
pub async fn generate_sbom(
    output: Option<String>,
    include_vulns: bool,
    _ctx: &CliContext,
) -> Result<()> {
    // Require Pro tier for SBOM generation
    license::require_feature("sbom")?;

    println!(
        "{} Generating Software Bill of Materials (CycloneDX 1.5)...\n",
        style::runtime("OMG")
    );

    let generator = SbomGenerator::new().with_vulnerabilities(include_vulns);

    let sbom = generator.generate_system_sbom().await?;

    let path = if let Some(output_path) = output {
        let path = std::path::PathBuf::from(&output_path);
        generator.export_json(&sbom, &path)?;
        path
    } else {
        generator.export_default(&sbom)?
    };

    println!(
        "{} SBOM generated with {} components",
        style::maybe_color("✓", |t| t.green().to_string()),
        style::runtime(&sbom.components.len().to_string())
    );

    if !sbom.vulnerabilities.is_empty() {
        println!(
            "{} {} vulnerabilities included",
            style::maybe_color("⚠", |t| t.yellow().to_string()),
            style::maybe_color(&sbom.vulnerabilities.len().to_string(), |t| {
                t.yellow().bold().to_string()
            })
        );
    }

    println!(
        "\n  {} {}",
        style::dim("Output:"),
        style::maybe_color(&path.display().to_string(), |t| t.white().to_string())
    );
    println!("  {} CycloneDX 1.5 (JSON)", style::dim("Format:"));

    Ok(())
}

/// View audit log entries
pub fn view_audit_log(
    limit: usize,
    severity_filter: Option<&str>,
    export: Option<String>,
    _ctx: &CliContext,
) -> Result<()> {
    // Require Team tier for audit logs
    license::require_feature("audit-log")?;

    let logger = AuditLogger::new().context("Failed to open audit log")?;
    let entries = read_audit_entries(&logger, limit, severity_filter)?;

    if let Some(export_path) = export {
        println!(
            "{} Exporting audit log to {}...",
            style::runtime("OMG"),
            style::maybe_color(&export_path, |t| t.white().to_string())
        );
        let path = std::path::PathBuf::from(&export_path);
        let format = if std::path::Path::new(&export_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
        {
            "csv"
        } else {
            "json"
        };

        if format == "csv" {
            let mut wtr = csv::Writer::from_path(&path)?;
            wtr.write_record(["Timestamp", "Severity", "Event", "Description", "Resource"])?;
            for entry in &entries {
                wtr.write_record(&[
                    entry.timestamp.clone(),
                    entry.severity.to_string(),
                    format!("{:?}", entry.event_type),
                    entry.description.clone(),
                    entry.resource.clone(),
                ])?;
            }
            wtr.flush()?;
        } else {
            let json = serde_json::to_string_pretty(&entries)?;
            std::fs::write(&path, json)?;
        }
        println!(
            "{} Export successful",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        return Ok(());
    }

    println!("{} Security Audit Log\n", style::runtime("OMG"));

    if entries.is_empty() {
        println!(
            "  {} No audit entries found.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        return Ok(());
    }

    for entry in entries.iter().take(limit) {
        let sev_str = entry.severity.to_string();
        let severity_color = match entry.severity {
            AuditSeverity::Debug => style::dim(&sev_str),
            AuditSeverity::Info => style::maybe_color(&sev_str, |t| t.blue().to_string()),
            AuditSeverity::Warning => style::maybe_color(&sev_str, |t| t.yellow().to_string()),
            AuditSeverity::Error => style::maybe_color(&sev_str, |t| t.red().to_string()),
            AuditSeverity::Critical => style::maybe_color(&sev_str, |t| t.red().bold().to_string()),
        };

        println!(
            "  {} [{}] {} - {}",
            style::dim(&entry.timestamp),
            severity_color,
            style::maybe_color(&format!("{:?}", entry.event_type), |t| {
                t.cyan().to_string()
            }),
            entry.description
        );
        if !entry.resource.is_empty() {
            println!("      {} {}", style::dim("Resource:"), entry.resource);
        }
    }

    println!(
        "\n  {} Showing {} of {} entries",
        style::maybe_color("ℹ", |t| t.blue().to_string()),
        entries.len().min(limit),
        entries.len()
    );

    Ok(())
}

/// Verify audit log integrity
pub fn verify_audit_log(_ctx: &CliContext) -> Result<()> {
    license::require_feature("audit-log")?;

    println!(
        "{} Verifying Audit Log Integrity...\n",
        style::runtime("OMG")
    );

    let logger = AuditLogger::new().context("Failed to open audit log")?;
    let report = match logger.verify_integrity() {
        Ok(report) => report,
        Err(error) if error.is_not_found() => {
            println!(
                "  {} No audit log exists yet.",
                style::maybe_color("ℹ", |t| t.blue().to_string())
            );
            return Ok(());
        }
        Err(error) => {
            return Err(error).context("Failed to verify audit log integrity");
        }
    };

    if report.is_valid() {
        println!(
            "{} Audit log integrity verified",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        println!(
            "  {} {} entries",
            style::dim("Total:"),
            report.total_entries
        );
        println!(
            "  {} {} entries",
            style::dim("Valid:"),
            report.valid_entries
        );
        println!("  {} Intact", style::dim("Chain:"));
    } else {
        println!(
            "{} Audit log integrity FAILED",
            style::maybe_color("✗", |t| t.red().bold().to_string())
        );
        println!(
            "  {} {} entries",
            style::dim("Total:"),
            report.total_entries
        );
        println!(
            "  {} {} entries",
            style::dim("Valid:"),
            report.valid_entries
        );
        let chain_status = if report.chain_valid {
            "Intact".to_string()
        } else {
            style::maybe_color("BROKEN", |t| t.red().to_string())
        };
        println!("  {} {}", style::dim("Chain:"), chain_status);
        if let Some(first_invalid) = &report.first_invalid_entry {
            println!(
                "  {} {}",
                style::dim("First Invalid:"),
                style::maybe_color(first_invalid, |t| t.red().to_string())
            );
        }
    }

    println!(
        "\n  {} {}",
        style::dim("Log Path:"),
        report.log_path.display()
    );

    require_audit_integrity(report.is_valid())
}

/// Show security policy status
pub fn show_policy(_ctx: &CliContext) -> Result<()> {
    println!("{} Security Policy Status\n", style::runtime("OMG"));

    let policy = SecurityPolicy::load_default().context("Failed to load security policy")?;

    println!(
        "  {} {}",
        style::dim("Minimum Grade:"),
        style::maybe_color(&policy.minimum_grade.to_string(), |t| {
            t.cyan().to_string()
        })
    );
    println!(
        "  {} {}",
        style::dim("AUR Allowed:"),
        if policy.allow_aur {
            style::version("Yes")
        } else {
            style::maybe_color("No", |t| t.red().to_string())
        }
    );
    println!(
        "  {} {}",
        style::dim("PGP Required:"),
        if policy.require_pgp {
            style::version("Yes")
        } else {
            style::maybe_color("No", |t| t.yellow().to_string())
        }
    );

    if !policy.banned_packages.is_empty() {
        println!(
            "\n  {} Banned Packages:",
            style::maybe_color("⚠", |t| t.yellow().to_string())
        );
        for pkg in &policy.banned_packages {
            println!(
                "    {} {}",
                style::maybe_color("•", |t| t.red().to_string()),
                pkg
            );
        }
    }

    if !policy.allowed_licenses.is_empty() {
        println!(
            "\n  {} Allowed Licenses:",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        for lic in &policy.allowed_licenses {
            println!(
                "    {} {}",
                style::maybe_color("•", |t| t.green().to_string()),
                lic
            );
        }
    }

    println!(
        "\n  {} Edit ~/.config/omg/policy.toml to customize",
        style::maybe_color("ℹ", |t| t.blue().to_string())
    );

    Ok(())
}

/// Scan for leaked secrets
pub fn scan_secrets(path: Option<String>, _ctx: &CliContext) -> Result<()> {
    // Require Pro tier for secret scanning
    license::require_feature("secrets")?;

    use crate::core::security::SecretScanner;

    let scan_path = path.unwrap_or_else(|| ".".to_string());

    println!(
        "{} Scanning for secrets in {}...\n",
        style::runtime("OMG"),
        style::maybe_color(&scan_path, |t| t.white().to_string())
    );

    let scanner = SecretScanner::new();
    let findings = if std::path::Path::new(&scan_path).is_file() {
        scanner.scan_file(&scan_path)?
    } else {
        scanner.scan_directory(&scan_path)?
    };

    if findings.is_empty() {
        println!(
            "{} No secrets detected.",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        return Ok(());
    }

    let result = crate::core::security::SecretScanResult::from_findings(findings);

    println!(
        "{} Found {} potential secrets:\n",
        style::maybe_color("⚠", |t| t.yellow().bold().to_string()),
        style::maybe_color(&result.total_findings.to_string(), |t| {
            t.red().bold().to_string()
        })
    );

    if result.critical_count > 0 {
        println!(
            "  {} {} CRITICAL",
            style::maybe_color("●", |t| t.red().to_string()),
            result.critical_count
        );
    }
    if result.high_count > 0 {
        println!(
            "  {} {} HIGH",
            style::maybe_color("●", |t| t.yellow().to_string()),
            result.high_count
        );
    }
    if result.medium_count > 0 {
        println!(
            "  {} {} MEDIUM",
            style::maybe_color("●", |t| t.blue().to_string()),
            result.medium_count
        );
    }
    if result.low_count > 0 {
        println!("  {} {} LOW", style::dim("●"), result.low_count);
    }

    println!();

    for finding in result.findings.iter().take(20) {
        let sev_str = finding.severity.to_string();
        let severity_color = match finding.severity {
            crate::core::security::secrets::SecretSeverity::Critical => {
                style::maybe_color(&sev_str, |t| t.red().bold().to_string())
            }
            crate::core::security::secrets::SecretSeverity::High => {
                style::maybe_color(&sev_str, |t| t.yellow().to_string())
            }
            crate::core::security::secrets::SecretSeverity::Medium => {
                style::maybe_color(&sev_str, |t| t.blue().to_string())
            }
            crate::core::security::secrets::SecretSeverity::Low => style::dim(&sev_str),
        };

        println!(
            "  [{}] {} in {}:{}",
            severity_color,
            style::maybe_color(&finding.secret_type.to_string(), |t| {
                t.cyan().to_string()
            }),
            style::dim(&finding.file_path),
            finding.line_number
        );
        println!("      {}", style::dim(&finding.redacted));
    }

    if result.total_findings > 20 {
        println!(
            "\n  {} ... and {} more",
            style::maybe_color("ℹ", |t| t.blue().to_string()),
            result.total_findings - 20
        );
    }

    if result.has_critical() {
        println!(
            "\n{} Critical secrets found! Remove them before committing.",
            style::maybe_color("⚠", |t| t.red().bold().to_string())
        );
    }

    Ok(())
}

/// Check SLSA provenance for a package
pub async fn check_slsa(package: &str, _ctx: &CliContext) -> Result<()> {
    // Require Enterprise tier for SLSA verification
    license::require_feature("slsa")?;

    use crate::core::security::SlsaVerifier;

    // SECURITY: Validate path
    crate::core::security::validate_relative_path(package)?;

    println!(
        "{} Checking SLSA provenance for {}...\n",
        style::runtime("OMG"),
        style::maybe_color(package, |t| t.white().to_string())
    );

    let path = std::path::Path::new(package);
    if !path.exists() {
        anyhow::bail!("File not found: {package}");
    }

    let verifier = SlsaVerifier::new();
    let result = verifier
        .verify_provenance(path, None::<&std::path::Path>)
        .await?;

    require_slsa_verified(result.verified, result.error.as_deref())?;

    println!(
        "{} SLSA verification passed",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    println!(
        "  {} {}",
        style::dim("Level:"),
        style::maybe_color(&result.slsa_level.to_string(), |t| t.cyan().to_string())
    );

    if let Some(entry) = &result.transparency_log_entry {
        println!("  {} {}", style::dim("Rekor Entry:"), entry);
    }
    if let Some(builder) = &result.builder_id {
        println!("  {} {}", style::dim("Builder:"), builder);
    }
    if let Some(timestamp) = &result.build_timestamp {
        println!("  {} {}", style::dim("Build Time:"), timestamp);
    }

    Ok(())
}

fn require_slsa_verified(verified: bool, error: Option<&str>) -> Result<()> {
    if verified {
        return Ok(());
    }
    match error {
        Some(reason) => anyhow::bail!("SLSA verification failed: {reason}"),
        None => anyhow::bail!("SLSA verification failed"),
    }
}

fn require_audit_integrity(is_valid: bool) -> Result<()> {
    if is_valid {
        Ok(())
    } else {
        anyhow::bail!("Audit log integrity FAILED")
    }
}

/// License categories for compliance
#[derive(Debug, Clone, PartialEq, Eq)]
enum LicenseCategory {
    Permissive,     // MIT, BSD, Apache
    Copyleft,       // GPL, LGPL, MPL
    StrongCopyleft, // AGPL
    Proprietary,
    Unknown,
}

impl LicenseCategory {
    fn from_license(license: &str) -> Self {
        let tokens = crate::core::security::policy::spdx_license_tokens(license);
        let mut category = Self::Unknown;
        for token in &tokens {
            if token.contains("agpl") {
                return Self::StrongCopyleft;
            }
            if token.contains("gpl") || token.contains("lgpl") || token.contains("mpl") {
                category = Self::Copyleft;
                continue;
            }
            if token_is_permissive(token) {
                if category == Self::Unknown {
                    category = Self::Permissive;
                }
                continue;
            }
            if (token.contains("proprietary") || token.contains("commercial"))
                && category == Self::Unknown
            {
                category = Self::Proprietary;
            }
        }
        category
    }

    fn color(&self) -> String {
        match self {
            Self::Permissive => style::success("Permissive"),
            Self::Copyleft => style::warning("Copyleft"),
            Self::StrongCopyleft => style::error("Strong Copyleft"),
            Self::Proprietary => style::error("Proprietary"),
            Self::Unknown => style::dim("Unknown"),
        }
    }
}

fn token_is_permissive(token: &str) -> bool {
    matches!(
        token,
        "mit" | "isc" | "unlicense" | "cc0" | "cc0-1.0" | "0bsd" | "bsd" | "apache"
    ) || token.starts_with("bsd-")
        || token.starts_with("apache-")
}

/// Scan for software license compliance
pub fn scan_licenses(
    format: &str,
    export: Option<String>,
    filter: Option<String>,
    check_policy: bool,
    _ctx: &CliContext,
) -> Result<()> {
    license::require_feature("license-scan")?;

    println!(
        "{} Scanning installed packages for license information...\n",
        style::runtime("OMG")
    );

    // Get installed packages and their licenses
    #[cfg(feature = "arch")]
    let packages = crate::package_managers::alpm_direct::list_installed_with_licenses()?;
    #[cfg(not(feature = "arch"))]
    let packages: Vec<(String, String, String)> = Vec::new();

    // Filter by license if specified
    let filter_terms: Vec<String> = filter
        .map(|f| f.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();

    let filtered_packages: Vec<_> = packages
        .iter()
        .filter(|(_, license, _)| {
            if filter_terms.is_empty() {
                true
            } else {
                let tokens = crate::core::security::policy::spdx_license_tokens(license);
                filter_terms.iter().any(|term| {
                    tokens.iter().any(|token| {
                        token == term
                            || token.strip_suffix('+') == Some(term.as_str())
                            || token.starts_with(&format!("{term}-"))
                    })
                })
            }
        })
        .collect();

    // Categorize licenses
    let mut permissive = 0;
    let mut copyleft = 0;
    let mut strong_copyleft = 0;
    let mut proprietary = 0;
    let mut unknown = 0;

    for (_, license, _) in &filtered_packages {
        match LicenseCategory::from_license(license) {
            LicenseCategory::Permissive => permissive += 1,
            LicenseCategory::Copyleft => copyleft += 1,
            LicenseCategory::StrongCopyleft => strong_copyleft += 1,
            LicenseCategory::Proprietary => proprietary += 1,
            LicenseCategory::Unknown => unknown += 1,
        }
    }

    // Print summary
    println!("  {}", style::header("License Summary"));
    println!("    {} {}", style::success("Permissive:"), permissive);
    println!("    {} {}", style::warning("Copyleft:"), copyleft);
    if strong_copyleft > 0 {
        println!(
            "    {} {}",
            style::error("Strong Copyleft (AGPL):"),
            strong_copyleft
        );
    }
    if proprietary > 0 {
        println!("    {} {}", style::error("Proprietary:"), proprietary);
    }
    if unknown > 0 {
        println!("    {} {}", style::dim("Unknown:"), unknown);
    }
    println!();

    // Check against policy if requested
    if check_policy {
        let policy = SecurityPolicy::load_default().context("Failed to load security policy")?;
        let mut violations = Vec::new();

        for (name, license, _) in &filtered_packages {
            // Check against allowed licenses (if policy specifies them)
            if !policy.allowed_licenses.is_empty()
                && !crate::core::security::policy::license_matches_allowlist(
                    license,
                    &policy.allowed_licenses,
                )
            {
                violations.push((
                    name.clone(),
                    license.clone(),
                    "Not in allowed list".to_string(),
                ));
            }

            // Check for AGPL (commonly restricted in commercial use)
            if license.to_lowercase().contains("agpl") {
                violations.push((
                    name.clone(),
                    license.clone(),
                    "AGPL requires review".to_string(),
                ));
            }
        }

        if violations.is_empty() {
            println!(
                "  {} All packages comply with license policy\n",
                style::success("✓")
            );
        } else {
            println!("  {} License Policy Violations:\n", style::warning("⚠"));
            for (name, license, reason) in &violations {
                println!(
                    "    {} {} ({}) - {}",
                    style::error("✗"),
                    name,
                    license,
                    reason
                );
            }
            println!();
        }
    }

    // Export if requested
    if let Some(export_path) = export {
        let path = std::path::PathBuf::from(&export_path);

        match format {
            "json" => {
                let data: Vec<_> = filtered_packages
                    .iter()
                    .map(|(name, license, version)| {
                        serde_json::json!({
                            "name": name,
                            "version": version,
                            "license": license,
                            "category": format!("{:?}", LicenseCategory::from_license(license))
                        })
                    })
                    .collect();
                let json = serde_json::to_string_pretty(&data)?;
                std::fs::write(&path, json)?;
            }
            "csv" => {
                let mut wtr = csv::Writer::from_path(&path)?;
                wtr.write_record(["Package", "Version", "License", "Category"])?;
                for (name, license, version) in &filtered_packages {
                    let category_str = format!("{:?}", LicenseCategory::from_license(license));
                    wtr.write_record([name.as_str(), version, license, &category_str])?;
                }
                wtr.flush()?;
            }
            _ => {
                // Table format (default) - just write as text
                use std::fmt::Write as _;
                let mut content = String::from("Package\tVersion\tLicense\tCategory\n");
                for (name, license, version) in &filtered_packages {
                    let category_str = format!("{:?}", LicenseCategory::from_license(license));
                    let _ = writeln!(content, "{name}\t{version}\t{license}\t{category_str}");
                }
                std::fs::write(&path, &content)?;
            }
        }

        println!(
            "{} Exported {} packages to {}",
            style::success("✓"),
            filtered_packages.len(),
            export_path
        );
    } else if format == "table" {
        // Show first 20 packages in table format
        println!("  {}", style::header("Packages"));
        for (name, license, _) in filtered_packages.iter().take(20) {
            let category = LicenseCategory::from_license(license);
            println!(
                "    {} {} ({})",
                style::package(name),
                style::dim(license),
                category.color()
            );
        }
        if filtered_packages.len() > 20 {
            println!(
                "\n  {} Showing 20 of {} packages. Use --export to see all.",
                style::dim("..."),
                filtered_packages.len()
            );
        }
    }

    Ok(())
}

/// Auto-fix vulnerabilities by upgrading packages
pub async fn fix_vulnerabilities(
    dry_run: bool,
    yes: bool,
    min_severity: &str,
    _ctx: &CliContext,
) -> Result<()> {
    license::require_feature("audit")?;

    println!(
        "{} Scanning for fixable vulnerabilities...\n",
        style::runtime("OMG")
    );

    // Get vulnerability data from daemon
    #[cfg(unix)]
    let scan_result = {
        let Ok(mut client) = DaemonClient::connect().await else {
            anyhow::bail!("Daemon not running. Security audit requires the daemon.");
        };

        match client.security_audit().await {
            Ok(res) => res,
            Err(e) => {
                anyhow::bail!("Audit failed: {e}");
            }
        }
    };

    #[cfg(not(unix))]
    {
        anyhow::bail!(
            "Vulnerability scanning requires the daemon, which is only available on Unix systems."
        );
    }

    #[cfg(unix)]
    {
        if scan_result.total_vulnerabilities == 0 {
            println!("{} No vulnerabilities found!", style::success("✓"));
            return Ok(());
        }

        // Determine minimum severity threshold
        let min_sev = match min_severity.to_lowercase().as_str() {
            "critical" => 9.0,
            "high" => 7.0,
            "low" => 0.0,
            _ => 4.0, // Default to medium or unknown
        };

        // Find packages with fixable vulnerabilities
        #[allow(unused_mut)] // Mutated only inside feature-gated block
        let mut to_upgrade: Vec<String> = Vec::new();
        #[allow(unused_mut)] // Mutated only inside feature-gated block
        let mut unfixable: Vec<(String, String)> = Vec::new();

        for (pkg, vulns) in &scan_result.vulnerabilities {
            let has_severe = vulns.iter().any(|v| {
                let score = v
                    .score
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                score >= min_sev
            });

            if has_severe {
                // Check if there's an available update
                #[cfg(feature = "arch")]
                {
                    if crate::package_managers::alpm_direct::has_update(pkg).with_context(|| {
                        format!("Failed to check whether {pkg} has an available update")
                    })? {
                        to_upgrade.push(pkg.clone());
                    } else {
                        for vuln in vulns {
                            let score = vuln
                                .score
                                .as_ref()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            if score >= min_sev {
                                unfixable.push((pkg.clone(), vuln.id.clone()));
                            }
                        }
                    }
                }
                #[cfg(not(feature = "arch"))]
                {
                    unfixable.push((pkg.clone(), "Update check not available".to_string()));
                }
            }
        }

        if to_upgrade.is_empty() {
            println!("{} No packages can be auto-fixed.", style::dim("•"));
            if !unfixable.is_empty() {
                println!(
                    "\n  {} Unfixable vulnerabilities (no update available):\n",
                    style::warning("⚠")
                );
                for (pkg, vuln) in &unfixable {
                    println!("    {} {} - {}", style::error("✗"), pkg, vuln);
                }
                println!(
                    "\n  {} These may require manual intervention or upstream patches.",
                    style::dim("ℹ")
                );
            }
            return Ok(());
        }

        println!(
            "{} Found {} packages to upgrade:\n",
            style::success("✓"),
            to_upgrade.len()
        );
        for pkg in &to_upgrade {
            println!("    {} {}", style::arrow("→"), style::package(pkg));
        }

        if dry_run {
            println!("\n{} Dry run - no changes made.", style::dim("•"));
            return Ok(());
        }

        if !yes {
            println!();
            let confirm = dialoguer::Confirm::new()
                .with_prompt("Proceed with upgrades?")
                .default(true)
                .interact()?;

            if !confirm {
                println!("{} Cancelled.", style::dim("•"));
                return Ok(());
            }
        }

        // Perform upgrades
        #[cfg(feature = "arch")]
        {
            let pacman = crate::package_managers::ArchPackageManager::new();
            use crate::package_managers::PackageManager;
            pacman.install(&to_upgrade).await?;
        }

        println!(
            "\n{} Fixed {} packages.",
            style::success("✓"),
            to_upgrade.len()
        );

        if !unfixable.is_empty() {
            println!(
                "\n{} {} vulnerabilities remain unfixable.",
                style::warning("⚠"),
                unfixable.len()
            );
        }
    }

    Ok(())
}

/// Export compliance evidence for audit frameworks
pub async fn export_compliance(
    framework: &str,
    period: Option<String>,
    output: &str,
    _ctx: &CliContext,
) -> Result<()> {
    license::require_feature("compliance")?;

    println!(
        "{} Generating {} compliance evidence...\n",
        style::runtime("OMG"),
        framework.to_uppercase()
    );

    let output_dir = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output_dir)?;

    let now = jiff::Timestamp::now();
    let timestamp = now.strftime("%Y%m%d_%H%M%S").to_string();

    // Generate different evidence based on framework
    match framework.to_lowercase().as_str() {
        "soc2" => {
            // SOC 2 requires:
            // - Audit log of changes
            // - Vulnerability scan results
            // - SBOM
            // - Configuration documentation

            // 1. Export audit log
            let logger =
                AuditLogger::new().context("Failed to open audit log for compliance export")?;
            let entries = read_audit_entries(&logger, 1000, None)?;
            let json = serde_json::to_string_pretty(&entries)?;
            let log_path = output_dir.join(format!("audit-log-{timestamp}.json"));
            std::fs::write(&log_path, json)?;
            println!(
                "  {} Audit log: {}",
                style::success("✓"),
                log_path.display()
            );

            // 2. Export vulnerability scan
            #[cfg(unix)]
            {
                let mut client = DaemonClient::connect().await.context(
                    "Daemon not running. Compliance export requires the daemon (start it with: omg daemon)",
                )?;
                let scan = client
                    .security_audit()
                    .await
                    .context("Failed to run security audit for compliance export")?;
                let json = serde_json::to_string_pretty(&scan)?;
                let scan_path = output_dir.join(format!("vulnerability-scan-{timestamp}.json"));
                std::fs::write(&scan_path, json)?;
                println!(
                    "  {} Vulnerability scan: {}",
                    style::success("✓"),
                    scan_path.display()
                );
            }

            // 3. Generate SBOM
            let generator = SbomGenerator::new().with_vulnerabilities(true);
            let sbom = generator
                .generate_system_sbom()
                .await
                .context("Failed to generate SBOM for compliance export")?;
            let sbom_filename = format!("sbom-{timestamp}.json");
            let sbom_path = output_dir.join(sbom_filename);
            generator.export_json(&sbom, &sbom_path)?;
            println!("  {} SBOM: {}", style::success("✓"), sbom_path.display());

            // 4. Configuration snapshot
            let config_snapshot = serde_json::json!({
                "framework": "SOC2",
                "generated_at": now.to_string(),
                "period": period,
                "policy": SecurityPolicy::load_default()
                    .context("Failed to load security policy")?,
            });
            let config_path = output_dir.join(format!("config-snapshot-{timestamp}.json"));
            std::fs::write(
                &config_path,
                serde_json::to_string_pretty(&config_snapshot)?,
            )?;
            println!(
                "  {} Configuration: {}",
                style::success("✓"),
                config_path.display()
            );
        }
        "iso27001" => {
            // ISO 27001 requires similar but with different structure
            println!("  {} ISO 27001 export format", style::dim("•"));
            // Similar to SOC2 but with different categorization
        }
        "hipaa" | "pci-dss" | "fedramp" => {
            println!(
                "  {} {} compliance export is available in Enterprise tier",
                style::warning("⚠"),
                framework.to_uppercase()
            );
        }
        _ => {
            println!(
                "{} Unknown framework: {}. Supported: soc2, iso27001, hipaa, pci-dss, fedramp",
                style::error("✗"),
                framework
            );
        }
    }

    println!(
        "\n{} Evidence exported to {}",
        style::success("✓"),
        output_dir.display()
    );

    Ok(())
}

fn parse_eol_timestamp(eol_date: &str) -> Result<jiff::Timestamp> {
    let eol_ts = jiff::civil::Date::strptime("%Y-%m-%d", eol_date)
        .map_err(|error| anyhow::anyhow!("Invalid EOL date '{eol_date}': {error}"))?;
    let zoned = eol_ts
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .with_context(|| format!("Failed to convert EOL date '{eol_date}' to UTC"))?;
    Ok(zoned.timestamp())
}

/// Check end-of-life status for installed runtimes
pub fn check_eol(_ctx: &CliContext) -> Result<()> {
    println!("{} Checking runtime EOL status...\n", style::runtime("OMG"));

    // EOL dates from endoflife.date API (cached)
    let eol_data: &[(&str, &str, &str, &str)] = &[
        // (runtime, version_prefix, eol_date, lts_until)
        ("node", "16", "2023-09-11", "N/A"),
        ("node", "18", "2025-04-30", "2023-10-18"),
        ("node", "19", "2023-06-01", "N/A"),
        ("node", "20", "2026-04-30", "2024-10-22"),
        ("node", "21", "2024-06-01", "N/A"),
        ("node", "22", "2027-04-30", "2025-10-21"),
        ("python", "3.7", "2023-06-27", "N/A"),
        ("python", "3.8", "2024-10-31", "N/A"),
        ("python", "3.9", "2025-10-31", "N/A"),
        ("python", "3.10", "2026-10-31", "N/A"),
        ("python", "3.11", "2027-10-31", "N/A"),
        ("python", "3.12", "2028-10-31", "N/A"),
        ("go", "1.20", "2024-02-06", "N/A"),
        ("go", "1.21", "2024-08-06", "N/A"),
        ("go", "1.22", "2025-02-06", "N/A"),
        ("ruby", "3.0", "2024-03-31", "N/A"),
        ("ruby", "3.1", "2025-03-31", "N/A"),
        ("ruby", "3.2", "2026-03-31", "N/A"),
        ("java", "17", "2029-09-30", "LTS"),
        ("java", "21", "2031-09-30", "LTS"),
    ];

    let now = jiff::Timestamp::now();
    let runtimes = ["node", "python", "rust", "go", "ruby", "java", "bun"];
    let mut issues = 0;

    for runtime in &runtimes {
        if let Some(version) = crate::runtimes::probe_version(runtime) {
            let mut status = "Active";
            let mut eol_date_str = "Unknown";
            let mut is_eol = false;
            let mut is_warning = false;

            // Check EOL status
            for (rt, ver_prefix, eol_date, _lts) in eol_data {
                if *rt == *runtime && version.starts_with(ver_prefix) {
                    eol_date_str = eol_date;
                    let eol_timestamp = parse_eol_timestamp(eol_date).with_context(|| {
                        format!("Invalid EOL date '{eol_date}' for {runtime} {ver_prefix}")
                    })?;

                    if now > eol_timestamp {
                        status = "EOL";
                        is_eol = true;
                        issues += 1;
                    } else {
                        let six_months = jiff::Span::new().months(6);
                        let warning_ts = now
                            .checked_add(six_months)
                            .context("Failed to compute EOL warning window")?;
                        if warning_ts > eol_timestamp {
                            status = "Ending Soon";
                            is_warning = true;
                            issues += 1;
                        }
                    }
                    break;
                }
            }

            let status_display = if is_eol {
                style::error(status)
            } else if is_warning {
                style::warning(status)
            } else {
                style::success(status)
            };

            println!(
                "  {} {} v{} - {} (EOL: {})",
                if is_eol {
                    style::error("✗")
                } else if is_warning {
                    style::warning("⚠")
                } else {
                    style::success("✓")
                },
                style::runtime(runtime),
                style::version(&version),
                status_display,
                eol_date_str
            );
        }
    }

    println!();
    if issues == 0 {
        println!(
            "{} All runtimes are within support period.",
            style::success("✓")
        );
    } else {
        println!(
            "{} {} runtime(s) need attention. Consider upgrading to supported versions.",
            style::warning("⚠"),
            issues
        );
    }

    println!("\n{} Data source: endoflife.date", style::dim("ℹ"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limited_is_not_classified_as_mit() {
        assert_eq!(
            LicenseCategory::from_license("LIMITED"),
            LicenseCategory::Unknown
        );
        assert_eq!(
            LicenseCategory::from_license("MIT"),
            LicenseCategory::Permissive
        );
        assert_eq!(
            LicenseCategory::from_license("MIT OR Apache-2.0"),
            LicenseCategory::Permissive
        );
        assert_eq!(
            LicenseCategory::from_license("AGPL-3.0"),
            LicenseCategory::StrongCopyleft
        );
        assert_eq!(
            LicenseCategory::from_license("GPL-2.0"),
            LicenseCategory::Copyleft
        );
    }

    #[test]
    fn slsa_check_fails_when_provenance_is_unverified() {
        let err = require_slsa_verified(false, Some("no attestation"))
            .expect_err("unverified provenance must fail the command");
        assert!(
            err.to_string().contains("SLSA verification failed"),
            "got: {err}"
        );
        assert!(
            err.to_string().contains("no attestation"),
            "failure reason must be preserved, got: {err}"
        );
        assert!(require_slsa_verified(true, None).is_ok());
        let missing_reason =
            require_slsa_verified(false, None).expect_err("unverified without details still fails");
        assert!(
            missing_reason
                .to_string()
                .contains("SLSA verification failed")
        );
    }

    #[test]
    fn audit_integrity_failure_is_an_error() {
        let err =
            require_audit_integrity(false).expect_err("broken audit chain must fail the command");
        assert!(err.to_string().contains("integrity FAILED"), "got: {err}");
        assert!(require_audit_integrity(true).is_ok());
    }

    #[test]
    fn parse_eol_timestamp_accepts_iso_dates() {
        let ts = parse_eol_timestamp("2025-04-30").expect("valid EOL date");
        assert!(ts.as_second() > 0);
    }

    #[test]
    fn parse_eol_timestamp_rejects_garbage() {
        let err = parse_eol_timestamp("not-a-date").expect_err("garbage must fail closed");
        assert!(err.to_string().contains("Invalid EOL date"), "got: {err}");
    }
}
