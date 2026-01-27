//! Update Command Model (Elm Architecture)
//!
//! Modern, stylish package update interface with Bubble Tea-inspired UX.

use crate::cli::components::Components;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use semver::Version;

/// Update state machine
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    ShowingUpdates,
    Confirming,
    Downloading,
    Installing,
    Complete,
    Failed,
}

/// Single update package info
#[derive(Debug, Clone)]
pub struct UpdatePackage {
    pub name: String,
    pub repo: String,
    pub old_version: String,
    pub new_version: String,
    pub update_type: UpdateType,
}

/// Type of update (for styling)
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateType {
    Major,
    Minor,
    Patch,
    Unknown,
}

impl UpdateType {
    /// Parse version strings and determine update type
    pub fn from_versions(old_ver: &str, new_ver: &str) -> Self {
        let old_str = old_ver.trim_start_matches(|c: char| !c.is_numeric());
        let new_str = new_ver.trim_start_matches(|c: char| !c.is_numeric());

        match (Version::parse(old_str), Version::parse(new_str)) {
            (Ok(old), Ok(new)) => {
                if new.major > old.major {
                    Self::Major
                } else if new.minor > old.minor {
                    Self::Minor
                } else {
                    Self::Patch
                }
            }
            _ => Self::Unknown,
        }
    }

    /// Get styled label for this update type
    pub fn styled_label(&self) -> String {
        match self {
            Self::Major => "MAJOR".red().bold().to_string(),
            Self::Minor => "minor".yellow().bold().to_string(),
            Self::Patch => "patch".green().bold().to_string(),
            Self::Unknown => "update".dimmed().to_string(),
        }
    }
}

use owo_colors::OwoColorize;

/// Update messages
#[derive(Debug, Clone)]
pub enum UpdateMsg {
    Check,
    UpdatesFound(Vec<UpdatePackage>),
    NoUpdates,
    Confirm(bool),
    Execute,
    DownloadProgress { percent: usize },
    InstallProgress { package: String },
    Complete,
    Error(String),
}

/// Update model state
#[derive(Debug, Clone)]
pub struct UpdateModel {
    pub state: UpdateState,
    pub updates: Vec<UpdatePackage>,
    pub error: Option<String>,
    pub check_only: bool,
    pub yes: bool,
    pub download_percent: usize,
    pub current_installing: Option<String>,
}

impl Default for UpdateModel {
    fn default() -> Self {
        Self {
            state: UpdateState::Checking,
            updates: Vec::new(),
            error: None,
            check_only: false,
            yes: false,
            download_percent: 0,
            current_installing: None,
        }
    }
}

impl UpdateModel {
    /// Create new update model
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set check-only mode (don't install)
    #[must_use]
    pub const fn with_check_only(mut self, check_only: bool) -> Self {
        self.check_only = check_only;
        self
    }

    /// Set auto-confirm mode
    #[must_use]
    pub const fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
    }
}

impl Model for UpdateModel {
    type Msg = UpdateMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::Exec(Box::new(|| {
            // Check if a runtime already exists (e.g., in tests)
            let updates_result: Result<
                Vec<crate::package_managers::types::UpdateInfo>,
                anyhow::Error,
            > = if tokio::runtime::Handle::try_current().is_ok() {
                // Runtime exists: use a thread to avoid nesting
                std::thread::spawn(|| {
                    let pm = get_package_manager();
                    let service = PackageService::new(pm);
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async { service.list_updates().await })
                })
                .join()
                .unwrap()
            } else {
                // No runtime: create one (production case)
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let pm = get_package_manager();
                    let service = PackageService::new(pm);
                    service.list_updates().await
                })
            };

