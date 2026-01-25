//! Example implementations of the Elm Architecture for common CLI operations

use super::{Cmd, Model, Msg, Program, Renderer};

/// Install Model - tracks package installation progress
#[derive(Debug, Clone)]
pub enum InstallMsg {
    Start(String),
    DownloadProgress { percent: usize },
    InstallComplete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct InstallModel {
    package: String,
    status: InstallStatus,
    error: Option<String>,
    download_percent: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum InstallStatus {
    Idle,
    Downloading,
    Installing,
    Complete,
    Failed,
}

impl InstallModel {
    pub fn new(package: String) -> Self {
        Self {
            package,
            status: InstallStatus::Idle,
            error: None,
            download_percent: 0,
        }
    }
}

impl Model for InstallModel {
    type Msg = InstallMsg;

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            InstallMsg::Start(pkg) => {
                self.package = pkg;
                self.status = InstallStatus::Downloading;
                Cmd::batch([
                    Cmd::header("Package Install", &format!("Installing {}", self.package)),
                    Cmd::info(format!("Fetching {}...", self.package)),
                ])
            }
            InstallMsg::DownloadProgress { percent } => {
                self.download_percent = percent;
                if percent >= 100 {
                    self.status = InstallStatus::Installing;
                    Cmd::info("Download complete, installing...".to_string())
                } else {
                    Cmd::none()
                }
            }
            InstallMsg::InstallComplete => {
                self.status = InstallStatus::Complete;
                Cmd::success(format!("{} installed successfully!", self.package))
            }
            InstallMsg::Error(err) => {
                self.status = InstallStatus::Failed;
                self.error = Some(err.clone());
                Cmd::error(format!("Installation failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.status {
            InstallStatus::Idle => format!("Ready to install: {}", self.package),
            InstallStatus::Downloading => {
                let bar_len = self.download_percent / 5;
                format!(
                    "Downloading: {} [{}{}]",
                    self.package,
                    "█".repeat(bar_len),
                    "░".repeat(20 - bar_len)
                )
            }
            InstallStatus::Installing => format!("Installing: {}...", self.package),
            InstallStatus::Complete => format!("✓ {} is ready!", self.package),
            InstallStatus::Failed => format!(
                "✗ Installation failed: {}",
                self.error.as_deref().unwrap_or("unknown error")
            ),
        }
    }
}

/// Status Model - displays system/package status
#[derive(Debug, Clone)]
pub enum StatusMsg {
    Refresh,
    DataReceived { total: usize, updates: usize, orphans: usize },
}

#[derive(Debug, Clone)]
pub struct StatusModel {
    total: usize,
    updates: usize,
    orphans: usize,
}

impl StatusModel {
    pub const fn new() -> Self {
        Self {
            total: 0,
            updates: 0,
            orphans: 0,
        }
    }
}

impl Default for StatusModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Model for StatusModel {
    type Msg = StatusMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::exec(|| StatusMsg::Refresh)
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            StatusMsg::Refresh => Cmd::exec(|| StatusMsg::DataReceived {
                total: 1250,
                updates: 23,
                orphans: 5,
            }),
            StatusMsg::DataReceived { total, updates, orphans } => {
                self.total = total;
                self.updates = updates;
                self.orphans = orphans;
                Cmd::none()
            }
        }
    }

    fn view(&self) -> String {
        format!(
            "{} packages installed{}",
            self.total,
            if self.updates > 0 {
                format!(" ({} updates available)", self.updates)
            } else {
                String::new()
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_model_lifecycle() {
        let mut model = InstallModel::new("test-pkg".to_string());

        let _cmd = model.update(InstallMsg::Start("test-pkg".to_string()));
        assert_eq!(model.status, InstallStatus::Downloading);

        let _cmd = model.update(InstallMsg::DownloadProgress { percent: 100 });
        assert_eq!(model.status, InstallStatus::Installing);

        let _cmd = model.update(InstallMsg::InstallComplete);
        assert_eq!(model.status, InstallStatus::Complete);
    }

    #[test]
    fn test_status_model() {
        let mut model = StatusModel::new();

        let _cmd = model.update(StatusMsg::DataReceived {
            total: 100,
            updates: 5,
            orphans: 1,
        });

        assert_eq!(model.total, 100);
        assert_eq!(model.updates, 5);
        assert!(model.view().contains("100"));
    }
}
