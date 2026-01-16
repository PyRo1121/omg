use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Types of secrets that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretType {
    AwsAccessKey,
    AwsSecretKey,
    GithubToken,
    GitlabToken,
    SlackToken,
    SlackWebhook,
    PrivateKey,
    GenericApiKey,
    GenericPassword,
    JwtToken,
    GoogleApiKey,
    StripeKey,
    TwilioKey,
    SendgridKey,
    NpmToken,
    PypiToken,
    DockerHubToken,
    AzureKey,
    HerokuApiKey,
    DigitalOceanToken,
}

impl std::fmt::Display for SecretType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AwsAccessKey => write!(f, "AWS Access Key"),
            Self::AwsSecretKey => write!(f, "AWS Secret Key"),
            Self::GithubToken => write!(f, "GitHub Token"),
            Self::GitlabToken => write!(f, "GitLab Token"),
            Self::SlackToken => write!(f, "Slack Token"),
            Self::SlackWebhook => write!(f, "Slack Webhook"),
            Self::PrivateKey => write!(f, "Private Key"),
            Self::GenericApiKey => write!(f, "API Key"),
            Self::GenericPassword => write!(f, "Password"),
            Self::JwtToken => write!(f, "JWT Token"),
            Self::GoogleApiKey => write!(f, "Google API Key"),
            Self::StripeKey => write!(f, "Stripe Key"),
            Self::TwilioKey => write!(f, "Twilio Key"),
            Self::SendgridKey => write!(f, "SendGrid Key"),
            Self::NpmToken => write!(f, "NPM Token"),
            Self::PypiToken => write!(f, "PyPI Token"),
            Self::DockerHubToken => write!(f, "Docker Hub Token"),
            Self::AzureKey => write!(f, "Azure Key"),
            Self::HerokuApiKey => write!(f, "Heroku API Key"),
            Self::DigitalOceanToken => write!(f, "DigitalOcean Token"),
        }
    }
}

/// A detected secret finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    pub secret_type: SecretType,
    pub file_path: String,
    pub line_number: usize,
    pub matched_text: String,
    pub redacted: String,
    pub severity: SecretSeverity,
}

/// Severity of the secret finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretSeverity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl std::fmt::Display for SecretSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Pattern definition for secret detection
struct SecretPattern {
    secret_type: SecretType,
    pattern: Regex,
    severity: SecretSeverity,
}