            match updates_result {
                Ok(updates_list) => {
                    let updates: Vec<crate::package_managers::types::UpdateInfo> = updates_list;
                    if updates.is_empty() {
                        UpdateMsg::NoUpdates
                    } else {
                        let packages: Vec<UpdatePackage> = updates
                            .into_iter()
                            .map(|u| {
                                let old = u.old_version.clone();
                                let new = u.new_version.clone();
                                UpdatePackage {
                                    name: u.name,
                                    repo: u.repo,
                                    old_version: old.clone(),
                                    new_version: new.clone(),
                                    update_type: UpdateType::from_versions(&old, &new),
                                }
                            })
                            .collect();
                        UpdateMsg::UpdatesFound(packages)
                    }
                }
                Err(err) => UpdateMsg::Error(format!("Failed to check updates: {err}")),
            }
        }))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            UpdateMsg::Check => {
                self.state = UpdateState::Checking;
                // Emit a spinner command
                Cmd::batch([
                    Components::loading("Checking for updates..."),
                    Cmd::Exec(Box::new(|| {
                        // The actual check is running in the background via init
                        UpdateMsg::Check // Placeholder, logic is in init
                    })),
                ])
            }
            UpdateMsg::UpdatesFound(updates) => {
                self.updates = updates;
                self.state = UpdateState::ShowingUpdates;

                let summary_data: Vec<(String, String, String)> = self
                    .updates
                    .iter()
                    .map(|u| (u.name.clone(), u.old_version.clone(), u.new_version.clone()))
                    .collect();

                let summary_cmd = Components::update_summary(summary_data);

                if self.check_only {
                    Cmd::batch([
                        summary_cmd,
                        Cmd::spacer(),
                        Cmd::info("Run 'omg update' to install"),
                    ])
                } else if self.yes {
                    Cmd::batch([summary_cmd, Cmd::Exec(Box::new(|| UpdateMsg::Execute))])
                } else {
                    self.state = UpdateState::Confirming;
                    Cmd::batch([summary_cmd, Components::confirm("System Upgrade", "Enter")])
                }
            }
            UpdateMsg::NoUpdates => {
                self.state = UpdateState::Complete;
                Components::up_to_date()
            }
            UpdateMsg::Confirm(should_proceed) => {
                if should_proceed {
                    Cmd::Exec(Box::new(|| UpdateMsg::Execute))
                } else {
                    self.state = UpdateState::Complete;
                    Cmd::warning("Upgrade cancelled.")
                }
            }
            UpdateMsg::Execute => {
                self.state = UpdateState::Downloading;
                Cmd::info("Executing system upgrade...")
            }
            UpdateMsg::DownloadProgress { percent } => {
                self.download_percent = percent.clamp(0, 100);
                if self.download_percent >= 100 {
                    self.state = UpdateState::Installing;
                }
                // Fallback to info message as progress component is missing
                Cmd::info(format!("Downloading... {}%", self.download_percent))
            }
            UpdateMsg::InstallProgress { package } => {
                self.current_installing = Some(package.clone());
                // Fallback to info message
                Cmd::info(format!("Installing {package}..."))
            }
            UpdateMsg::Complete => {
                self.state = UpdateState::Complete;
                Components::complete("System upgrade complete!")
            }
            UpdateMsg::Error(err) => {
                self.state = UpdateState::Failed;
                self.error = Some(err.clone());
                Cmd::error(format!("Update failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        // View is now minimal as Components handle the output via side-effects (Cmds)
        // We only use view for static prompts if not handled by Cmds
        match self.state {
            UpdateState::Confirming => {
                // The confirmation prompt is printed via Cmd::batch, but we might need
                // to render the prompt line here if we want it to persist at the bottom
                // "Proceed? [Y/n]" is usually handled by the input loop (wrappers.rs)
                // which prints the prompt.
                String::new()
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_type_detection() {
        assert_eq!(
            UpdateType::from_versions("1.0.0", "2.0.0"),
            UpdateType::Major
        );
        assert_eq!(
            UpdateType::from_versions("1.0.0", "1.1.0"),
            UpdateType::Minor
        );
        assert_eq!(
            UpdateType::from_versions("1.0.0", "1.0.1"),
            UpdateType::Patch
        );
    }

    #[test]
    fn test_update_type_pacman_version() {
        // Pacman versions like "1.15.6-1" should parse correctly
        assert_eq!(
            UpdateType::from_versions("1.15.6-1", "1.15.8-1"),
            UpdateType::Patch
        );
        assert_eq!(
            UpdateType::from_versions("1.20.0-1", "1.21.0-1"),
            UpdateType::Minor
        );
    }
}
