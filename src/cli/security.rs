use crate::core::client::DaemonClient;
use crate::core::security::{AuditLogger, AuditSeverity, SbomGenerator, SecurityPolicy};
use anyhow::Result;
use owo_colors::OwoColorize;

/// Perform security audit (vulnerability scan)
pub async fn scan() -> Result<()> {
    println!(
        "{} Performing system security audit...\n",
        "OMG".cyan().bold()
    );

    let Ok(mut client) = DaemonClient::connect().await else {
        println!(
            "{} Daemon not running. Security audit requires the daemon.",
            "✗".red()
        );
        return Ok(());
    };

    match client.security_audit().await {
        Ok(res) => {
            if res.total_vulnerabilities == 0 {
                println!(
                    "{} No vulnerabilities found in scanned packages.",
                    "✓".green()
                );
            } else {
                println!(
                    "{} Found {} vulnerabilities ({} high severity):\n",
                    "⚠".red().bold(),
                    res.total_vulnerabilities,
                    res.high_severity.to_string().red().bold()
                );
                for (pkg, vulns) in res.vulnerabilities {
                    println!("  {} ({} issues):", pkg.white().bold(), vulns.len());
                    for vuln in vulns {
                        let score = vuln
                            .score
                            .map(|s| format!(" [Score: {s}]"))
                            .unwrap_or_default();
                        println!(
                            "    {} {} - {}{}",
                            "→".red(),
                            vuln.id.yellow(),
                            vuln.summary,
                            score.dimmed()
                        );
                    }
                    println!();
                }
            }
        }
        Err(e) => {
            println!("{} Audit failed: {}", "✗".red(), e);
        }
    }

    Ok(())
}

/// Generate SBOM (Software Bill of Materials)
pub async fn generate_sbom(output: Option<String>, include_vulns: bool) -> Result<()> {
    println!(
        "{} Generating Software Bill of Materials (CycloneDX 1.5)...\n",
        "OMG".cyan().bold()
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
        "✓".green(),
        sbom.components.len().to_string().cyan().bold()
    );

    if !sbom.vulnerabilities.is_empty() {
        println!(
            "{} {} vulnerabilities included",
            "⚠".yellow(),
            sbom.vulnerabilities.len().to_string().yellow().bold()
        );
    }

    println!(
        "\n  {} {}",
        "Output:".dimmed(),
        path.display().to_string().white()
    );
    println!("  {} CycloneDX 1.5 (JSON)", "Format:".dimmed());

    Ok(())
}

/// View audit log entries
pub fn view_audit_log(limit: usize, severity_filter: Option<String>) -> Result<()> {
    println!("{} Security Audit Log\n", "OMG".cyan().bold());

    let logger = match AuditLogger::new() {
        Ok(l) => l,
        Err(_) => {
            println!(
                "  {} No audit log exists yet. Events will be logged during package operations.",
                "ℹ".blue()
            );
            return Ok(());
        }
    };

    let entries = if let Some(sev) = severity_filter {
        let min_severity = match sev.to_lowercase().as_str() {
            "debug" => AuditSeverity::Debug,
            "info" => AuditSeverity::Info,
            "warning" | "warn" => AuditSeverity::Warning,
            "error" => AuditSeverity::Error,
            "critical" => AuditSeverity::Critical,
            _ => {
                println!("{} Invalid severity: {}", "✗".red(), sev);
                return Ok(());
            }
        };
        logger.filter_by_severity(min_severity).unwrap_or_default()
    } else {
        logger.get_recent(limit).unwrap_or_default()
    };

    if entries.is_empty() {
        println!("  {} No audit entries found.", "ℹ".blue());
        return Ok(());
    }

    for entry in entries.iter().take(limit) {
        let severity_color = match entry.severity {
            AuditSeverity::Debug => entry.severity.to_string().dimmed().to_string(),
            AuditSeverity::Info => entry.severity.to_string().blue().to_string(),
            AuditSeverity::Warning => entry.severity.to_string().yellow().to_string(),
            AuditSeverity::Error => entry.severity.to_string().red().to_string(),
            AuditSeverity::Critical => entry.severity.to_string().red().bold().to_string(),
        };

        println!(
            "  {} [{}] {} - {}",
            entry.timestamp.dimmed(),
            severity_color,
            format!("{:?}", entry.event_type).cyan(),
            entry.description
        );
        if !entry.resource.is_empty() {
            println!("      {} {}", "Resource:".dimmed(), entry.resource);
        }
    }

    println!(
        "\n  {} Showing {} of {} entries",
        "ℹ".blue(),
        entries.len().min(limit),
        entries.len()
    );

    Ok(())
}

