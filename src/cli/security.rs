use anyhow::Result;
use owo_colors::OwoColorize;

use crate::cli::{AuditCommands, CliContext, LocalCommandRunner, style, ui};
use crate::core::client::DaemonClient;
use crate::core::license;
use crate::core::security::{AuditLogger, AuditSeverity, SbomGenerator, SecurityPolicy};

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
            } => view_audit_log(*limit, severity.clone(), export.clone(), ctx),
            AuditCommands::Verify => verify_audit_log(ctx),
            AuditCommands::Policy => show_policy(ctx),
            AuditCommands::Slsa { package } => check_slsa(package, ctx).await,
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

    let Ok(mut client) = DaemonClient::connect().await else {
        ui::print_error("Daemon not running. Security audit requires the daemon.");
        return Ok(());
    };

    match client.security_audit().await {
        Ok(res) => {
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
        }
        Err(e) => {
            ui::print_error(format!("Audit failed: {e}"));
        }
    }

    Ok(())
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
    severity_filter: Option<String>,
    export: Option<String>,
    _ctx: &CliContext,
) -> Result<()> {
    // Require Team tier for audit logs
    license::require_feature("audit-log")?;

    let Ok(logger) = AuditLogger::new() else {
        println!(
            "  {} No audit log exists yet. Events will be logged during package operations.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        return Ok(());
    };

    let entries = if let Some(sev) = severity_filter {
        let min_severity = match sev.to_lowercase().as_str() {
            "debug" => AuditSeverity::Debug,
            "info" => AuditSeverity::Info,
            "warning" | "warn" => AuditSeverity::Warning,
            "error" => AuditSeverity::Error,
            "critical" => AuditSeverity::Critical,
            _ => {
                println!(
                    "{} Invalid severity: {}",
                    style::maybe_color("✗", |t| t.red().to_string()),
                    sev
                );
                return Ok(());
            }
        };
        logger.filter_by_severity(min_severity).unwrap_or_default()
    } else {
        logger.get_recent(limit).unwrap_or_default()
    };

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
    println!(
        "{} Verifying Audit Log Integrity...\n",
        style::runtime("OMG")
    );

    let Ok(logger) = AuditLogger::new() else {
        println!(
            "  {} No audit log exists yet.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        return Ok(());
    };
    let Ok(report) = logger.verify_integrity() else {
        println!(
            "  {} No audit log exists yet.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        return Ok(());
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

    Ok(())
}

/// Show security policy status
pub fn show_policy(_ctx: &CliContext) -> Result<()> {
    println!("{} Security Policy Status\n", style::runtime("OMG"));

    let policy = SecurityPolicy::load_default().unwrap_or_default();

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
    let findings = scanner.scan_directory(&scan_path)?;

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
        println!("{}", style::error(&format!("File not found: {package}")));
        return Ok(());
    }

    let verifier = SlsaVerifier::new()?;
    let result = verifier
        .verify_provenance(path, None::<&std::path::Path>)
        .await?;

    if result.verified {
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
    } else {
        println!(
            "{} SLSA verification failed",
            style::maybe_color("✗", |t| t.red().to_string())
        );
        if let Some(error) = &result.error {
            println!("  {} {}", style::dim("Reason:"), error);
        }
        println!(
            "\n  {} Package has no SLSA provenance attestation.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        println!(
            "  {} This is normal for AUR packages.",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
    }

    Ok(())
}
