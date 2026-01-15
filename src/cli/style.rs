use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

#[must_use]
pub fn header(msg: &str) -> String {
    format!("{} {}", "==>".magenta().bold(), msg.bold())
}

#[must_use]
pub fn success(msg: &str) -> String {
    format!("{} {}", "✓".green().bold(), msg)
}

#[must_use]
pub fn error(msg: &str) -> String {
    format!("{} {}", "✗".red().bold(), msg)
}

#[must_use]
pub fn info(msg: &str) -> String {
    format!("{} {}", "ℹ".blue().bold(), msg)
}

#[must_use]
pub fn warning(msg: &str) -> String {
    format!("{} {}", "⚠".yellow().bold(), msg)
}

#[must_use]
pub fn arrow(msg: &str) -> String {
    format!("{} {}", "→".cyan().bold(), msg)
}

#[must_use]
pub fn dim(msg: &str) -> String {
    msg.dimmed().to_string()
}

#[must_use]
pub fn command(cmd: &str) -> String {
    format!("`{}`", cmd.cyan())
}

#[must_use]
pub fn url(link: &str) -> String {
    link.underline().blue().to_string()
}

#[must_use]
pub fn package(name: &str) -> String {
    name.white().bold().to_string()
}

#[must_use]
pub fn version(ver: &str) -> String {
    ver.green().to_string()
}

#[must_use]
#[allow(clippy::literal_string_with_formatting_args)]
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}
