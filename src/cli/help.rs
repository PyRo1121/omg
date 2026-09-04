//! Root help rendering.

use clap::CommandFactory;

use crate::cli::args::Cli;

const COMMON_COMMANDS: &[&str] = &[
    "search", "install", "remove", "update", "info", "outdated", "use", "list", "which", "run",
    "status", "doctor",
];

/// Render focused root help, with an opt-in view of advanced commands.
pub fn print_root_help(show_all: bool) -> anyhow::Result<()> {
    let mut command = Cli::command();
    if !show_all {
        let advanced_commands: Vec<String> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_string())
            .filter(|name| !COMMON_COMMANDS.contains(&name.as_str()))
            .collect();
        for name in advanced_commands {
            command = command.mut_subcommand(name, |subcommand| subcommand.hide(true));
        }
        command = command
            .after_help("Run `omg --help --all-commands` to see advanced commands and workflows.");
    }
    command.print_help()?;
    println!();
    Ok(())
}