/// Verify audit log integrity
pub fn verify_audit_log() -> Result<()> {
    println!("{} Verifying Audit Log Integrity...\n", "OMG".cyan().bold());

    let logger = match AuditLogger::new() {
        Ok(l) => l,
        Err(_) => {
            println!("  {} No audit log exists yet.", "ℹ".blue());
            return Ok(());
        }
    };
    let report = match logger.verify_integrity() {
        Ok(r) => r,
        Err(_) => {
            println!("  {} No audit log exists yet.", "ℹ".blue());
            return Ok(());
        }
    };

    if report.is_valid() {
        println!("{} Audit log integrity verified", "✓".green());
        println!("  {} {} entries", "Total:".dimmed(), report.total_entries);
        println!("  {} {} entries", "Valid:".dimmed(), report.valid_entries);
        println!("  {} Intact", "Chain:".dimmed());
    } else {
        println!("{} Audit log integrity FAILED", "✗".red().bold());
        println!("  {} {} entries", "Total:".dimmed(), report.total_entries);
        println!("  {} {} entries", "Valid:".dimmed(), report.valid_entries);
        let chain_status = if report.chain_valid {
            "Intact".to_string()
        } else {
            "BROKEN".red().to_string()
        };
        println!("  {} {}", "Chain:".dimmed(), chain_status);
        if let Some(first_invalid) = &report.first_invalid_entry {
            println!("  {} {}", "First Invalid:".dimmed(), first_invalid.red());
        }
    }

    println!("\n  {} {}", "Log Path:".dimmed(), report.log_path.display());

    Ok(())
}

/// Show security policy status
pub fn show_policy() -> Result<()> {
    println!("{} Security Policy Status\n", "OMG".cyan().bold());

    let policy = SecurityPolicy::load_default().unwrap_or_default();

    println!(
        "  {} {}",
        "Minimum Grade:".dimmed(),
        format!("{}", policy.minimum_grade).cyan()
    );
    println!(
        "  {} {}",
        "AUR Allowed:".dimmed(),
        if policy.allow_aur {
            "Yes".green().to_string()
        } else {
            "No".red().to_string()
        }
    );
    println!(
        "  {} {}",
        "PGP Required:".dimmed(),
        if policy.require_pgp {
            "Yes".green().to_string()
        } else {
            "No".yellow().to_string()
        }
    );

    if !policy.banned_packages.is_empty() {
        println!("\n  {} Banned Packages:", "⚠".yellow());
        for pkg in &policy.banned_packages {
            println!("    {} {}", "•".red(), pkg);
        }
    }

    if !policy.allowed_licenses.is_empty() {
        println!("\n  {} Allowed Licenses:", "ℹ".blue());
        for lic in &policy.allowed_licenses {
            println!("    {} {}", "•".green(), lic);
        }
    }

    println!(
        "\n  {} Edit ~/.config/omg/policy.toml to customize",
        "ℹ".blue()
    );

    Ok(())
}

