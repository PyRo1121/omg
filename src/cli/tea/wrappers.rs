//! Integration wrappers for Elm-based models

use crate::cli::tea::{InfoModel, InstallModel, Program, SearchModel, StatusModel, UpdateModel};

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
pub fn run_install_elm(packages: Vec<String>, yes: bool) -> Result<(), std::io::Error> {
    let model = InstallModel::new(packages).with_yes(yes);
    Program::new(model).run()
}

/// Run update command using Elm Architecture
pub fn run_update_elm(check_only: bool, yes: bool) -> Result<(), std::io::Error> {
    let model = UpdateModel::new().with_check_only(check_only).with_yes(yes);
    Program::new(model).run()
}

/// Run search command using Elm Architecture
pub fn run_search_elm(query: String) -> Result<(), std::io::Error> {
    let model = SearchModel::new().with_query(query);
    Program::new(model).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_wrapper_creates_model() {
        let model = StatusModel::new().with_fast_mode(true);
        assert!(model.fast_mode);
    }

    #[test]
    fn test_info_wrapper_creates_model() {
        let model = InfoModel::new("test-pkg".to_string());
        assert_eq!(model.package_name(), "test-pkg");
    }

    #[test]
    fn test_install_wrapper_creates_model() {
        let model = InstallModel::new(vec!["pkg1".to_string(), "pkg2".to_string()]);
        assert_eq!(model.packages.len(), 2);
    }

    #[test]
    fn test_update_wrapper_creates_model() {
        let model = UpdateModel::new().with_check_only(true).with_yes(false);
        assert!(model.check_only);
        assert!(!model.yes);
    }
}
