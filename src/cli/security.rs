use crate::core::client::DaemonClient;
use anyhow::Result;
use colored::Colorize;

/// Perform security audit
pub async fn audit() -> Result<()> {
    println!(
        "{} Performing system security audit...\n",
        "OMG".cyan().bold()
    );

    let mut client = if let Ok(c) = DaemonClient::connect().await {
        c
    } else {
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
                    "{} Found {} vulnerabilities:\n",
                    "⚠".red().bold(),
                    res.total_vulnerabilities
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
