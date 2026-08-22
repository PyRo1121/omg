//! Secret detection using regex patterns and entropy analysis
//!
//! Scans files and content for accidentally committed secrets like API keys,
//! tokens, private keys, and credentials across 20 secret types.

use std::io;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// A detected secret finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    /// Category of credential detected.
    pub secret_type: SecretType,
    /// File the match came from (or the caller-provided source label).
    pub file_path: String,
    /// 1-based line number of the match.
    pub line_number: usize,
    /// Raw matched text. Treat as sensitive; do not print.
    pub matched_text: String,
    /// Masked form safe for display and reports.
    pub redacted: String,
    /// Severity assigned by the pattern that matched.
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
    pattern: &'static Regex,
    severity: SecretSeverity,
}

// Static regex patterns compiled once at first use
#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_AWS_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(AKIA[0-9A-Z]{16})").expect("valid AWS access key regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_AWS_SECRET_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key['"]?\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})"#)
        .expect("valid AWS secret key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|gho_[a-zA-Z0-9]{36}|ghu_[a-zA-Z0-9]{36}|ghs_[a-zA-Z0-9]{36}|ghr_[a-zA-Z0-9]{36})")
        .expect("valid GitHub token regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_GITLAB_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(glpat-[a-zA-Z0-9\-]{20,})").expect("valid GitLab token regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_SLACK_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*)")
        .expect("valid Slack token regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_SLACK_WEBHOOK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https://hooks\.slack\.com/services/T[a-zA-Z0-9_]+/B[a-zA-Z0-9_]+/[a-zA-Z0-9_]+")
        .expect("valid Slack webhook regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")
        .expect("valid private key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_JWT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*")
        .expect("valid JWT token regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_GOOGLE_API_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"AIza[0-9A-Za-z\-_]{35}").expect("valid Google API key regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_STRIPE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(sk_live_[0-9a-zA-Z]{24}|rk_live_[0-9a-zA-Z]{24})")
        .expect("valid Stripe key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_TWILIO_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"SK[0-9a-fA-F]{32}").expect("valid Twilio key regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_SENDGRID_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"SG\.[a-zA-Z0-9_-]{22}\.[a-zA-Z0-9_-]{43}").expect("valid SendGrid key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_NPM_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"npm_[a-zA-Z0-9]{36}").expect("valid NPM token regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_PYPI_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"pypi-AgEIcHlwaS5vcmc[A-Za-z0-9\-_]{50,}").expect("valid PyPI token regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_DOCKER_HUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"dckr_pat_[a-zA-Z0-9_-]{27}").expect("valid Docker Hub token regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_HEROKU_API_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)heroku[_-]?api[_-]?key['"]?\s*[:=]\s*['"]?([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})"#)
        .expect("valid Heroku API key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_DIGITALOCEAN_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dop_v1_[a-f0-9]{64}").expect("valid DigitalOcean token regex"));

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_GENERIC_API_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(api[_-]?key|apikey)['"]?\s*[:=]\s*['"]?([a-zA-Z0-9_-]{20,})"#)
        .expect("valid generic API key regex")
});

#[expect(clippy::expect_used)] // Static LazyLock<Regex> with compile-time-verified pattern
static RE_GENERIC_PASSWORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(password|passwd|pwd)['"]?\s*[:=]\s*['"]?([^\s'"]{8,})"#)
        .expect("valid generic password regex")
});

