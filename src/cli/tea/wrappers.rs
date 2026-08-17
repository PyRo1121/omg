//! Integration wrappers for Elm-based models

use crate::cli::tea::{InfoModel, Program, StatusModel};

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
