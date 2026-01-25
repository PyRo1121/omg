//! Status Model - Elm Architecture implementation for status command

use super::{Cmd, Model};

/// Messages that can update the `StatusModel`
#[derive(Debug, Clone)]
pub enum StatusMsg {
    Refresh,
    DataReceived(StatusData),
    Error(String),
}

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

/// The Status Model
#[derive(Debug, Clone)]
pub struct StatusModel {
    data: Option<StatusData>,
    loading: bool,
    error: Option<String>,
    fast_mode: bool,
}

impl StatusModel {
    pub const fn new() -> Self {
        Self {
            data: None,
            loading: false,
            error: None,
            fast_mode: false,
        }
    }

    #[must_use]
    pub const fn with_fast_mode(mut self, fast: bool) -> Self {
        self.fast_mode = fast;
        self
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
            StatusMsg::Refresh => {
                self.loading = true;
                self.error = None;
                let fast = self.fast_mode;
                Cmd::exec(move || {
                    StatusMsg::DataReceived(StatusData {
                        total_packages: 1250,
                        explicit_packages: 450,
                        orphan_packages: 5,
                        updates_available: 23,
                        duration_ms: 1.2,
                        fast_mode: fast,
                    })
                })
            }
            StatusMsg::DataReceived(data) => {
                self.data = Some(data);
                self.loading = false;
                Cmd::none()
            }
            StatusMsg::Error(err) => {
                self.error = Some(err.clone());
                self.loading = false;
                Cmd::error(format!("Failed to fetch status: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        if self.loading {
            return "Loading status...".to_string();
        }
        if let Some(err) = &self.error {
            return format!("Error: {err}");
        }
        let Some(data) = &self.data else {
            return "No status data available".to_string();
        };
        format!(
            "Status Overview ({:.1}ms)\nTotal: {}\nExplicit: {}\nOrphans: {}\nUpdates: {}",
            data.duration_ms,
            data.total_packages,
            data.explicit_packages,
            data.orphan_packages,
            data.updates_available
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_model_initial_state() {
        let model = StatusModel::new();
        assert!(model.data.is_none());
        assert!(!model.loading);
        assert!(model.error.is_none());
    }

    #[test]
    fn test_status_model_with_fast_mode() {
        let model = StatusModel::new().with_fast_mode(true);
        assert!(model.fast_mode);
    }

    #[test]
    fn test_status_model_refresh_message() {
        let mut model = StatusModel::new();
        let _cmd = model.update(StatusMsg::Refresh);
        assert!(model.loading);
    }

    #[test]
    fn test_status_model_data_received() {
        let mut model = StatusModel::new();
        let test_data = StatusData {
            total_packages: 100,
            explicit_packages: 50,
            orphan_packages: 2,
            updates_available: 5,
            duration_ms: 10.0,
            fast_mode: false,
        };
        let _cmd = model.update(StatusMsg::DataReceived(test_data));
        assert!(!model.loading);
        assert_eq!(model.data.as_ref().unwrap().total_packages, 100);
    }

    #[test]
    fn test_status_view_with_data() {
        let mut model = StatusModel::new();
        let _ = model.update(StatusMsg::DataReceived(StatusData {
            total_packages: 100,
            explicit_packages: 50,
            orphan_packages: 2,
            updates_available: 5,
            duration_ms: 10.0,
            fast_mode: false,
        }));
        let view = model.view();
        assert!(view.contains("100"));
    }
}