/// Scan for leaked secrets
pub fn scan_secrets(path: Option<String>) -> Result<()> {
    use crate::core::security::SecretScanner;

    let scan_path = path.unwrap_or_else(|| ".".to_string());

    println!(
        "{} Scanning for secrets in {}...\n",
        "OMG".cyan().bold(),
        scan_path.white()
    );

    let scanner = SecretScanner::new();
    let findings = scanner.scan_directory(&scan_path)?;

    if findings.is_empty() {
        println!("{} No secrets detected.", "✓".green());
        return Ok(());
    }

    let result = crate::core::security::SecretScanResult::from_findings(findings);

    println!(
        "{} Found {} potential secrets:\n",
        "⚠".yellow().bold(),
        result.total_findings.to_string().red().bold()
    );

    if result.critical_count > 0 {
        println!("  {} {} CRITICAL", "●".red(), result.critical_count);
    }
    if result.high_count > 0 {
        println!("  {} {} HIGH", "●".yellow(), result.high_count);
    }
    if result.medium_count > 0 {
        println!("  {} {} MEDIUM", "●".blue(), result.medium_count);
    }
    if result.low_count > 0 {
        println!("  {} {} LOW", "●".dimmed(), result.low_count);
    }

    println!();

    for finding in result.findings.iter().take(20) {
        let severity_color = match finding.severity {
            crate::core::security::secrets::SecretSeverity::Critical => {
                finding.severity.to_string().red().bold().to_string()
            }
            crate::core::security::secrets::SecretSeverity::High => {
                finding.severity.to_string().yellow().to_string()
            }
            crate::core::security::secrets::SecretSeverity::Medium => {
                finding.severity.to_string().blue().to_string()
            }
            crate::core::security::secrets::SecretSeverity::Low => {
                finding.severity.to_string().dimmed().to_string()
            }
        };

        println!(
            "  [{}] {} in {}:{}",
            severity_color,
            finding.secret_type.to_string().cyan(),
            finding.file_path.dimmed(),
            finding.line_number
        );
        println!("      {}", finding.redacted.dimmed());
    }

    if result.total_findings > 20 {
        println!(
            "\n  {} ... and {} more",
            "ℹ".blue(),
            result.total_findings - 20
        );
    }

    if result.has_critical() {
        println!(
            "\n{} Critical secrets found! Remove them before committing.",
            "⚠".red().bold()
        );
    }

    Ok(())
}

/// Check SLSA provenance for a package
pub async fn check_slsa(package: &str) -> Result<()> {
    use crate::core::security::SlsaVerifier;

    println!(
        "{} Checking SLSA provenance for {}...\n",
        "OMG".cyan().bold(),
        package.white()
    );

    let path = std::path::Path::new(package);
    if !path.exists() {
        println!("{} File not found: {}", "✗".red(), package);
        return Ok(());
    }

    let verifier = SlsaVerifier::new()?;
    let result = verifier
        .verify_provenance(path, None::<&std::path::Path>)
        .await?;

    if result.verified {
        println!("{} SLSA verification passed", "✓".green());
        println!(
            "  {} {}",
            "Level:".dimmed(),
            result.slsa_level.to_string().cyan()
        );

        if let Some(entry) = &result.transparency_log_entry {
            println!("  {} {}", "Rekor Entry:".dimmed(), entry);
        }
        if let Some(builder) = &result.builder_id {
            println!("  {} {}", "Builder:".dimmed(), builder);
        }
        if let Some(timestamp) = &result.build_timestamp {
            println!("  {} {}", "Build Time:".dimmed(), timestamp);
        }
    } else {
        println!("{} SLSA verification failed", "✗".red());
        if let Some(error) = &result.error {
            println!("  {} {}", "Reason:".dimmed(), error);
        }
        println!(
            "\n  {} Package has no SLSA provenance attestation.",
            "ℹ".blue()
        );
        println!("  {} This is normal for AUR packages.", "ℹ".blue());
    }

    Ok(())
}

/// Legacy function for backward compatibility
pub async fn audit() -> Result<()> {
    scan().await
}
