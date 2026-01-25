//! Update Command Model (Elm Architecture)
//!
//! Modern, stylish package update interface with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use semver::Version;
use std::fmt::Write;
use std::sync::Arc;

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

    /// Render a single update entry with beautiful styling
    fn render_update(upd: &UpdatePackage) -> String {
        format!(
            "  {:>8} {} {} {} → {}",
            upd.update_type.styled_label(),
            style::package(&upd.name),
            style::dim(&format!("({})", upd.repo)),
            style::dim(&upd.old_version),
            style::version(&upd.new_version)
        )
    }

    /// Render progress bar for downloads
    pub fn render_progress_bar(&self, width: usize) -> String {
        let clamped = self.download_percent.min(100);
        let filled = (self.download_percent * width / 100).min(width);
        let empty = width - filled;

        let filled_bar = "█".repeat(filled);
        let empty_bar = "░".repeat(empty);

        format!(
            "{}{}{}",
            filled_bar.green().bold(),
            empty_bar.dimmed(),
            format!(" {clamped}%").dimmed()
        )
    }

    /// Render header with beautiful box drawing
    fn render_header(title: &str, subtitle: &str) -> String {
        format!(
            "\n{} {}\n{} {}\n{}{}\n",
            "┌─".cyan().bold(),
            title.cyan().bold(),
            "│".cyan().bold(),
            subtitle.white(),
            "└".cyan().bold(),
            "─".repeat(subtitle.len()).cyan().bold()
        )
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
                    let pm = Arc::from(get_package_manager());
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
                    let pm = Arc::from(get_package_manager());
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
                Cmd::Exec(Box::new(|| {
                    // Async check will be handled in init
                    UpdateMsg::Check
                }))
            }
            UpdateMsg::UpdatesFound(updates) => {
                self.updates = updates;
                self.state = UpdateState::ShowingUpdates;

                if self.check_only {
                    // Show check-only message
                    Cmd::batch([
                        Cmd::PrintLn(String::new()),
                        Cmd::Info("Run 'omg update' to install".to_string()),
                    ])
                } else if self.yes {
                    // Auto-confirm and execute
                    Cmd::Exec(Box::new(|| UpdateMsg::Execute))
                } else {
                    // Show confirmation prompt
                    self.state = UpdateState::Confirming;
                    Cmd::PrintLn(String::new())
                }
            }
            UpdateMsg::NoUpdates => {
                self.state = UpdateState::Complete;
                Cmd::batch([
                    Cmd::PrintLn(String::new()),
                    Cmd::Success("System is up to date!".to_string()),
                    Cmd::PrintLn(String::new()),
                ])
            }
            UpdateMsg::Confirm(should_proceed) => {
                if should_proceed {
                    Cmd::Exec(Box::new(|| UpdateMsg::Execute))
                } else {
                    self.state = UpdateState::Complete;
                    Cmd::batch([
                        Cmd::PrintLn(String::new()),
                        Cmd::Warning("Upgrade cancelled.".to_string()),
                        Cmd::PrintLn(String::new()),
                    ])
                }
            }
            UpdateMsg::Execute => {
                self.state = UpdateState::Downloading;
                Cmd::batch([
                    Cmd::PrintLn(String::new()),
                    Cmd::Info("→ Executing system upgrade...".to_string()),
                ])
            }
            UpdateMsg::DownloadProgress { percent } => {
                self.download_percent = percent.clamp(0, 100);
                if self.download_percent >= 100 {
                    self.state = UpdateState::Installing;
                }
                Cmd::none()
            }
            UpdateMsg::InstallProgress { package } => {
                self.current_installing = Some(package);
                Cmd::none()
            }
            UpdateMsg::Complete => {
                self.state = UpdateState::Complete;
                Cmd::Success("System upgrade complete!".to_string())
            }
            UpdateMsg::Error(err) => {
                self.state = UpdateState::Failed;
                self.error = Some(err.clone());
                Cmd::Error(format!("Update failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            UpdateState::Idle => String::new(),
            UpdateState::Checking => "⟳ Checking for updates...".cyan().dimmed().to_string(),
            UpdateState::ShowingUpdates => {
                let mut output = String::new();

                // Beautiful header
                output.push_str(&Self::render_header(
                    "OMG",
                    &format!("Found {} update(s)", self.updates.len()),
                ));

                // Update list
                for upd in &self.updates {
                    output.push_str(&Self::render_update(upd));
                    output.push('\n');
                }

                // Confirmation prompt
                if !self.check_only && !self.yes {
                    let _ = write!(
                        output,
                        "\n{} Proceed with system upgrade? · ",
                        "✓".green().bold()
                    );
                }

                output
            }
            UpdateState::Confirming => {
                // Already handled in ShowingUpdates
                String::new()
            }
            UpdateState::Downloading => {
                format!(
                    "{}\n{} {}",
                    style::arrow("→→"),
                    "Downloading packages...".cyan(),
                    self.render_progress_bar(40)
                )
            }
            UpdateState::Installing => {
                if let Some(pkg) = &self.current_installing {
                    format!(
                        "{} Installing {}...",
                        style::arrow("→→"),
                        style::package(pkg)
                    )
                } else {
                    format!("{} Installing updates...", style::arrow("→→"))
                }
            }
            UpdateState::Complete => {
                if self.updates.is_empty() {
                    format!("\n✓ {}\n", "System is up to date!".green().bold())
                } else {
                    format!("\n✓ {}\n", "System upgrade complete!".green().bold())
                }
            }
            UpdateState::Failed => {
                if let Some(err) = &self.error {
                    format!("\n✗ Update failed: {}\n", err.red())
                } else {
                    "\n✗ Update failed\n".to_string()
                }
            }
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

    #[test]
    fn test_render_progress_bar() {
        let model = UpdateModel {
            download_percent: 50,
            ..Default::default()
        };
        let bar = model.render_progress_bar(10);
        // Should have 5 filled and 5 empty at 50%
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }

    #[test]
    fn test_progress_bar_clamping() {
        let model = UpdateModel {
            download_percent: 150, // Over 100
            ..Default::default()
        };
        let bar = model.render_progress_bar(10);
        // Should clamp to 100%
        assert!(bar.contains("100%"));
    }
}
