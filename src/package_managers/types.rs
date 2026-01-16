//! Shared package manager types

#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub install_size: i64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct SyncPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub download_size: i64,
    pub installed: bool,
}

#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: String,
    pub size: u64,
    pub download_size: u64,
    pub repo: String,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
}
