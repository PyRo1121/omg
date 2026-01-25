//! Install Model - Elm Architecture implementation for install command

use super::{Cmd, Model};

/// Messages that can update the `InstallModel`
#[derive(Debug, Clone)]
pub enum InstallMsg {
    Start(Vec<String>),
    AnalysisComplete {
        packages: Vec<String>,
    },
    DownloadProgress {
        package: String,
        percent: usize,
    },
    InstallComplete,
    Error(String),
    PackageNotFound {
        package: String,
        suggestions: Vec<String>,
    },
}

/// Installation state
#[derive(Debug, Clone, PartialEq)]
pub enum InstallState {
    Idle,
    Analyzing,
    Downloading,
    Installing,
    Complete,
    Failed,
    NotFound,
}

/// The Install Model
#[derive(Debug, Clone)]
pub struct InstallModel {
    packages: Vec<String>,
    state: InstallState,
    error: Option<String>,
    download_percent: usize,
    current_package: Option<String>,
    suggestions: Vec<String>,
}

impl InstallModel {
    pub fn new(packages: Vec<String>) -> Self {
        Self {
            packages,
            state: InstallState::Idle,
            error: None,
            download_percent: 0,
            current_package: None,
            suggestions: Vec::new(),
        }
    }

    pub fn packages(&self) -> &[String] {
        &self.packages
    }
}

impl Model for InstallModel {
    type Msg = InstallMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::batch([
            Cmd::header(
                "Install",
                format!("Installing {} package(s)", self.packages.len()),
            ),
            Cmd::info("Analyzing packages...".to_string()),
        ])
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            InstallMsg::Start(pkgs) => {
                self.packages = pkgs;
                self.state = InstallState::Analyzing;
                Cmd::exec(|| InstallMsg::AnalysisComplete {
                    packages: vec!["test-pkg".to_string()],
                })
            }
            InstallMsg::AnalysisComplete { packages } => {
                self.packages = packages;
                self.state = InstallState::Downloading;
                self.current_package = self.packages.first().cloned();
                Cmd::info(format!(
                    "Downloading {}...",
                    self.current_package.as_deref().unwrap_or("")
                ))
            }
            InstallMsg::DownloadProgress { package, percent } => {
                self.current_package = Some(package);
                self.download_percent = percent.clamp(0, 100);
                if self.download_percent >= 100 {
                    self.state = InstallState::Installing;
                    Cmd::info("Download complete, installing...".to_string())
                } else {
                    Cmd::none()
                }
            }
            InstallMsg::InstallComplete => {
                self.state = InstallState::Complete;
                Cmd::success(format!(
                    "{} package(s) installed successfully!",
                    self.packages.len()
                ))
            }
            InstallMsg::Error(err) => {
                self.state = InstallState::Failed;
                self.error = Some(err.clone());
                Cmd::error(format!("Installation failed: {err}"))
            }
            InstallMsg::PackageNotFound {
                package,
                suggestions,
            } => {
                self.state = InstallState::NotFound;
                self.current_package = Some(package);
                self.suggestions = suggestions;
                Cmd::error(format!(
                    "Package '{}' not found",
                    self.current_package.as_deref().unwrap_or("")
                ))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            InstallState::Idle => format!("Ready to install: {}", self.packages.join(", ")),
            InstallState::Analyzing => format!("Analyzing {} package(s)...", self.packages.len()),
            InstallState::Downloading => {
                let clamped = self.download_percent.min(100);
                let filled = clamped / 5;
                format!(
                    "Downloading: {} [{}{}] {}%",
                    self.current_package.as_deref().unwrap_or("unknown"),
                    "█".repeat(filled),
                    "░".repeat(20 - filled),
                    self.download_percent
                )
            }
            InstallState::Installing => format!(
                "Installing: {}...",
                self.current_package.as_deref().unwrap_or("unknown")
            ),
            InstallState::Complete => format!(
                "✓ {} package(s) installed successfully!",
                self.packages.len()
            ),
            InstallState::Failed => format!(
                "✗ Installation failed: {}",
                self.error.as_deref().unwrap_or("unknown")
            ),
            InstallState::NotFound => {
                if self.suggestions.is_empty() {
                    format!(
                        "✗ Package '{}' not found",
                        self.current_package.as_deref().unwrap_or("unknown")
                    )
                } else {
                    format!(
                        "✗ Package '{}' not found. Did you mean:\n  - {}",
                        self.current_package.as_deref().unwrap_or(""),
                        self.suggestions.join("\n  - ")
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_model_initial_state() {
        let model = InstallModel::new(vec!["pkg1".to_string()]);
        assert_eq!(model.packages().len(), 1);
        assert_eq!(model.state, InstallState::Idle);
    }

    #[test]
    fn test_install_model_start_message() {
        let mut model = InstallModel::new(Vec::new());
        let _cmd = model.update(InstallMsg::Start(vec!["pkg1".to_string()]));
        assert_eq!(model.state, InstallState::Analyzing);
    }

    #[test]
    fn test_install_model_complete() {
        let mut model = InstallModel::new(vec!["pkg1".to_string()]);
        let _cmd = model.update(InstallMsg::InstallComplete);
        assert_eq!(model.state, InstallState::Complete);
    }

    #[test]
    fn test_install_view_complete() {
        let mut model = InstallModel::new(vec!["pkg1".to_string()]);
        model.state = InstallState::Complete;
        let view = model.view();
        assert!(view.contains("✓"));
        assert!(view.contains("installed successfully"));
    }
}
