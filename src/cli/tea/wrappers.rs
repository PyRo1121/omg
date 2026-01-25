//! Integration wrappers for Elm-based models

use crate::cli::tea::{InfoModel, InstallModel, Program, StatusModel};

// Import Model trait for use in test modules
#[cfg(test)]
use crate::cli::tea::Model;

/// Run status command using Elm Architecture
pub fn run_status_elm(fast: bool) -> Result<(), std::io::Error> {
    let model = StatusModel::new().with_fast_mode(fast);
    Program::new(model).run()
}

/// Run info command using Elm Architecture
pub fn run_info_elm(package: String) -> Result<(), std::io::Error> {
    let model = InfoModel::new(package);
    Program::new(model).run()
}

/// Run install command using Elm Architecture
pub fn run_install_elm(packages: Vec<String>) -> Result<(), std::io::Error> {
    let model = InstallModel::new(packages);
    Program::new(model).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_wrapper_creates_model() {
        let model = StatusModel::new().with_fast_mode(true);
        assert!(model.view().contains("No status"));
    }

    #[test]
    fn test_info_wrapper_creates_model() {
        let model = InfoModel::new("test-pkg".to_string());
        assert_eq!(model.package_name(), "test-pkg");
    }

    #[test]
    fn test_install_wrapper_creates_model() {
        let model = InstallModel::new(vec!["pkg1".to_string(), "pkg2".to_string()]);
        assert_eq!(model.packages().len(), 2);
    }
}
