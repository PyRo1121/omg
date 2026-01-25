//! Remove Model - Elm Architecture implementation for remove command
//!
//! Modern, stylish package removal interface with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;
use owo_colors::OwoColorize;
use std::fmt::Write;
use std::sync::Arc;

/// Removal state machine
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveState {
    Idle,
    Confirming,
    Removing,
    Complete,
    Failed,
}

/// Remove messages
#[derive(Debug, Clone)]
pub enum RemoveMsg {
    Start(Vec<String>),
    Confirm(bool),
    Execute,
    Progress(String),
    Complete,
    Error(String),
}

/// Remove model state
#[derive(Debug, Clone)]
pub struct RemoveModel {
    pub packages: Vec<String>,
    pub state: RemoveState,
    pub error: Option<String>,
    pub recursive: bool,
    pub yes: bool,
    pub current_status: String,
}

impl Default for RemoveModel {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            state: RemoveState::Idle,
            error: None,
            recursive: false,
            yes: false,
            current_status: String::new(),
        }
    }
}

impl RemoveModel {
    /// Create new remove model
    #[must_use]
    pub fn new(packages: Vec<String>) -> Self {
        Self {
            packages,
            ..Default::default()
        }
    }

    /// Set recursive mode
    #[must_use]
    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Set auto-confirm mode
    #[must_use]
    pub const fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
    }

    /// Render header
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

impl Model for RemoveModel {
    type Msg = RemoveMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        if self.packages.is_empty() {
            return Cmd::Error("No packages specified".to_string());
        }
        Cmd::Exec(Box::new(|| RemoveMsg::Start(Vec::new())))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            RemoveMsg::Start(_) => {
                // In Start, we just transition to Confirming or Execute based on flags
                if self.yes {
                    Cmd::Exec(Box::new(|| RemoveMsg::Execute))
                } else {
                    self.state = RemoveState::Confirming;
                    Cmd::PrintLn(String::new())
                }
            }
            RemoveMsg::Confirm(should_proceed) => {
                if should_proceed {
                    Cmd::Exec(Box::new(|| RemoveMsg::Execute))
                } else {
                    self.state = RemoveState::Complete;
                    Cmd::batch([
                        Cmd::PrintLn(String::new()),
                        Cmd::Warning("Removal cancelled.".to_string()),
                        Cmd::PrintLn(String::new()),
                    ])
                }
            }
            RemoveMsg::Execute => {
                self.state = RemoveState::Removing;
                self.current_status = "Removing packages...".to_string();

                let packages = self.packages.clone();
                let recursive = self.recursive;

                Cmd::Exec(Box::new(move || {
                    let result = if tokio::runtime::Handle::try_current().is_ok() {
                        std::thread::spawn(move || {
                            let pm = Arc::from(get_package_manager());
                            let service = PackageService::new(pm);
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async { service.remove(&packages, recursive).await })
                        })
                        .join()
                        .unwrap()
                    } else {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async {
                            let pm = Arc::from(get_package_manager());
                            let service = PackageService::new(pm);
                            service.remove(&packages, recursive).await
                        })
                    };

                    match result {
                        Ok(()) => RemoveMsg::Complete,
                        Err(e) => RemoveMsg::Error(e.to_string()),
                    }
                }))
            }
            RemoveMsg::Progress(status) => {
                self.current_status = status;
                Cmd::none()
            }
            RemoveMsg::Complete => {
                self.state = RemoveState::Complete;
                Cmd::batch([
                    Cmd::PrintLn(String::new()),
                    Cmd::Success(format!(
                        "Successfully removed {} package(s)!",
                        self.packages.len()
                    )),
                    Cmd::PrintLn(String::new()),
                ])
            }
            RemoveMsg::Error(err) => {
                self.state = RemoveState::Failed;
                self.error = Some(err.clone());
                Cmd::Error(format!("Removal failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            RemoveState::Idle => String::new(),
            RemoveState::Confirming => {
                let mut output = String::new();
                output.push_str(&Self::render_header(
                    "OMG",
                    &format!("Removing {} package(s)", self.packages.len()),
                ));

                for pkg in &self.packages {
                    let _ = writeln!(output, "  {} {}", style::arrow("→"), style::package(pkg));
                }

                if self.recursive {
                    let _ = writeln!(output, "  {}", style::dim("(Recursive mode enabled)"));
                }

                if !self.yes {
                    let _ = write!(
                        output,
                        "\n{} Proceed with removal? · ",
                        "✓".green().bold()
                    );
                }
                output
            }
            RemoveState::Removing => {
                format!(
                    "{} {} {}",
                    "⟳".cyan(),
                    self.current_status,
                    self.packages.join(", ").dimmed()
                )
            }
            RemoveState::Complete => {
                format!("\n✓ {}\n", "Removal complete!".green().bold())
            }
            RemoveState::Failed => {
                if let Some(err) = &self.error {
                    format!("\n✗ Removal failed: {}\n", err.red())
                } else {
                    "\n✗ Removal failed\n".to_string()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_model_initial_state() {
        let model = RemoveModel::new(vec!["vim".to_string()]);
        assert_eq!(model.state, RemoveState::Idle);
        assert_eq!(model.packages.len(), 1);
        assert!(!model.recursive);
    }

    #[test]
    fn test_remove_model_recursive() {
        let model = RemoveModel::new(vec!["vim".to_string()]).with_recursive(true);
        assert!(model.recursive);
    }

    #[test]
    fn test_remove_model_start_confirm() {
        let mut model = RemoveModel::new(vec!["vim".to_string()]);
        let _cmd = model.update(RemoveMsg::Start(vec![]));
        assert_eq!(model.state, RemoveState::Confirming);
    }

    #[test]
    fn test_remove_model_start_yes() {
        let mut model = RemoveModel::new(vec!["vim".to_string()]).with_yes(true);
        // We can't easily test the transition to Executing in unit test because Start triggers an Exec command
        // But we can verify it doesn't go to Confirming immediately if logic is right (it stays Idle/Start or transitions via command)
        // Actually update returns a command to transition.
        // Let's check state after update.
        // The Start logic returns Exec cmd, but doesn't change state immediately for yes=true.
        // Wait, looking at logic:
        // if self.yes { Cmd::Exec(...) } else { self.state = Confirming; ... }
        // So state remains Idle until Execute msg comes back.
        let _cmd = model.update(RemoveMsg::Start(vec![]));
        assert_eq!(model.state, RemoveState::Idle);
    }
}