/// Secret scanner for detecting leaked credentials
pub struct SecretScanner {
    patterns: Vec<SecretPattern>,
}

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretScanner {
    pub fn new() -> Self {
        let patterns = vec![
            // AWS
            SecretPattern {
                secret_type: SecretType::AwsAccessKey,
                pattern: Regex::new(r"(AKIA[0-9A-Z]{16})").unwrap(),
                severity: SecretSeverity::Critical,
            },
            SecretPattern {
                secret_type: SecretType::AwsSecretKey,
                pattern: Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key['"]?\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})"#).unwrap(),
                severity: SecretSeverity::Critical,
            },
            // GitHub
            SecretPattern {
                secret_type: SecretType::GithubToken,
                pattern: Regex::new(r"(ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|gho_[a-zA-Z0-9]{36}|ghu_[a-zA-Z0-9]{36}|ghs_[a-zA-Z0-9]{36}|ghr_[a-zA-Z0-9]{36})").unwrap(),
                severity: SecretSeverity::Critical,
            },
            // GitLab
            SecretPattern {
                secret_type: SecretType::GitlabToken,
                pattern: Regex::new(r"(glpat-[a-zA-Z0-9\-]{20,})").unwrap(),
                severity: SecretSeverity::Critical,
            },
            // Slack
            SecretPattern {
                secret_type: SecretType::SlackToken,
                pattern: Regex::new(r"(xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*)").unwrap(),
                severity: SecretSeverity::High,
            },
            SecretPattern {
                secret_type: SecretType::SlackWebhook,
                pattern: Regex::new(r"https://hooks\.slack\.com/services/T[a-zA-Z0-9_]+/B[a-zA-Z0-9_]+/[a-zA-Z0-9_]+").unwrap(),
                severity: SecretSeverity::High,
            },
            // Private Keys
            SecretPattern {
                secret_type: SecretType::PrivateKey,
                pattern: Regex::new(r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----").unwrap(),
                severity: SecretSeverity::Critical,
            },
            // JWT
            SecretPattern {
                secret_type: SecretType::JwtToken,
                pattern: Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*").unwrap(),
                severity: SecretSeverity::Medium,
            },
            // Google
            SecretPattern {
                secret_type: SecretType::GoogleApiKey,
                pattern: Regex::new(r"AIza[0-9A-Za-z\-_]{35}").unwrap(),
                severity: SecretSeverity::High,
            },
            // Stripe
            SecretPattern {
                secret_type: SecretType::StripeKey,
                pattern: Regex::new(r"(sk_live_[0-9a-zA-Z]{24}|rk_live_[0-9a-zA-Z]{24})").unwrap(),
                severity: SecretSeverity::Critical,
            },
            // Twilio
            SecretPattern {
                secret_type: SecretType::TwilioKey,
                pattern: Regex::new(r"SK[0-9a-fA-F]{32}").unwrap(),
                severity: SecretSeverity::High,
            },
            // SendGrid
            SecretPattern {
                secret_type: SecretType::SendgridKey,
                pattern: Regex::new(r"SG\.[a-zA-Z0-9_-]{22}\.[a-zA-Z0-9_-]{43}").unwrap(),
                severity: SecretSeverity::High,
            },
            // NPM
            SecretPattern {
                secret_type: SecretType::NpmToken,
                pattern: Regex::new(r"npm_[a-zA-Z0-9]{36}").unwrap(),
                severity: SecretSeverity::High,
            },
            // PyPI
            SecretPattern {
                secret_type: SecretType::PypiToken,
                pattern: Regex::new(r"pypi-AgEIcHlwaS5vcmc[A-Za-z0-9\-_]{50,}").unwrap(),
                severity: SecretSeverity::High,
            },
            // Docker Hub
            SecretPattern {
                secret_type: SecretType::DockerHubToken,
                pattern: Regex::new(r"dckr_pat_[a-zA-Z0-9_-]{27}").unwrap(),
                severity: SecretSeverity::High,
            },
            // Heroku
            SecretPattern {
                secret_type: SecretType::HerokuApiKey,
                pattern: Regex::new(r#"(?i)heroku[_-]?api[_-]?key['"]?\s*[:=]\s*['"]?([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})"#).unwrap(),
                severity: SecretSeverity::High,
            },
            // DigitalOcean
            SecretPattern {
                secret_type: SecretType::DigitalOceanToken,
                pattern: Regex::new(r"dop_v1_[a-f0-9]{64}").unwrap(),
                severity: SecretSeverity::High,
            },
            // Generic patterns (lower priority)
            SecretPattern {
                secret_type: SecretType::GenericApiKey,
                pattern: Regex::new(r#"(?i)(api[_-]?key|apikey)['"]?\s*[:=]\s*['"]?([a-zA-Z0-9_-]{20,})"#).unwrap(),
                severity: SecretSeverity::Medium,
            },
            SecretPattern {
                secret_type: SecretType::GenericPassword,
                pattern: Regex::new(r#"(?i)(password|passwd|pwd)['"]?\s*[:=]\s*['"]?([^\s'"]{8,})"#).unwrap(),
                severity: SecretSeverity::Medium,
            },
        ];

        Self { patterns }
    }

    /// Scan a file for secrets
    pub fn scan_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SecretFinding>> {
        let content = std::fs::read_to_string(&path)?;
        let path_str = path.as_ref().display().to_string();

        self.scan_content(&content, &path_str)
    }

    /// Scan content for secrets
    pub fn scan_content(&self, content: &str, source: &str) -> Result<Vec<SecretFinding>> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for pattern in &self.patterns {
                if let Some(captures) = pattern.pattern.captures(line) {
                    let matched = captures.get(0).map_or("", |m| m.as_str());

                    // Skip if it looks like a placeholder or example
                    if Self::is_placeholder(matched) {
                        continue;
                    }

                    findings.push(SecretFinding {
                        secret_type: pattern.secret_type.clone(),
                        file_path: source.to_string(),
                        line_number: line_num + 1,
                        matched_text: matched.to_string(),
                        redacted: Self::redact(matched),
                        severity: pattern.severity,
                    });
                }
            }
        }

        Ok(findings)
    }

    /// Scan a directory recursively for secrets
    pub fn scan_directory<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SecretFinding>> {
        let mut findings = Vec::new();

        self.scan_directory_recursive(path.as_ref(), &mut findings)?;

        Ok(findings)
    }

    fn scan_directory_recursive(
        &self,
        path: &Path,
        findings: &mut Vec<SecretFinding>,
    ) -> Result<()> {
        if !path.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip common non-text directories
            if entry_path.is_dir() {
                let dir_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if [
                    "node_modules",
                    ".git",
                    "target",
                    "vendor",
                    "__pycache__",
                    ".venv",
                    "venv",
                ]
                .contains(&dir_name)
                {
                    continue;
                }

                self.scan_directory_recursive(&entry_path, findings)?;
            } else if Self::is_scannable_file(&entry_path)
                && let Ok(file_findings) = self.scan_file(&entry_path)
            {
                findings.extend(file_findings);
            }
        }

        Ok(())
    }

    /// Check if a file should be scanned
    fn is_scannable_file(path: &Path) -> bool {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let scannable_extensions = [
            "rs",
            "py",
            "js",
            "ts",
            "jsx",
            "tsx",
            "go",
            "rb",
            "java",
            "kt",
            "c",
            "cpp",
            "h",
            "hpp",
            "cs",
            "php",
            "sh",
            "bash",
            "zsh",
            "yaml",
            "yml",
            "json",
            "toml",
            "ini",
            "cfg",
            "conf",
            "config",
            "env",
            "properties",
            "xml",
            "md",
            "txt",
        ];

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check for dotfiles that might contain secrets
        let sensitive_files = [
            ".env",
            ".env.local",
            ".env.production",
            ".env.development",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".gitconfig",
            "credentials",
            "secrets",
            "config",
        ];

        scannable_extensions.contains(&extension)
            || sensitive_files.iter().any(|f| file_name.contains(f))
    }

    /// Check if a match looks like a placeholder
    fn is_placeholder(text: &str) -> bool {
        let placeholders = [
            "example",
            "sample",
            "test",
            "demo",
            "placeholder",
            "xxx",
            "yyy",
            "zzz",
            "abc",
            "123",
            "fake",
            "dummy",
            "your_",
            "my_",
            "<",
            ">",
            "${",
            "{{",
        ];

        let lower = text.to_lowercase();
        placeholders.iter().any(|p| lower.contains(p))
    }

    /// Redact a secret for safe display
    fn redact(text: &str) -> String {
        if text.len() <= 8 {
            return "*".repeat(text.len());
        }

        let visible_chars = 4;
        let prefix = &text[..visible_chars];
        let suffix = &text[text.len() - visible_chars..];
        let hidden_len = text.len() - (visible_chars * 2);

        format!("{}{}...{}", prefix, "*".repeat(hidden_len.min(10)), suffix)
    }
}

/// Scan result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretScanResult {
    pub total_findings: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub findings: Vec<SecretFinding>,
}

impl SecretScanResult {
    pub fn from_findings(findings: Vec<SecretFinding>) -> Self {
        let critical_count = findings
            .iter()
            .filter(|f| f.severity == SecretSeverity::Critical)
            .count();
        let high_count = findings
            .iter()
            .filter(|f| f.severity == SecretSeverity::High)
            .count();
        let medium_count = findings
            .iter()
            .filter(|f| f.severity == SecretSeverity::Medium)
            .count();
        let low_count = findings
            .iter()
            .filter(|f| f.severity == SecretSeverity::Low)
            .count();

        Self {
            total_findings: findings.len(),
            critical_count,
            high_count,
            medium_count,
            low_count,
            findings,
        }
    }

    pub fn has_critical(&self) -> bool {
        self.critical_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_key_detection() {
        let scanner = SecretScanner::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...";
        let findings = scanner.scan_content(content, "key.pem").unwrap();

        assert!(!findings.is_empty(), "Should detect private key");
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.secret_type, SecretType::PrivateKey))
        );
    }

    #[test]
    fn test_placeholder_ignored() {
        let scanner = SecretScanner::new();
        let content = "api_key = 'your_api_key_here'";
        let findings = scanner.scan_content(content, "config.py").unwrap();

        assert!(findings.is_empty(), "Should ignore placeholder values");
    }

    #[test]
    fn test_redaction() {
        let _scanner = SecretScanner::new();
        let redacted = SecretScanner::redact("secret_token_1234567890abcdef");

        assert!(redacted.contains('*'), "Should contain asterisks");
        assert!(!redacted.is_empty(), "Should produce output");
    }

    #[test]
    fn test_scan_result_from_findings() {
        let findings = vec![SecretFinding {
            secret_type: SecretType::PrivateKey,
            file_path: "test.pem".to_string(),
            line_number: 1,
            matched_text: "-----BEGIN PRIVATE KEY-----".to_string(),
            redacted: "****".to_string(),
            severity: SecretSeverity::Critical,
        }];

        let result = SecretScanResult::from_findings(findings);
        assert_eq!(result.total_findings, 1);
        assert_eq!(result.critical_count, 1);
        assert!(result.has_critical());
    }
}