/// Static list of all secret patterns
static PATTERNS: LazyLock<Vec<SecretPattern>> = LazyLock::new(|| {
    vec![
        // AWS
        SecretPattern {
            secret_type: SecretType::AwsAccessKey,
            pattern: &RE_AWS_ACCESS_KEY,
            severity: SecretSeverity::Critical,
        },
        SecretPattern {
            secret_type: SecretType::AwsSecretKey,
            pattern: &RE_AWS_SECRET_KEY,
            severity: SecretSeverity::Critical,
        },
        // GitHub
        SecretPattern {
            secret_type: SecretType::GithubToken,
            pattern: &RE_GITHUB_TOKEN,
            severity: SecretSeverity::Critical,
        },
        // GitLab
        SecretPattern {
            secret_type: SecretType::GitlabToken,
            pattern: &RE_GITLAB_TOKEN,
            severity: SecretSeverity::Critical,
        },
        // Slack
        SecretPattern {
            secret_type: SecretType::SlackToken,
            pattern: &RE_SLACK_TOKEN,
            severity: SecretSeverity::High,
        },
        SecretPattern {
            secret_type: SecretType::SlackWebhook,
            pattern: &RE_SLACK_WEBHOOK,
            severity: SecretSeverity::High,
        },
        // Private Keys
        SecretPattern {
            secret_type: SecretType::PrivateKey,
            pattern: &RE_PRIVATE_KEY,
            severity: SecretSeverity::Critical,
        },
        // JWT
        SecretPattern {
            secret_type: SecretType::JwtToken,
            pattern: &RE_JWT,
            severity: SecretSeverity::Medium,
        },
        // Google
        SecretPattern {
            secret_type: SecretType::GoogleApiKey,
            pattern: &RE_GOOGLE_API_KEY,
            severity: SecretSeverity::High,
        },
        // Stripe
        SecretPattern {
            secret_type: SecretType::StripeKey,
            pattern: &RE_STRIPE_KEY,
            severity: SecretSeverity::Critical,
        },
        // Twilio
        SecretPattern {
            secret_type: SecretType::TwilioKey,
            pattern: &RE_TWILIO_KEY,
            severity: SecretSeverity::High,
        },
        // SendGrid
        SecretPattern {
            secret_type: SecretType::SendgridKey,
            pattern: &RE_SENDGRID_KEY,
            severity: SecretSeverity::High,
        },
        // NPM
        SecretPattern {
            secret_type: SecretType::NpmToken,
            pattern: &RE_NPM_TOKEN,
            severity: SecretSeverity::High,
        },
        // PyPI
        SecretPattern {
            secret_type: SecretType::PypiToken,
            pattern: &RE_PYPI_TOKEN,
            severity: SecretSeverity::High,
        },
        // Docker Hub
        SecretPattern {
            secret_type: SecretType::DockerHubToken,
            pattern: &RE_DOCKER_HUB_TOKEN,
            severity: SecretSeverity::High,
        },
        // Heroku
        SecretPattern {
            secret_type: SecretType::HerokuApiKey,
            pattern: &RE_HEROKU_API_KEY,
            severity: SecretSeverity::High,
        },
        // DigitalOcean
        SecretPattern {
            secret_type: SecretType::DigitalOceanToken,
            pattern: &RE_DIGITALOCEAN_TOKEN,
            severity: SecretSeverity::High,
        },
        // Generic patterns (lower priority)
        SecretPattern {
            secret_type: SecretType::GenericApiKey,
            pattern: &RE_GENERIC_API_KEY,
            severity: SecretSeverity::Medium,
        },
        SecretPattern {
            secret_type: SecretType::GenericPassword,
            pattern: &RE_GENERIC_PASSWORD,
            severity: SecretSeverity::Medium,
        },
    ]
});

/// Failures reading files or directories while scanning for secrets.
#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Failed to read '{path}'")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Directory nesting exceeds the maximum scan depth of {max} at '{path}'")]
    DepthExceeded { path: String, max: usize },
}

