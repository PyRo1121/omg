//! Status Model - Elm Architecture implementation for status command
//!
//! Modern, stylish system status dashboard with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};
use crate::package_managers::get_package_manager;
use owo_colors::OwoColorize;
use std::fmt::Write;

/// Status data structure
#[derive(Debug, Clone, Default)]
pub struct StatusData {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
    pub duration_ms: f64,
    pub fast_mode: bool,
}

/// Status state machine
#[derive(Debug, Clone, PartialEq)]
pub enum StatusState {
    Idle,
    Loading,
    Complete,
    Failed,
}

/// Status messages
#[derive(Debug, Clone)]
pub enum StatusMsg {
    Load,
    Loaded(StatusData),
    Error(String),
}

/// Status model state
#[derive(Debug, Clone)]
pub struct StatusModel {
    pub data: Option<StatusData>,
    pub state: StatusState,
    pub error: Option<String>,
    pub fast_mode: bool,
}

impl Default for StatusModel {
    fn default() -> Self {
        Self {
            data: None,
            state: StatusState::Idle,
            error: None,
            fast_mode: false,
        }
    }
}

impl StatusModel {
    /// Create new status model
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set fast mode
    #[must_use]
    pub const fn with_fast_mode(mut self, fast: bool) -> Self {
        self.fast_mode = fast;
        self
    }

    /// Render a metric line
    fn render_metric(label: &str, value: &str) -> String {
        format!("  {:<20} {}", label.bold(), value)
    }
}

