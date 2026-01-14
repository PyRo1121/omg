//! Common types used throughout OMG

use serde::{Deserialize, Serialize};

/// Supported runtimes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Node,
    Bun,
    Python,
    Go,
    Rust,
    Ruby,
    Java,
}

impl Runtime {
    /// Get all supported runtimes
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Node,
            Self::Bun,
            Self::Python,
            Self::Go,
            Self::Rust,
            Self::Ruby,
            Self::Java,
        ]
    }

    /// Get the version file name for this runtime
    #[must_use]
    pub const fn version_file(&self) -> &'static str {
        match self {
            Self::Node => ".nvmrc",
            Self::Bun => ".bun-version",
            Self::Python => ".python-version",
            Self::Go => ".go-version",
            Self::Rust => ".rust-version",
            Self::Ruby => ".ruby-version",
            Self::Java => ".java-version",
        }
    }

    /// Get the binary names managed by this runtime
    #[must_use]
    pub const fn binaries(&self) -> &'static [&'static str] {
        match self {
            Self::Node => &["node", "npm", "npx"],
            Self::Bun => &["bun", "bunx"],
            Self::Python => &["python", "python3", "pip", "pip3"],
            Self::Go => &["go", "gofmt"],
            Self::Rust => &["rustc", "cargo", "rustup"],
            Self::Ruby => &["ruby", "gem", "irb", "bundle"],
            Self::Java => &["java", "javac", "jar"],
        }
    }
}

impl std::fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Bun => write!(f, "bun"),
            Self::Python => write!(f, "python"),
            Self::Go => write!(f, "go"),
            Self::Rust => write!(f, "rust"),
            Self::Ruby => write!(f, "ruby"),
            Self::Java => write!(f, "java"),
        }
    }
}

impl std::str::FromStr for Runtime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "node" | "nodejs" => Ok(Self::Node),
            "bun" => Ok(Self::Bun),
            "python" | "python3" => Ok(Self::Python),
            "go" | "golang" => Ok(Self::Go),
            "rust" | "rustc" => Ok(Self::Rust),
            "ruby" => Ok(Self::Ruby),
            "java" => Ok(Self::Java),
            _ => Err(format!("Unknown runtime: {s}")),
        }
    }
}

/// Package source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    /// Official Arch Linux repositories
    Official,
    /// Arch User Repository
    Aur,
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: PackageSource,
    pub installed: bool,
}

/// Runtime version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub runtime: Runtime,
    pub version: String,
    pub installed: bool,
    pub active: bool,
    pub path: Option<std::path::PathBuf>,
}
