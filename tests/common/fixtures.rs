//! Shared integration-test fixtures.

pub use omg_lib::core::testing::fixtures::*;

use crate::common::mocks::MockPackage;

/// Convert a package fixture into the mock representation used by logic tests.
pub trait PackageFixtureExt {
    fn to_mock_package(self) -> MockPackage;
}

impl PackageFixtureExt for PackageFixture {
    fn to_mock_package(self) -> MockPackage {
        let package = self.build();

        #[cfg(feature = "arch")]
        let version = package.version.to_string();
        #[cfg(not(feature = "arch"))]
        let version = package.version;

        MockPackage {
            name: package.name,
            version,
            description: package.description,
        }
    }
}

pub mod packages {
    /// Package names guaranteed not to exist in supported repositories.
    pub const NONEXISTENT: &[&str] = &[
        "this-package-does-not-exist-12345",
        "fake-package-xyz-99999",
        "nonexistent-lib-abc",
    ];
}

pub mod policies {
    pub const STRICT_POLICY: &str = r#"
allow_aur = false
require_pgp = true
minimum_grade = "Verified"
banned_packages = ["telnet", "ftp"]
allowed_licenses = ["MIT", "Apache-2.0", "BSD-3-Clause", "GPL-3.0"]
"#;

    pub const ENTERPRISE_POLICY: &str = r#"
allow_aur = false
require_pgp = true
minimum_grade = "Verified"
banned_packages = ["telnet", "ftp", "rsh", "rlogin"]
allowed_licenses = ["MIT", "Apache-2.0", "BSD-3-Clause"]
require_sbom = true
require_slsa = true
max_cve_age_days = 30
"#;
}

pub mod locks {
    pub const VALID_LOCK: &str = r#"[environment]
hash = "abc123def456"
captured_at = "2025-01-19T12:00:00Z"

[runtimes]
node = "20.10.0"
python = "3.11.0"

[packages]
git = "2.43.0"
curl = "8.5.0"
"#;
}

pub mod validation {
    /// Potentially dangerous inputs for security testing.
    pub const INJECTION_ATTEMPTS: &[&str] = &[
        "; rm -rf /",
        "$(cat /etc/passwd)",
        "`whoami`",
        "| cat /etc/shadow",
        "&& echo pwned",
        "'; DROP TABLE packages;--",
        "<script>alert('xss')</script>",
        "../../../etc/passwd",
        "/etc/passwd%00.txt",
    ];

    /// Unicode edge cases (excluding null bytes, which `Command` rejects).
    pub const UNICODE_INPUTS: &[&str] = &[
        "unicode-package",
        "пакет",
        "🔥📦",
        "test_null",
        "test\u{FEFF}bom",
        "Ñoño",
    ];

    pub fn very_long_input(len: usize) -> String {
        "a".repeat(len)
    }

    pub const EMPTY_INPUTS: &[&str] = &["", " ", "\t", "\n", "   \t\n   "];
}

pub mod error_conditions {
    use crate::common::TestProject;
    use std::fs;

    /// Create a project with a corrupted database file.
    pub fn corrupted_database() -> TestProject {
        let project = TestProject::new();
        let db_path = project.data_dir.path().join("corrupted.db");
        fs::write(db_path, "corrupted database data {{{{\n").unwrap();
        project
    }

    /// Create a project with an invalid lock file.
    pub fn invalid_lock_file() -> TestProject {
        let project = TestProject::new();
        project.create_file("omg.lock", "invalid toml {{{{");
        project
    }
}