impl Model for StatusModel {
    type Msg = StatusMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::Exec(Box::new(|| StatusMsg::Load))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            StatusMsg::Load => {
                self.state = StatusState::Loading;
                let fast = self.fast_mode;

                Cmd::Exec(Box::new(move || {
                    // This logic mirrors the original status.rs logic
                    let start = std::time::Instant::now();

                    // 1. Try Daemon (Hot Path)
                    let daemon_result = if tokio::runtime::Handle::try_current().is_ok() {
                        // Already in runtime
                        std::thread::spawn(move || {
                            let Ok(rt) = tokio::runtime::Runtime::new() else {
                                return None;
                            };
                            rt.block_on(async {
                                if let Ok(mut client) = DaemonClient::connect().await
                                    && let Ok(ResponseResult::Status(status)) =
                                        client.call(Request::Status { id: 0 }).await
                                {
                                    return Some(StatusData {
                                        total_packages: status.total_packages,
                                        explicit_packages: status.explicit_packages,
                                        orphan_packages: status.orphan_packages,
                                        updates_available: status.updates_available,
                                        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                                        fast_mode: fast,
                                    });
                                }
                                None
                            })
                        })
                        .join()
                        .ok()
                        .flatten()
                    } else {
                        // Create runtime
                        let Ok(rt) = tokio::runtime::Runtime::new() else {
                            return StatusMsg::Error("Failed to create async runtime".to_string());
                        };
                        rt.block_on(async {
                            if let Ok(mut client) = DaemonClient::connect().await
                                && let Ok(ResponseResult::Status(status)) =
                                    client.call(Request::Status { id: 0 }).await
                            {
                                return Some(StatusData {
                                    total_packages: status.total_packages,
                                    explicit_packages: status.explicit_packages,
                                    orphan_packages: status.orphan_packages,
                                    updates_available: status.updates_available,
                                    duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                                    fast_mode: fast,
                                });
                            }
                            None
                        })
                    };

                    if let Some(data) = daemon_result {
                        return StatusMsg::Loaded(data);
                    }

                    // 2. Fallback to direct path
                    let fallback_result: anyhow::Result<(usize, usize, usize, usize)> =
                        if tokio::runtime::Handle::try_current().is_ok() {
                            std::thread::spawn(move || {
                                let Ok(rt) = tokio::runtime::Runtime::new() else {
                                    return Err(anyhow::anyhow!("Failed to create async runtime"));
                                };
                                rt.block_on(async {
                                    let pm = get_package_manager();
                                    pm.get_status(fast).await
                                })
                            })
                            .join()
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))
                        } else {
                            let Ok(rt) = tokio::runtime::Runtime::new() else {
                                return StatusMsg::Error(
                                    "Failed to create async runtime".to_string(),
                                );
                            };
                            rt.block_on(async {
                                let pm = get_package_manager();
                                pm.get_status(fast).await
                            })
                        };

                    match fallback_result {
                        Ok((total, explicit, orphans, updates)) => StatusMsg::Loaded(StatusData {
                            total_packages: total,
                            explicit_packages: explicit,
                            orphan_packages: orphans,
                            updates_available: updates,
                            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                            fast_mode: fast,
                        }),
                        Err(e) => StatusMsg::Error(e.to_string()),
                    }
                }))
            }
            StatusMsg::Loaded(data) => {
                self.data = Some(data);
                self.state = StatusState::Complete;
                Cmd::none()
            }
            StatusMsg::Error(err) => {
                self.state = StatusState::Failed;
                self.error = Some(err.clone());
                Cmd::Error(format!("Status check failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            StatusState::Idle => "No status data available".to_string(),
            StatusState::Loading => "⟳ Gathering system status...".cyan().dimmed().to_string(),
            StatusState::Complete => {
                if let Some(data) = &self.data {
                    let mut output = String::new();

                    // Header
                    let _ = writeln!(
                        output,
                        "  {} Status Overview ({:.1}ms)",
                        "📋".bold(),
                        data.duration_ms
                    );
                    let _ = writeln!(output, "  {}", "─".repeat(40).dimmed());

                    // Metrics
                    let _ = writeln!(
                        output,
                        "{}",
                        Self::render_metric(
                            "Total Packages:",
                            &data.total_packages.to_string().cyan().to_string()
                        )
                    );
                    let _ = writeln!(
                        output,
                        "{}",
                        Self::render_metric(
                            "Explicitly Installed:",
                            &data.explicit_packages.to_string().green().to_string()
                        )
                    );

                    if data.fast_mode {
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_metric(
                                "Orphans/Updates:",
                                &"skipped (fast mode)".dimmed().to_string()
                            )
                        );
                    } else {
                        let orphans_str = if data.orphan_packages > 0 {
                            data.orphan_packages.to_string().yellow().to_string()
                        } else {
                            "0".dimmed().to_string()
                        };
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_metric("Orphan Packages:", &orphans_str)
                        );

                        let updates_str = if data.updates_available > 0 {
                            data.updates_available
                                .to_string()
                                .bright_magenta()
                                .to_string()
                        } else {
                            "0".dimmed().to_string()
                        };
                        let _ = writeln!(
                            output,
                            "{}",
                            Self::render_metric("Updates Available:", &updates_str)
                        );
                    }

                    // Tip
                    let _ = writeln!(
                        output,
                        "\n  {} {}",
                        style::arrow("Tip:"),
                        style::dim("Use 'omg clean' to remove orphans and free up disk space.")
                    );

                    output
                } else {
                    "No data available".to_string()
                }
            }
            StatusState::Failed => {
                if let Some(err) = &self.error {
                    format!("\n✗ Status failed: {}\n", err.red())
                } else {
                    "\n✗ Status failed\n".to_string()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_model_initial_state() {
        let model = StatusModel::new();
        assert_eq!(model.state, StatusState::Idle);
        assert!(model.data.is_none());
        assert!(!model.fast_mode);
    }

    #[test]
    fn test_status_model_fast_mode() {
        let model = StatusModel::new().with_fast_mode(true);
        assert!(model.fast_mode);
    }

    #[test]
    fn test_status_model_loading() {
        let mut model = StatusModel::new();
        let _cmd = model.update(StatusMsg::Load);
        assert_eq!(model.state, StatusState::Loading);
    }

    #[test]
    fn test_status_model_loaded() {
        let mut model = StatusModel::new();
        let data = StatusData {
            total_packages: 100,
            explicit_packages: 50,
            orphan_packages: 2,
            updates_available: 5,
            duration_ms: 10.0,
            fast_mode: false,
        };
        let _cmd = model.update(StatusMsg::Loaded(data));
        assert_eq!(model.state, StatusState::Complete);
        assert!(model.data.is_some());
    }
}
