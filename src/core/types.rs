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
    pub fn all() -> &'static [Runtime] {
        &[
            Runtime::Node,
            Runtime::Bun,
            Runtime::Python,
            Runtime::Go,
            Runtime::Rust,
            Runtime::Ruby,
            Runtime::Java,
        ]
    }

    /// Get the version file name for this runtime
    pub fn version_file(&self) -> &'static str {
        match self {
            Runtime::Node => ".nvmrc",
            Runtime::Bun => ".bun-version",
            Runtime::Python => ".python-version",
            Runtime::Go => ".go-version",
            Runtime::Rust => ".rust-version",
            Runtime::Ruby => ".ruby-version",
            Runtime::Java => ".java-version",
        }
    }

    /// Get the binary names managed by this runtime
    pub fn binaries(&self) -> &'static [&'static str] {
        match self {
            Runtime::Node => &["node", "npm", "npx"],
            Runtime::Bun => &["bun", "bunx"],
            Runtime::Python => &["python", "python3", "pip", "pip3"],
            Runtime::Go => &["go", "gofmt"],
            Runtime::Rust => &["rustc", "cargo", "rustup"],
            Runtime::Ruby => &["ruby", "gem", "irb", "bundle"],
            Runtime::Java => &["java", "javac", "jar"],
        }
    }
}

impl std::fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runtime::Node => write!(f, "node"),
            Runtime::Bun => write!(f, "bun"),
            Runtime::Python => write!(f, "python"),
            Runtime::Go => write!(f, "go"),
            Runtime::Rust => write!(f, "rust"),
            Runtime::Ruby => write!(f, "ruby"),
            Runtime::Java => write!(f, "java"),
        }
    }
}

impl std::str::FromStr for Runtime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "node" | "nodejs" => Ok(Runtime::Node),
            "bun" => Ok(Runtime::Bun),
            "python" | "python3" => Ok(Runtime::Python),
            "go" | "golang" => Ok(Runtime::Go),
            "rust" | "rustc" => Ok(Runtime::Rust),
            "ruby" => Ok(Runtime::Ruby),
            "java" => Ok(Runtime::Java),
            _ => Err(format!("Unknown runtime: {}", s)),
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
