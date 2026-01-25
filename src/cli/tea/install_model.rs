//! Install Model - Elm Architecture implementation for install command
//!
//! Modern, stylish package installation interface with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use owo_colors::OwoColorize;
use std::fmt::Write;
use std::sync::Arc;

/// Installation state machine
#[derive(Debug, Clone, PartialEq)]
pub enum InstallState {
    Idle,
    Resolving,
    Confirming,
    Installing,
    Complete,
    Failed,
}

/// Install messages
#[derive(Debug, Clone)]
pub enum InstallMsg {
    Resolve,
    PackagesResolved(Vec<String>),
    Confirm(bool),
    Execute,
    Progress(String),
    Complete,
    Error(String),
    PackageNotFound {
        package: String,
        suggestions: Vec<String>,
    },
}

/// Install model state
#[derive(Debug, Clone)]
pub struct InstallModel {
    pub packages: Vec<String>,
    pub state: InstallState,
    pub error: Option<String>,
    pub suggestions: Option<(String, Vec<String>)>,
    pub current_status: String,
    pub yes: bool,
}

impl Default for InstallModel {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            state: InstallState::Idle,
            error: None,
            suggestions: None,
            current_status: String::new(),
            yes: false,
        }
    }
}

impl InstallModel {
    /// Create new install model
    #[must_use]
    pub fn new(packages: Vec<String>) -> Self {
        Self {
            packages,
            ..Default::default()
        }
    }

    /// Set auto-confirm mode
    #[must_use]
    pub const fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
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

impl Model for InstallModel {
    type Msg = InstallMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        if self.packages.is_empty() {
            return Cmd::Error("No packages specified".to_string());
        }
        Cmd::Exec(Box::new(|| InstallMsg::Resolve))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            InstallMsg::Resolve => {
                self.state = InstallState::Resolving;

                // In a real Elm app, we'd do the resolution here.
                // For now, we assume packages are valid or will fail during execution
                // to match the existing behavior where PackageService handles validation.

                let pkgs = self.packages.clone();
                Cmd::Exec(Box::new(move || InstallMsg::PackagesResolved(pkgs)))
            }
            InstallMsg::PackagesResolved(pkgs) => {
                self.packages = pkgs;
                if self.yes {
                    Cmd::Exec(Box::new(|| InstallMsg::Execute))
                } else {
                    self.state = InstallState::Confirming;
                    Cmd::PrintLn(String::new())
                }
            }
            InstallMsg::Confirm(should_proceed) => {
                if should_proceed {
                    Cmd::Exec(Box::new(|| InstallMsg::Execute))
                } else {
                    self.state = InstallState::Complete;
                    Cmd::batch([
                        Cmd::PrintLn(String::new()),
                        Cmd::Warning("Installation cancelled.".to_string()),
                        Cmd::PrintLn(String::new()),
                    ])
                }
            }
            InstallMsg::Execute => {
                self.state = InstallState::Installing;
                self.current_status = "Installing...".to_string();

                let packages = self.packages.clone();
                let yes = self.yes;

                Cmd::Exec(Box::new(move || {
                    // This blocks, but in a real async runtime we'd spawn it properly.
                    // For CLI tools, blocking briefly is often acceptable, but we'll use
                    // a thread to be safe if a runtime exists.

                    let result = if tokio::runtime::Handle::try_current().is_ok() {
                        std::thread::spawn(move || {
                            let pm = Arc::from(get_package_manager());
                            let service = PackageService::new(pm);
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async { service.install(&packages, yes).await })
                        })
                        .join()
                        .unwrap()
                    } else {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async {
                            let pm = Arc::from(get_package_manager());
                            let service = PackageService::new(pm);
                            service.install(&packages, yes).await
                        })
                    };

                    match result {
                        Ok(()) => InstallMsg::Complete,
                        Err(e) => {
                            // Basic fuzzy matching hook could go here if we extracted logic from install.rs
                            // For now, simple error reporting
                            InstallMsg::Error(e.to_string())
                        }
                    }
                }))
            }
            InstallMsg::Progress(status) => {
                self.current_status = status;
                Cmd::none()
            }
            InstallMsg::Complete => {
                self.state = InstallState::Complete;
                Cmd::batch([
                    Cmd::PrintLn(String::new()),
                    Cmd::Success(format!(
                        "Successfully installed {} package(s)!",
                        self.packages.len()
                    )),
                    Cmd::PrintLn(String::new()),
                ])
            }
            InstallMsg::Error(err) => {
                self.state = InstallState::Failed;
                self.error = Some(err.clone());
                Cmd::Error(format!("Installation failed: {err}"))
            }
            InstallMsg::PackageNotFound {
                package,
                suggestions,
            } => {
                self.state = InstallState::Failed; // Or specialized state
                self.suggestions = Some((package, suggestions));
                Cmd::none()
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            InstallState::Idle => String::new(),
            InstallState::Resolving => "⟳ Resolving packages...".cyan().dimmed().to_string(),
            InstallState::Confirming => {
                let mut output = String::new();
                output.push_str(&Self::render_header(
                    "OMG",
                    &format!("Installing {} package(s)", self.packages.len()),
                ));

                for pkg in &self.packages {
                    let _ = writeln!(output, "  {} {}", style::arrow("→"), style::package(pkg));
                }

                if !self.yes {
                    let _ = write!(
                        output,
                        "\n{} Proceed with installation? · ",
                        "✓".green().bold()
                    );
                }
                output
            }
            InstallState::Installing => {
                format!(
                    "{} {} {}",
                    "⟳".cyan(), // Simple spinner for now
                    self.current_status,
                    self.packages.join(", ").dimmed()
                )
            }
            InstallState::Complete => {
                format!("\n✓ {}\n", "Installation complete!".green().bold())
            }
            InstallState::Failed => {
                if let Some((pkg, suggestions)) = &self.suggestions {
                    let mut s = format!("\n✗ Package '{}' not found.", pkg.red());
                    if !suggestions.is_empty() {
                        s.push_str("\n\nDid you mean one of these?");
                        for sug in suggestions {
                            let _ = write!(s, "\n  - {}", style::package(sug));
                        }
                    }
                    s
                } else if let Some(err) = &self.error {
                    format!("\n✗ Installation failed: {}\n", err.red())
                } else {
                    "\n✗ Installation failed\n".to_string()
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
        let model = InstallModel::new(vec!["vim".to_string()]);
        assert_eq!(model.state, InstallState::Idle);
        assert_eq!(model.packages.len(), 1);
        assert_eq!(model.packages[0], "vim");
    }

    #[test]
    fn test_install_model_resolve() {
        let mut model = InstallModel::new(vec!["vim".to_string()]);
        let _cmd = model.update(InstallMsg::Resolve);
        // The real implementation is async, but update sets state
        assert_eq!(model.state, InstallState::Resolving);
    }

    #[test]
    fn test_install_model_confirming() {
        let mut model = InstallModel::new(vec!["vim".to_string()]);
        let _cmd = model.update(InstallMsg::PackagesResolved(vec!["vim".to_string()]));
        assert_eq!(model.state, InstallState::Confirming);
    }

    #[test]
    fn test_install_model_complete() {
        let mut model = InstallModel::new(vec!["vim".to_string()]);
        let _cmd = model.update(InstallMsg::Complete);
        assert_eq!(model.state, InstallState::Complete);
    }
}
