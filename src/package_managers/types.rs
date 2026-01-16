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
