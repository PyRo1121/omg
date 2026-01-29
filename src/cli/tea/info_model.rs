//! Info Model - Elm Architecture implementation for info command
//!
//! Modern, stylish package information display with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::client::DaemonClient;
use crate::package_managers::get_package_manager;
use owo_colors::OwoColorize;
use std::fmt::Write;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, search_detailed};

/// Source of package information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoSource {
    Official,
    Aur,
    Flatpak,
}

impl InfoSource {
    pub fn styled_label(&self) -> String {
        match self {
            Self::Official => "Official Repository".cyan().to_string(),
            Self::Aur => "AUR (Arch User Repository)".yellow().to_string(),
            Self::Flatpak => "Flatpak".blue().to_string(),
        }
    }
}

/// Package information structure
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: InfoSource,
    pub repo: String,
    pub url: Option<String>,
    pub size: Option<u64>,
    pub licenses: Vec<String>,
    pub maintainer: Option<String>,
    pub popularity: Option<f64>,
    pub out_of_date: bool,
}

/// Info state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoState {
    Idle,
    Loading,
    Complete,
    Failed,
    NotFound,
}

/// Info messages
#[derive(Debug, Clone)]
pub enum InfoMsg {
    Fetch(String),
    InfoReceived(PackageInfo),
    NotFound(String),
    Error(String),
}

/// The Info Model
#[derive(Debug, Clone)]
pub struct InfoModel {
    pub package_name: String,
    pub info: Option<PackageInfo>,
    pub state: InfoState,
    pub error: Option<String>,
}

impl Default for InfoModel {
    fn default() -> Self {
        Self {
            package_name: String::new(),
            info: None,
            state: InfoState::Idle,
            error: None,
        }
    }
}

impl InfoModel {
    /// Create new info model
    #[must_use]
    pub fn new(package_name: String) -> Self {
        Self {
            package_name,
            ..Default::default()
        }
    }

    /// Get package name
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    /// Render a key-value pair
    fn render_kv(key: &str, value: &str) -> String {
        format!("  {:<15} {}", key.bold(), value)
    }
}

impl Model for InfoModel {
    type Msg = InfoMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        let pkg = self.package_name.clone();
        Cmd::Exec(Box::new(move || InfoMsg::Fetch(pkg)))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            InfoMsg::Fetch(pkg) => {
                self.package_name.clone_from(&pkg);
                self.state = InfoState::Loading;

                Cmd::Exec(Box::new(move || {
                    let pkg_name = pkg;

                    // Logic mirrors src/cli/packages/info.rs
                    if tokio::runtime::Handle::try_current().is_ok() {
                        std::thread::spawn(move || {
                            let Ok(rt) = tokio::runtime::Runtime::new() else {
                                return InfoMsg::Error(
                                    "Failed to create async runtime".to_string(),
                                );
                            };
                            rt.block_on(async { fetch_info(&pkg_name).await })
                        })
                        .join()
                        .unwrap_or_else(|_| InfoMsg::Error("Thread panicked".to_string()))
                    } else {
                        let Ok(rt) = tokio::runtime::Runtime::new() else {
                            return InfoMsg::Error("Failed to create async runtime".to_string());
                        };
                        rt.block_on(async { fetch_info(&pkg_name).await })
                    }
                }))
            }
            InfoMsg::InfoReceived(info) => {
                self.info = Some(info);
                self.state = InfoState::Complete;
                Cmd::none()
            }
            InfoMsg::NotFound(pkg) => {
                self.package_name = pkg;
                self.state = InfoState::NotFound;
                Cmd::error(format!("Package '{}' not found", self.package_name))
            }
            InfoMsg::Error(err) => {
                self.state = InfoState::Failed;
                self.error = Some(err.clone());
                Cmd::error(format!("Failed to fetch info: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            InfoState::Idle => String::new(),
            InfoState::Loading => format!("⟳ Fetching info for '{}'...", self.package_name)
                .cyan()
                .dimmed()
                .to_string(),
            InfoState::Complete => {
                if let Some(info) = &self.info {
                    let mut output = String::new();

                    // Header
                    let _ = writeln!(
                        output,
                        "\n{} {}\n{} {}\n{}{}\n",
                        "┌─".cyan().bold(),
                        "OMG".cyan().bold(),
                        "│".cyan().bold(),
                        format!("Package Information: {}", info.name).white(),
                        "└".cyan().bold(),
                        "─".repeat(21 + info.name.len()).cyan().bold()
                    );

                    // Details
                    let _ = writeln!(
                        output,
                        "{}",
                        Self::render_kv("Name", &style::package(&info.name))
                    );
                    let _ = writeln!(
                        output,
                        "{}",
                        Self::render_kv("Version", &style::version(&info.version))
                    );
                    let _ = writeln!(
                        output,
                        "{}",
                        Self::render_kv("Description", &info.description)
                    );

                    let source_val = if info.repo == "aur" {
                        info.source.styled_label()
                    } else {
                        format!(
                            "{} ({})",
                            info.source.styled_label(),
                            style::info(&info.repo)
                        )
                    };
                    let _ = writeln!(output, "{}", Self::render_kv("Source", &source_val));

                    if let Some(url) = &info.url {
                        let _ = writeln!(output, "{}", Self::render_kv("URL", &style::url(url)));
                    }

                    if let Some(size) = info.size {
                        let _ = writeln!(output, "{}", Self::render_kv("Size", &style::size(size)));
                    }

                    if !info.licenses.is_empty() {
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_kv("License", &info.licenses.join(", "))
                        );
                    }

                    if let Some(maintainer) = &info.maintainer {
                        let _ = writeln!(output, "{}", Self::render_kv("Maintainer", maintainer));
                    }

                    if let Some(pop) = info.popularity {
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_kv("Popularity", &format!("{pop:.2}%"))
                        );
                    }

                    if info.out_of_date {
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_kv("Status", &style::error("OUT OF DATE"))
                        );
                    }

                    output
                } else {
                    "No info available".to_string()
                }
            }
            InfoState::NotFound => {
                format!(
                    "\n✗ Package '{}' not found in official repositories or AUR.\n",
                    style::package(&self.package_name).red()
                )
            }
            InfoState::Failed => {
                if let Some(err) = &self.error {
                    format!("\n✗ Failed to fetch info: {}\n", err.red())
                } else {
                    "\n✗ Failed to fetch info\n".to_string()
                }
            }
        }
    }
}