/// Secret scanner for detecting leaked credentials
pub struct SecretScanner;

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretScanner {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Scan a file for secrets
    pub fn scan_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SecretFinding>, SecretError> {
        let path_str = path.as_ref().display().to_string();
        let content = std::fs::read_to_string(&path).map_err(|source| SecretError::Read {
            path: path_str.clone(),
            source,
        })?;

        Ok(self.scan_content(&content, &path_str))
    }

    /// Scan content for secrets
    #[must_use]
    pub fn scan_content(&self, content: &str, source: &str) -> Vec<SecretFinding> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for pattern in PATTERNS.iter() {
                if let Some(captures) = pattern.pattern.captures(line) {
                    let matched = captures.get(0).map_or("", |m| m.as_str());

                    // Skip only when the captured secret value itself is an
                    // obvious placeholder; never drop findings merely because
                    // the value contains a placeholder-like substring.
                    let secret_value = captures
                        .get(captures.len() - 1)
                        .map_or(matched, |m| m.as_str());
                    if Self::is_placeholder(secret_value) {
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

        findings
    }

    /// Scan a directory recursively for secrets
    ///
    /// Symlinked directories are never followed (a cyclic symlink would
    /// otherwise recurse forever), and nesting deeper than
    /// [`MAX_SCAN_DEPTH`] fails closed rather than silently skipping files.
    pub fn scan_directory<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<SecretFinding>, SecretError> {
        let mut findings = Vec::new();

        self.scan_directory_recursive(path.as_ref(), &mut findings, 0)?;

        Ok(findings)
    }

    /// Maximum directory nesting followed during a scan. Deep enough for
    /// real source trees (after `node_modules`/`target` pruning) while
    /// bounding attacker-controlled recursion work.
    const MAX_SCAN_DEPTH: usize = 64;

    fn scan_directory_recursive(
        &self,
        path: &Path,
        findings: &mut Vec<SecretFinding>,
        depth: usize,
    ) -> Result<(), SecretError> {
        if depth > Self::MAX_SCAN_DEPTH {
            return Err(SecretError::DepthExceeded {
                path: path.display().to_string(),
                max: Self::MAX_SCAN_DEPTH,
            });
        }
        let path_str = path.display().to_string();
        for entry in std::fs::read_dir(path).map_err(|source| SecretError::Read {
            path: path_str.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| SecretError::Read {
                path: path_str.clone(),
                source,
            })?;
            let entry_path = entry.path();
            // DirEntry::file_type does not traverse symlinks, so symlinked
            // directories and files are skipped outright instead of being
            // descended into via the cycle-prone `Path::is_dir`.
            if entry
                .file_type()
                .map_err(|source| SecretError::Read {
                    path: path_str.clone(),
                    source,
                })?
                .is_symlink()
            {
                continue;
            }

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

                self.scan_directory_recursive(&entry_path, findings, depth + 1)?;
            } else if Self::is_scannable_file(&entry_path) {
                findings.extend(self.scan_file(&entry_path)?);
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

    /// Check if the captured secret value looks like a placeholder.
    ///
    /// Placeholder detection must be conservative: generic words such as
    /// `test` or `123` appear inside real credentials all the time, so they
    /// are only skipped when they equal the whole value. Structural template
    /// markers (`<`, `${`, `{{`) and explicit `your_`/`my_` prefixes remain
    /// substring/prefix checks because they cannot occur in legitimate key
    /// material emitted by any provider covered by [`PATTERNS`].
    fn is_placeholder(value: &str) -> bool {
        const TEMPLATE_MARKERS: [&str; 3] = ["<", "${", "{{"];
        if TEMPLATE_MARKERS.iter().any(|marker| value.contains(marker)) {
            return true;
        }

        let normalized = value.trim().to_lowercase();
        const PLACEHOLDER_VALUES: [&str; 12] = [
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
        ];
        if PLACEHOLDER_VALUES.contains(&normalized.as_str()) {
            return true;
        }

        normalized.starts_with("your_")
            || normalized.starts_with("my_")
            || normalized.starts_with("example_")
            || normalized.starts_with("sample_")
            || normalized.starts_with("placeholder_")
    }

    /// Redact a secret for safe display
    ///
    /// Slices on `char` boundaries only: matched secrets may contain
    /// arbitrary non-whitespace UTF-8 (e.g. via the generic password
    /// pattern), and byte-index slicing would panic on multi-byte chars.
    fn redact(text: &str) -> String {
        if text.len() <= 8 {
            return "*".repeat(text.len());
        }

        let visible_chars = 4;
        let total_chars = text.chars().count();
        let prefix: String = text.chars().take(visible_chars).collect();
        let suffix: String = text
            .chars()
            .skip(total_chars.saturating_sub(visible_chars))
            .collect();
        let hidden_chars = total_chars.saturating_sub(visible_chars * 2);

        format!(
            "{}{}...{}",
            prefix,
            "*".repeat(hidden_chars.min(10)),
            suffix
        )
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
    /// Summarize findings by severity. `#[must_use]`: dropping the summary
    /// defeats the critical/high counts callers rely on.
    #[must_use]
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

    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.critical_count > 0
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    #[test]
    fn test_private_key_detection() {
        let scanner = SecretScanner::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...";
        let findings = scanner.scan_content(content, "key.pem");

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
        let findings = scanner.scan_content(content, "config.py");

        assert!(findings.is_empty(), "Should ignore placeholder values");
    }

    #[test]
    fn test_redaction() {
        let redacted = SecretScanner::redact("secret_token_1234567890abcdef");

        assert!(redacted.contains('*'), "Should contain asterisks");
        assert!(!redacted.is_empty(), "Should produce output");
    }

    #[test]
    fn redaction_never_panics_on_multibyte_secrets() {
        // Regression: byte-index slicing panicked on non-ASCII matches from
        // the generic password pattern ([^\s"]{8,}).
        let scanner = SecretScanner::new();
        let content = "password = \u{43f}\u{430}\u{440}\u{43e}\u{43b}\u{44c}12345678"; // Cyrillic + digits, > 8 chars
        let findings = scanner.scan_content(content, "dotfile");

        assert!(!findings.is_empty(), "multibyte password must be detected");
        let redacted = findings[0].redacted.clone();
        assert!(
            redacted.contains('*'),
            "redacted output must mask the secret"
        );
        // The masked form keeps only a short prefix/suffix window and never
        // the full secret.
        assert!(redacted.contains('*'), "got: {redacted}");
        assert!(redacted.contains("..."), "got: {redacted}");
        assert!(
            !redacted.contains("\u{5bc2}\u{9759}\u{5bc6}"),
            "middle leaked: {redacted}"
        );
    }

    #[test]
    fn scan_directory_survives_symlink_cycles() {
        // Regression: `entry_path.is_dir()` follows symlinks, so a cyclic
        // symlink (`sub/loop -> root`) recursed forever and crashed the
        // scanner with a stack overflow.
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("sub").join("loop")).unwrap();
        std::fs::write(
            root.join("secrets.txt"),
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE...",
        )
        .unwrap();

        let findings = SecretScanner::new().scan_directory(temp.path()).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.secret_type, SecretType::PrivateKey)),
            "real file inside the cycle must still be scanned"
        );
    }

    #[test]
    fn scan_directory_fails_closed_beyond_max_depth() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut deep = temp.path().to_path_buf();
        for level in 0..SecretScanner::MAX_SCAN_DEPTH + 2 {
            deep = deep.join(format!("level-{level}"));
        }
        std::fs::create_dir_all(&deep).unwrap();

        let error = SecretScanner::new()
            .scan_directory(temp.path())
            .expect_err("nesting beyond the scan-depth bound must fail closed");
        assert!(
            matches!(
                error,
                SecretError::DepthExceeded {
                    max: SecretScanner::MAX_SCAN_DEPTH,
                    ..
                }
            ),
            "got: {error}"
        );
    }

    #[test]
    fn scan_directory_skips_symlinked_files_without_error() {
        let temp = tempfile::TempDir::new().unwrap();
        let real = temp.path().join("real.env");
        std::fs::write(&real, "password = super-secret-value-123").unwrap();
        std::os::unix::fs::symlink(&real, temp.path().join("link.env")).unwrap();

        let findings = SecretScanner::new().scan_directory(temp.path()).unwrap();
        assert_eq!(
            findings.len(),
            1,
            "symlink duplicates must not be scanned twice"
        );
        assert_eq!(findings[0].file_path, real.display().to_string());
    }

    #[test]
    fn real_api_key_containing_placeholder_substrings_is_still_reported() {
        // Regression: substring placeholder filtering dropped genuine keys
        // that merely contained words like `test` or `123`.
        let scanner = SecretScanner::new();
        let content = "apikey = 'abcd1234test567890efgh'";
        let findings = scanner.scan_content(content, "config.ini");

        assert!(
            !findings.is_empty(),
            "a real key containing the word 'test' must still be reported"
        );
    }

    #[test]
    fn exact_placeholder_values_are_still_skipped() {
        let scanner = SecretScanner::new();
        let content = "api_key = 'YOUR_API_KEY_HERE'\napi_key = example";
        let findings = scanner.scan_content(content, "config.py");

        assert!(
            findings.is_empty(),
            "exact placeholder values must be skipped"
        );
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

    #[test]
    fn scan_directory_fails_closed_on_unreadable_file() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("secrets.env"), [0xff, 0xfe, 0xfd]).unwrap();
        let error = SecretScanner::new()
            .scan_directory(temp.path())
            .expect_err("invalid UTF-8 in a scannable file must fail closed");
        assert!(matches!(error, SecretError::Read { .. }), "got: {error}");
    }

    #[test]
    fn scan_directory_fails_closed_when_path_is_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let missing = temp.path().join("does-not-exist");
        let error = SecretScanner::new()
            .scan_directory(&missing)
            .expect_err("a missing scan root must fail closed");
        assert!(matches!(error, SecretError::Read { .. }), "got: {error}");
    }

    #[test]
    fn scan_file_finds_private_key() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("key.pem");
        std::fs::write(&path, "-----BEGIN RSA PRIVATE KEY-----\nMIIE...").unwrap();
        let findings = SecretScanner::new().scan_file(&path).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.secret_type, SecretType::PrivateKey))
        );
    }
}
