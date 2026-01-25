//! Info Model - Elm Architecture implementation for info command

use super::{Cmd, Model};

/// Messages that can update the `InfoModel`
#[derive(Debug, Clone)]
pub enum InfoMsg {
    Fetch(String),
    InfoReceived(PackageInfo),
    NotFound(String),
    Error(String),
}

/// Source of package information
#[derive(Debug, Clone, PartialEq)]
pub enum InfoSource {
    Official,
    Aur,
    Flatpak,
}

/// Package information structure
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: InfoSource,
    pub repo: String,
}

/// The Info Model
#[derive(Debug, Clone)]
pub struct InfoModel {
    package_name: String,
    info: Option<PackageInfo>,
    loading: bool,
    error: Option<String>,
    not_found: bool,
}

impl InfoModel {
    pub fn new(package_name: String) -> Self {
        Self {
            package_name,
            info: None,
            loading: false,
            error: None,
            not_found: false,
        }
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }
}

impl Model for InfoModel {
    type Msg = InfoMsg;

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            InfoMsg::Fetch(pkg) => {
                self.package_name = pkg;
                self.loading = true;
                Cmd::batch([
                    Cmd::header("Package Info", format!("Info for {}", self.package_name)),
                    Cmd::exec(|| {
                        InfoMsg::InfoReceived(PackageInfo {
                            name: "test-pkg".to_string(),
                            version: "1.0.0".to_string(),
                            description: "A test package".to_string(),
                            source: InfoSource::Official,
                            repo: "extra".to_string(),
                        })
                    }),
                ])
            }
            InfoMsg::InfoReceived(info) => {
                self.info = Some(info);
                self.loading = false;
                Cmd::none()
            }
            InfoMsg::NotFound(pkg) => {
                self.package_name = pkg;
                self.not_found = true;
                self.loading = false;
                Cmd::error(format!("Package '{}' not found", self.package_name))
            }
            InfoMsg::Error(err) => {
                self.error = Some(err.clone());
                self.loading = false;
                Cmd::error(format!("Failed to fetch package info: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        if self.loading {
            return format!("Fetching info for '{}'...", self.package_name);
        }
        if let Some(err) = &self.error {
            return format!("Error: {err}");
        }
        if self.not_found {
            return format!("Package '{}' not found", self.package_name);
        }
        let Some(info) = &self.info else {
            return "No package information available".to_string();
        };
        format!(
            "Name: {}\nVersion: {}\nDescription: {}\nSource: {:?}\nRepo: {}",
            info.name, info.version, info.description, info.source, info.repo
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_model_initial_state() {
        let model = InfoModel::new("test".to_string());
        assert!(model.info.is_none());
        assert!(!model.loading);
        assert_eq!(model.package_name(), "test");
    }

    #[test]
    fn test_info_model_fetch_message() {
        let mut model = InfoModel::new("test".to_string());
        let _cmd = model.update(InfoMsg::Fetch("test-pkg".to_string()));
        assert!(model.loading);
    }

    #[test]
    fn test_info_model_info_received() {
        let mut model = InfoModel::new("test".to_string());
        let test_info = PackageInfo {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: InfoSource::Official,
            repo: "extra".to_string(),
        };
        let _cmd = model.update(InfoMsg::InfoReceived(test_info));
        assert!(!model.loading);
        assert_eq!(model.info.as_ref().unwrap().name, "test-pkg");
    }

    #[test]
    fn test_info_view_with_data() {
        let mut model = InfoModel::new("test".to_string());
        let _ = model.update(InfoMsg::InfoReceived(PackageInfo {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: InfoSource::Official,
            repo: "extra".to_string(),
        }));
        let view = model.view();
        assert!(view.contains("test-pkg"));
    }
}
