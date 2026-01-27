//! Test fixtures for OMG testing
//!
//! Pre-defined test data and scenarios for consistent testing.

// Re-export library fixtures
pub use omg_lib::core::testing::fixtures::*;

use crate::common::mocks::MockPackage;

/// Extension trait to convert `PackageFixture` to `MockPackage` for testing
pub trait PackageFixtureExt {
    /// Convert a `PackageFixture` into a `MockPackage` for use in tests
    fn to_mock_package(&self) -> MockPackage;
}

impl PackageFixtureExt for PackageFixture {
    fn to_mock_package(&self) -> MockPackage {
        // Build the Package first to get defaults applied
        let pkg = self.clone().build();

        MockPackage {
            name: pkg.name,
            version: pkg.version.to_string(),
            description: pkg.description,
            repo: "test".to_string(),
            dependencies: vec![],
            installed_size: 100,
        }
    }
}

/// Common package names for testing across distros
pub mod packages {
    /// Packages that exist on all supported distros
    pub const UNIVERSAL: &[&str] = &[
        "git", "curl", "wget", "vim", "nano", "make", "gcc", "python3",
    ];

    /// Arch-specific packages
    pub const ARCH_ONLY: &[&str] = &[
        "pacman",
        "yay",
        "paru",
        "base-devel",
        "linux",
        "linux-headers",
    ];

    /// Debian/Ubuntu-specific packages
    pub const DEBIAN_ONLY: &[&str] = &["apt", "dpkg", "build-essential", "linux-image-generic"];

    /// Packages that definitely don't exist
    pub const NONEXISTENT: &[&str] = &[
        "this-package-does-not-exist-12345",
        "fake-package-xyz-99999",
        "nonexistent-lib-abc",
    ];

    /// Popular packages for search testing
    pub const POPULAR: &[&str] = &["firefox", "chromium", "vlc", "gimp", "libreoffice"];

    /// Development tools
    pub const DEV_TOOLS: &[&str] = &["nodejs", "python", "ruby", "golang", "rustup"];
}

/// Runtime version fixtures
pub mod runtimes {
    pub const NODE_VERSIONS: &[&str] = &["18.0.0", "20.10.0", "21.0.0", "lts", "latest"];
    pub const PYTHON_VERSIONS: &[&str] = &["3.10.0", "3.11.0", "3.12.0"];
    pub const GO_VERSIONS: &[&str] = &["1.20", "1.21", "1.22"];
    pub const RUST_CHANNELS: &[&str] = &["stable", "beta", "nightly"];
    pub const RUBY_VERSIONS: &[&str] = &["3.1.0", "3.2.0", "3.3.0"];
    pub const JAVA_VERSIONS: &[&str] = &["17", "21", "22"];
    pub const BUN_VERSIONS: &[&str] = &["1.0.0", "1.1.0"];
}

/// Version file content fixtures
pub mod version_files {
    pub const NVMRC_SIMPLE: &str = "20.10.0";
    pub const NVMRC_LTS: &str = "lts/*";
    pub const NVMRC_WITH_V: &str = "v20.10.0";
    pub const NVMRC_WITH_COMMENT: &str = "# Node version\n20.10.0";
    pub const NVMRC_WITH_WHITESPACE: &str = "  20.10.0  \n";

    pub const PYTHON_VERSION_SIMPLE: &str = "3.11.0";
    pub const PYTHON_VERSION_MAJOR_MINOR: &str = "3.11";

    pub const TOOL_VERSIONS_MULTI: &str = r"nodejs 20.10.0
python 3.11.0
ruby 3.2.0
golang 1.21";

    pub const MISE_TOML_SIMPLE: &str = r#"[tools]
node = "20.10.0"
python = "3.11.0"
"#;

    pub const MISE_TOML_COMPLEX: &str = r#"[tools]
node = "20.10.0"
python = "3.11.0"
ruby = "3.2.0"
go = "1.21"

[env]
NODE_ENV = "development"

[tasks.build]
run = "npm run build"
"#;

    pub const RUST_TOOLCHAIN_SIMPLE: &str = "[toolchain]\nchannel = \"stable\"";
    pub const RUST_TOOLCHAIN_NIGHTLY: &str = "[toolchain]\nchannel = \"nightly\"";
    pub const RUST_TOOLCHAIN_SPECIFIC: &str = r#"[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
"#;

    pub const GO_MOD_SIMPLE: &str = "module test\n\ngo 1.21";
    pub const GO_MOD_WITH_DEPS: &str = r"module test

go 1.21

require (
    github.com/gin-gonic/gin v1.9.0
)
";

