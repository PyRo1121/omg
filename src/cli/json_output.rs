//! JSON output formatting for CLI commands

use serde::Serialize;

#[derive(Serialize)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
    pub installed: bool,
}

#[derive(Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub url: Option<String>,
    pub size: u64,
    pub download_size: Option<u64>,
    pub install_size: Option<i64>,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
    pub installed: bool,
}

#[derive(Serialize)]
pub struct UpdateInfo {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repo: String,
}

#[derive(Serialize)]
pub struct StatusInfo {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
}

#[derive(Serialize)]
pub struct ListResult {
    pub packages: Vec<String>,
    pub count: usize,
}

pub fn print_json<T: Serialize>(data: &T) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        println!("{json}");
    }
}

pub fn print_json_compact<T: Serialize>(data: &T) {
    if let Ok(json) = serde_json::to_string(data) {
        println!("{json}");
    }
}
