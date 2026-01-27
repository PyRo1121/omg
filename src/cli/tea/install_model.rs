//! Install Model - Elm Architecture implementation for install command
//!
//! Modern, stylish package installation interface with Bubble Tea-inspired UX.
//!
//! Modernized to use the unified Components library.

use crate::cli::components::Components;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use owo_colors::OwoColorize;

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
                Cmd::batch([
                    Components::loading("Resolving packages..."),
                    // In a real Elm app, we'd do the resolution here.
                    // For now, we assume packages are valid or will fail during execution
                    // to match the existing behavior where PackageService handles validation.
                    Cmd::Exec(Box::new({
                        let pkgs = self.packages.clone();
                        move || InstallMsg::PackagesResolved(pkgs)
                    })),
                ])
            }
            InstallMsg::PackagesResolved(pkgs) => {
                self.packages = pkgs;

                let items: Vec<(String, Option<String>)> = self
                    .packages
                    .iter()
                    .map(|p| (p.clone(), Some(String::new())))
                    .collect();
                let pkg_list_cmd = Components::package_list(
                    format!("Installing {} package(s)", self.packages.len()),
                    items,
                );

                if self.yes {
                    Cmd::batch([pkg_list_cmd, Cmd::Exec(Box::new(|| InstallMsg::Execute))])
                } else {
                    self.state = InstallState::Confirming;
                    Cmd::batch([pkg_list_cmd, Components::confirm("Installation", "Enter")])
                }
            }
            InstallMsg::Confirm(should_proceed) => {
                if should_proceed {
                    Cmd::Exec(Box::new(|| InstallMsg::Execute))
                } else {
                    self.state = InstallState::Complete;
                    Cmd::warning("Installation cancelled.")
                }
            }
            InstallMsg::Execute => {
                self.state = InstallState::Installing;
                self.current_status = "Installing...".to_string();

                let packages = self.packages.clone();
                let yes = self.yes;

                Cmd::batch([
                    Cmd::info("Installing packages..."),
                    Cmd::Exec(Box::new(move || {
                        // This blocks, but in a real async runtime we'd spawn it properly.
                        // For CLI tools, blocking briefly is often acceptable, but we'll use
                        // a thread to be safe if a runtime exists.

                        let packages_task = packages.clone();
                        let result = if tokio::runtime::Handle::try_current().is_ok() {
                            std::thread::spawn(move || {
                                let pm = get_package_manager();
                                let service = PackageService::new(pm);
                                let Ok(rt) = tokio::runtime::Runtime::new() else {
                                    return Err(anyhow::anyhow!("Failed to create async runtime"));
                                };
                                rt.block_on(async { service.install(&packages_task, yes).await })
                            })
                            .join()
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))
                        } else {
                            let Ok(rt) = tokio::runtime::Runtime::new() else {
                                return InstallMsg::Error(
                                    "Failed to create async runtime".to_string(),
                                );
                            };
                            rt.block_on(async {
                                let pm = get_package_manager();
                                let service = PackageService::new(pm);
                                service.install(&packages, yes).await
                            })
                        };

                        match result {
                            Ok(()) => InstallMsg::Complete,
                            Err(e) => {
                                let msg = e.to_string();
                                if msg.contains("not found") {
                                    // Basic extraction of package name from error message if possible
                                    // ideally the error would be structured
                                    if let Some(pkg) = packages.iter().find(|p| msg.contains(*p)) {
                                        // We don't have suggestions here easily without calling the daemon
                                        // So we just report it as PackageNotFound with empty suggestions
                                        // The original install.rs logic for suggestions is complex to port
                                        // fully inside this blocking closure without async/await access
                                        return InstallMsg::PackageNotFound {
                                            package: pkg.clone(),
                                            suggestions: Vec::new(),
                                        };
                                    }
                                }
                                InstallMsg::Error(e.to_string())
                            }
                        }
                    })),
                ])
            }
            InstallMsg::Progress(status) => {
                self.current_status = status;
                // Spinner update if we had a handle
                Cmd::none()
            }
            InstallMsg::Complete => {
                self.state = InstallState::Complete;
                Components::complete(format!(
                    "Successfully installed {} package(s)!",
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
                self.state = InstallState::Failed;
                self.suggestions = Some((package.clone(), suggestions.clone()));

                if suggestions.is_empty() {
                    Cmd::error(format!("Package '{package}' not found."))
                } else {
                    Components::error_with_suggestion(
                        format!("Package '{package}' not found."),
                        format!("Did you mean: {}", suggestions.join(", ")),
                    )
                }
            }
        }
    }

    fn view(&self) -> String {
        // View is handled by Components/Cmds side-effects
        match self.state {
            InstallState::Installing => {
                // Show a simple spinner status if needed, or rely on Components::spinner
                // Since we don't have a persistent spinner component that updates via view(),
                // we can return a string here for the render loop.
                format!("{} {}", "⟳".cyan(), self.current_status)
            }
            _ => String::new(),
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