    pub const PACKAGE_JSON_WITH_ENGINES: &str = r#"{
  "name": "test-project",
  "version": "1.0.0",
  "engines": {
    "node": ">=18.0.0"
  }
}"#;

    pub const PACKAGE_JSON_WITH_VOLTA: &str = r#"{
  "name": "test-project",
  "version": "1.0.0",
  "volta": {
    "node": "20.10.0"
  }
}"#;
}

/// Security policy fixtures
pub mod policies {
    pub const STRICT_POLICY: &str = r#"
allow_aur = false
require_pgp = true
minimum_grade = "Verified"
banned_packages = ["telnet", "ftp"]
allowed_licenses = ["MIT", "Apache-2.0", "BSD-3-Clause", "GPL-3.0"]
"#;

    pub const PERMISSIVE_POLICY: &str = r#"
allow_aur = true
require_pgp = false
minimum_grade = "Unverified"
banned_packages = []
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

/// Lock file fixtures
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

    pub const INVALID_LOCK_TOML: &str = "this is not valid toml {{{{";

    pub const WRONG_SCHEMA_LOCK: &str = r#"[wrong_section]
key = "value"
"#;

    pub const EMPTY_LOCK: &str = r#"[environment]
hash = ""
captured_at = ""
"#;
}

/// Team configuration fixtures
pub mod team {
    pub const TEAM_CONFIG: &str = r#"[team]
id = "acme/frontend"
name = "ACME Frontend Team"
remote = "https://github.com/acme/frontend-env"

[members]
alice = { role = "admin", email = "alice@acme.com" }
bob = { role = "developer", email = "bob@acme.com" }
"#;

    pub const GOLDEN_PATH_REACT: &str = r#"[template]
name = "react-app"
description = "React application template"

[runtimes]
node = "20.10.0"

[packages]
required = ["git", "curl"]

[npm_packages]
global = ["typescript", "eslint", "prettier"]
"#;
}

/// CI/CD fixtures
pub mod ci {
    pub const GITHUB_ACTIONS: &str = r"name: CI

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install OMG
        run: curl -fsSL https://omg.dev/install.sh | sh
";

    pub const GITLAB_CI: &str = r"stages:
  - build
  - test

build:
  stage: build
  script:
    - omg env sync omg.lock
    - omg run build
";
}

/// Input validation test cases
pub mod validation {
    /// Potentially dangerous inputs for security testing
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

    /// Unicode edge cases (excluding null bytes which cause Command errors)
    pub const UNICODE_INPUTS: &[&str] = &[
        "unicode-package",
        "пакет",
        "🔥📦",
        "test_null",
        "test\u{FEFF}bom",
        "Ñoño",
    ];

    /// Long inputs for buffer testing
    pub fn very_long_input(len: usize) -> String {
        "a".repeat(len)
    }

    /// Empty and whitespace inputs
    pub const EMPTY_INPUTS: &[&str] = &["", " ", "\t", "\n", "   \t\n   "];
}

/// Performance test parameters
/// Note: First run may be slower due to cold start, these are generous limits
pub mod perf {
    use std::time::Duration;

    pub const HELP_MAX_MS: u64 = 100;
    pub const STATUS_MAX_MS: u64 = 2000;
    pub const LIST_MAX_MS: u64 = 2000;
    pub const WHICH_MAX_MS: u64 = 2000;
    pub const SEARCH_MAX_MS: u64 = 3000;
    pub const COMPLETIONS_MAX_MS: u64 = 2000;
    pub const ENV_CAPTURE_MAX_MS: u64 = 2000;
    pub const VERSION_SWITCH_MAX_MS: u64 = 500;

    pub fn max_duration(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }
}

/// Test scenarios combining multiple fixtures
pub mod scenarios {
    /// Full-stack Node.js project
    pub struct NodeProject;
    impl NodeProject {
        pub const NVMRC: &'static str = "20.10.0";
        pub const PACKAGE_JSON: &'static str = r#"{
  "name": "test-node-project",
  "version": "1.0.0",
  "engines": { "node": ">=18.0.0" },
  "scripts": { "build": "echo build", "test": "echo test" }
}"#;
    }

    /// Full-stack Python project
    pub struct PythonProject;
    impl PythonProject {
        pub const PYTHON_VERSION: &'static str = "3.11.0";
        pub const REQUIREMENTS: &'static str = "requests==2.31.0\npytest==7.4.0";
        pub const PYPROJECT: &'static str = r#"[project]
name = "test"
version = "0.1.0"
requires-python = ">=3.11"
"#;
    }

    /// Monorepo with multiple runtimes
    pub struct Monorepo;
    impl Monorepo {
        pub const TOOL_VERSIONS: &'static str = r"nodejs 20.10.0
python 3.11.0
ruby 3.2.0
golang 1.21
";
    }
}
