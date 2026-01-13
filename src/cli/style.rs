use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

pub fn header(msg: &str) -> String {
    format!("{} {}", "==>".magenta().bold(), msg.bold())
}

pub fn success(msg: &str) -> String {
    format!("{} {}", "✓".green().bold(), msg)
}

pub fn error(msg: &str) -> String {
    format!("{} {}", "✗".red().bold(), msg)
}

pub fn info(msg: &str) -> String {
    format!("{} {}", "ℹ".blue().bold(), msg)
}

pub fn warning(msg: &str) -> String {
    format!("{} {}", "⚠".yellow().bold(), msg)
}

pub fn arrow(msg: &str) -> String {
    format!("{} {}", "→".cyan().bold(), msg)
}

pub fn dim(msg: &str) -> String {
    msg.dimmed().to_string()
}

pub fn command(cmd: &str) -> String {
    format!("`{}`", cmd.cyan())
}

pub fn url(link: &str) -> String {
    link.underline().blue().to_string()
}

pub fn package(name: &str) -> String {
    name.white().bold().to_string()
}

pub fn version(ver: &str) -> String {
    ver.green().to_string()
}

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