/// Helper function to fetch package info
async fn fetch_info(package: &str) -> InfoMsg {
    // 1. Try Daemon first
    if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(info) = client.info(package).await
    {
        return InfoMsg::InfoReceived(PackageInfo {
            name: info.name,
            version: info.version,
            description: info.description,
            source: if info.source == "official" {
                InfoSource::Official
            } else {
                InfoSource::Aur
            },
            repo: info.repo,
            url: if info.url.is_empty() {
                None
            } else {
                Some(info.url)
            },
            size: Some(info.size),
            licenses: info.licenses,
            maintainer: None,
            popularity: None,
            out_of_date: false,
        });
    }

    // 2. Fallback to local package manager
    let pm = get_package_manager();
    if pm.name() == "pacman" {
        #[cfg(feature = "arch")]
        {
            if let Some(info) = crate::package_managers::get_sync_pkg_info(package)
                .ok()
                .flatten()
            {
                return InfoMsg::InfoReceived(PackageInfo {
                    name: info.name,
                    version: info.version.to_string(),
                    description: info.description,
                    source: InfoSource::Official,
                    repo: info.repo,
                    url: info.url,
                    size: info.install_size.map(|s| s as u64),
                    licenses: vec![], // Local info might not have license list handy in this struct
                    maintainer: None,
                    popularity: None,
                    out_of_date: false,
                });
            }
        }
    } else if pm.name() == "apt" {
        #[cfg(feature = "debian")]
        {
            if let Some(info) = crate::package_managers::apt_get_sync_pkg_info(package)
                .ok()
                .flatten()
            {
                return InfoMsg::InfoReceived(PackageInfo {
                    name: info.name,
                    version: info.version.clone(),
                    description: info.description,
                    source: InfoSource::Official,
                    repo: "apt".to_string(),
                    url: info.url,
                    size: info.install_size.map(|s| s as u64),
                    licenses: vec![],
                    maintainer: None,
                    popularity: None,
                    out_of_date: false,
                });
            }
        }
    }

    // 3. AUR Fallback (Arch Only)
    #[cfg(feature = "arch")]
    {
        let aur = AurClient::new();
        if let Ok(Some(info)) = aur.info(package).await {
            // Get more details if possible
            let mut popularity = None;
            let mut maintainer = None;
            let mut out_of_date = false;
            let mut url = None;
            let mut licenses = vec![];

            // Try detailed search to enrich
            if let Ok(detailed) = search_detailed(package).await
                && let Some(d) = detailed.into_iter().find(|p| p.name == info.name)
            {
                popularity = Some(d.popularity);
                if let Some(lic) = d.license {
                    licenses = lic;
                }
                url = d.url;
                maintainer = d.maintainer;
                out_of_date = d.out_of_date.is_some();
            }

            return InfoMsg::InfoReceived(PackageInfo {
                name: info.name,
                version: info.version.to_string(),
                description: info.description,
                source: InfoSource::Aur,
                repo: "aur".to_string(),
                url,
                size: None, // AUR packages don't have binary size until built
                licenses,
                maintainer,
                popularity,
                out_of_date,
            });
        }
    }

    InfoMsg::NotFound(package.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_model_initial_state() {
        let model = InfoModel::new("test".to_string());
        assert!(model.info.is_none());
        assert_eq!(model.state, InfoState::Idle);
        assert_eq!(model.package_name(), "test");
    }

    #[test]
    fn test_info_model_fetch_message() {
        let mut model = InfoModel::new("test".to_string());
        let _cmd = model.update(InfoMsg::Fetch("test-pkg".to_string()));
        assert_eq!(model.state, InfoState::Loading);
        assert_eq!(model.package_name, "test-pkg");
    }

    #[test]
    fn test_info_model_info_received() {
        let mut model = InfoModel::new("test".to_string());
        let test_info = PackageInfo {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: InfoSource::Official,
            repo: "extra".to_string(),
            url: None,
            size: None,
            licenses: vec![],
            maintainer: None,
            popularity: None,
            out_of_date: false,
        };
        let _cmd = model.update(InfoMsg::InfoReceived(test_info));
        assert_eq!(model.state, InfoState::Complete);
        assert_eq!(model.info.as_ref().unwrap().name, "test-pkg");
    }

    #[test]
    fn test_info_view_with_data() {
        let mut model = InfoModel::new("test".to_string());
        let _ = model.update(InfoMsg::InfoReceived(PackageInfo {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: InfoSource::Official,
            repo: "extra".to_string(),
            url: None,
            size: None,
            licenses: vec![],
            maintainer: None,
            popularity: None,
            out_of_date: false,
        }));
        let view = model.view();
        assert!(view.contains("test-pkg"));
        assert!(view.contains("Official Repository"));
    }
}
