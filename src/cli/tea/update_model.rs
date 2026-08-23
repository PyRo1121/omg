//! Update Command Model (Elm Architecture)
//!
//! Modern, stylish package update interface with Bubble Tea-inspired UX.

use crate::cli::components::Components;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use semver::Version;

/// Update state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Checking,
    ShowingUpdates,
    Confirming,
    Complete,
    Failed,
}

/// Single update package info
#[derive(Debug, Clone)]
pub struct UpdatePackage {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

/// Type of update (for styling and JSON output)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum UpdateType {
    Major,
    Minor,
    Patch,
    Unknown,
}

impl std::fmt::Display for UpdateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Patch => write!(f, "patch"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
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
}

/// Update messages
#[derive(Debug, Clone)]
pub enum UpdateMsg {
    Check,
    UpdatesFound(Vec<UpdatePackage>),
    NoUpdates,
    Execute,
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
}

impl Default for UpdateModel {
    fn default() -> Self {
        Self {
            state: UpdateState::Checking,
            updates: Vec::new(),
            error: None,
            check_only: false,
            yes: false,
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
        Cmd::exec(|| {
            let updates_result = crate::cli::tea::async_bridge::run_blocking_future(async {
                let pm = get_package_manager()?;
                let service = PackageService::new(pm)?;
                service.list_updates().await
            })
            .and_then(std::convert::identity);

            match updates_result {
                Ok(updates) => {
                    if updates.is_empty() {
                        UpdateMsg::NoUpdates
                    } else {
                        let packages: Vec<UpdatePackage> = updates
                            .into_iter()
                            .map(|u| UpdatePackage {
                                name: u.name,
                                old_version: u.old_version,
                                new_version: u.new_version,
                            })
                            .collect();
                        UpdateMsg::UpdatesFound(packages)
                    }
                }
                Err(err) => UpdateMsg::Error(format!("Failed to check updates: {err}")),
            }
        })
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            UpdateMsg::Check => {
                self.state = UpdateState::Checking;
                // The actual check runs in `init`; this arm only reflects the
                // state transition. It must NOT re-emit `Check`, which would
                // loop forever in `Program`.
                Components::loading("Checking for updates...")
            }
            UpdateMsg::UpdatesFound(updates) => {
                self.updates = updates;
                self.state = UpdateState::ShowingUpdates;

                let summary_data: Vec<_> = self
                    .updates
                    .iter()
                    .map(|u| {
                        (
                            u.name.as_str(),
                            u.old_version.as_str(),
                            u.new_version.as_str(),
                        )
                    })
                    .collect();

                let summary_cmd = Components::update_summary(summary_data);

                if self.check_only {
                    Cmd::batch([
                        summary_cmd,
                        Cmd::spacer(),
                        Cmd::info("Run 'omg update' to install"),
                    ])
                } else if self.yes {
                    Cmd::batch([summary_cmd, Cmd::exec(|| UpdateMsg::Execute)])
                } else {
                    self.state = UpdateState::Confirming;
                    Cmd::batch([summary_cmd, Components::confirm("System Upgrade", "Enter")])
                }
            }
            UpdateMsg::NoUpdates => {
                self.state = UpdateState::Complete;
                Components::up_to_date()
            }
            UpdateMsg::Execute => {
                // Honest failure: no upgrade executor is wired to this model.
                // Claiming progress here would fabricate an upgrade that never
                // happens; callers must drive the package manager directly.
                self.state = UpdateState::Failed;
                self.error = Some("upgrade execution is not implemented in this model".to_string());
                Cmd::error("System upgrade execution is not implemented; nothing was installed")
            }
            UpdateMsg::Complete => {
                self.state = UpdateState::Complete;
                Components::complete("System upgrade complete!")
            }
            UpdateMsg::Error(err) => {
                self.state = UpdateState::Failed;
                let message = format!("Update failed: {err}");
                self.error = Some(err);
                Cmd::error(message)
            }
        }
    }

    fn view(&self) -> String {
        String::new()
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
