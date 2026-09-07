//! Charm-inspired UI primitives for OMG CLI
//!
//! Provides high-polish components like cards, tips, and contextual headers.
//! Implements a "Lip Gloss" compatible API using owo-colors for the Bubble Tea feel.

use owo_colors::OwoColorize;
use std::fmt::Display;

/// Color palette matching standard TUI needs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Blue,
    Cyan,
    Yellow,
    Magenta,
    White,
    Black,
    Gray,
}

/// A builder-pattern style struct mimicking Lip Gloss
#[derive(Debug, Clone, Default)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    is_bold: bool,
    is_italic: bool,
    padding_left: usize,
    padding_right: usize,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn foreground(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    #[must_use]
    pub fn bold(mut self, yes: bool) -> Self {
        self.is_bold = yes;
        self
    }

    #[must_use]
    pub fn italic(mut self, yes: bool) -> Self {
        self.is_italic = yes;
        self
    }

    #[must_use]
    pub fn padding_left(mut self, n: usize) -> Self {
        self.padding_left = n;
        self
    }

    #[must_use]
    pub fn padding_right(mut self, n: usize) -> Self {
        self.padding_right = n;
        self
    }

    pub fn render<S: Display>(&self, text: S) -> String {
        let s = format!(
            "{}{}{}",
            " ".repeat(self.padding_left),
            text,
            " ".repeat(self.padding_right)
        );
        if !crate::cli::style::colors_enabled() {
            return s;
        }

        // Apply colors and styles to the padded payload so background colors
        // cover the full component width.
        let mut styled = match self.fg {
            Some(Color::Red) => s.red().to_string(),
            Some(Color::Green) => s.green().to_string(),
            Some(Color::Blue) => s.blue().to_string(),
            Some(Color::Cyan) => s.cyan().to_string(),
            Some(Color::Yellow) => s.yellow().to_string(),
            Some(Color::Magenta) => s.magenta().to_string(),
            Some(Color::White) => s.white().to_string(),
            Some(Color::Black) => s.black().to_string(),
            Some(Color::Gray) => s.white().dimmed().to_string(), // Gray approximation
            None => s,
        };

        if let Some(bg) = self.bg {
            styled = match bg {
                Color::Red => styled.on_red().to_string(),
                Color::Green => styled.on_green().to_string(),
                Color::Blue => styled.on_blue().to_string(),
                Color::Cyan => styled.on_cyan().to_string(),
                Color::Yellow => styled.on_yellow().to_string(),
                Color::Magenta => styled.on_magenta().to_string(),
                Color::White => styled.on_white().to_string(),
                Color::Black | Color::Gray => styled.on_black().to_string(), // Fallback
            };
        }

        if self.is_bold {
            styled = styled.bold().to_string();
        }
        if self.is_italic {
            styled = styled.italic().to_string();
        }

        styled
    }
}

/// A professional instructional tip to guide the user.
pub fn print_tip(msg: &str) {
    let style = Style::new().foreground(Color::Gray).italic(true);
    let label_style = Style::new().foreground(Color::Gray).italic(true).bold(true);
    println!("\n  {} {}", label_style.render("Tip:"), style.render(msg));
}

/// Print a blank line for "airy" spacing (consistent 1-line margin).
pub fn print_spacer() {
    println!();
}

/// A list item with a "Charm-style" bullet.
pub fn print_list_item(item: &str, metadata: Option<&str>) {
    let bullet = Style::new().foreground(Color::Cyan).bold(true).render("•");
    if let Some(meta) = metadata {
        let meta_style = Style::new().foreground(Color::Gray);
        println!("  {} {} {}", bullet, item, meta_style.render(meta));
    } else {
        println!("  {bullet} {item}");
    }
}

/// A high-contrast contextual header.
pub fn print_header(context: &str, title: &str) {
    let ctx_style = Style::new()
        .background(Color::Cyan)
        .foreground(Color::Black)
        .bold(true)
        .padding_left(1)
        .padding_right(1);

    let title_style = Style::new().bold(true);

    println!(
        "\n{} {}",
        ctx_style.render(context),
        title_style.render(title)
    );
}

/// A standard success message with an icon.
pub fn print_success(msg: impl Display) {
    let icon_style = Style::new().foreground(Color::Green).bold(true);
    println!("  {} {}", icon_style.render("✓"), msg);
}

/// A standard warning message with an icon.
pub fn print_warning(msg: impl Display) {
    let icon_style = Style::new().foreground(Color::Yellow).bold(true);
    println!("  {} {}", icon_style.render("⚠"), msg);
}

/// Dry-run footer: confirms no mutations occurred.
pub fn print_dry_run_footer() {
    println!(
        "\n  {} No changes made (dry run)",
        crate::cli::style::dim("ℹ")
    );
}

fn format_kv_key(key: &str) -> String {
    let padded = format!("{key:>12}");
    Style::new().foreground(Color::Gray).render(padded)
}

/// Print a key-value pair with consistent formatting.
pub fn print_kv(key: &str, value: &str) {
    println!("  {}: {}", format_kv_key(key), value);
}

/// Default `omg info` rows. Extra metadata is a separate struct so verbose
/// output cannot leak into the compact view by accident.
pub struct InfoCore<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub source: &'a str,
    pub installed: bool,
    pub description: &'a str,
}

/// Fields shown only with global `-v` / `--verbose`.
pub struct InfoExtras<'a> {
    pub url: Option<&'a str>,
    pub size: Option<u64>,
    pub download: Option<u64>,
    pub licenses: &'a [String],
    pub depends: &'a [String],
    pub maintainer: Option<&'a str>,
    pub votes: Option<i32>,
    pub popularity: Option<f64>,
    pub out_of_date: bool,
}

impl InfoExtras<'static> {
    #[must_use]
    pub fn none() -> Self {
        Self {
            url: None,
            size: None,
            download: None,
            licenses: &[],
            depends: &[],
            maintainer: None,
            votes: None,
            popularity: None,
            out_of_date: false,
        }
    }
}

/// Compact Charm-style package info. Extra rows require verbose mode.
pub fn print_package_info(core: &InfoCore<'_>, extras: &InfoExtras<'_>) {
    use crate::cli::{modern_ui, style};

    modern_ui::print_phase_header("", "Info", &style::sanitize_terminal_text(core.name));
    print_kv(
        "Name",
        &style::package(&style::sanitize_terminal_text(core.name)),
    );
    print_kv(
        "Version",
        &style::version(&style::sanitize_terminal_text(core.version)),
    );
    print_kv("Source", core.source);
    print_kv("Installed", if core.installed { "yes" } else { "no" });
    print_kv(
        "Description",
        &style::sanitize_terminal_text(core.description),
    );
    if extras.out_of_date {
        print_kv("Status", &style::error("OUT OF DATE"));
    }
    if !modern_ui::is_verbose() {
        return;
    }
    if let Some(url) = extras.url.filter(|url| !url.is_empty()) {
        print_kv("URL", &style::url(&style::sanitize_terminal_text(url)));
    }
    if let Some(size) = extras.size {
        print_kv("Size", &style::size(size));
    }
    if let Some(download) = extras.download {
        print_kv("Download", &style::size(download));
    }
    if !extras.licenses.is_empty() {
        print_kv(
            "License",
            &extras
                .licenses
                .iter()
                .map(|license| style::sanitize_terminal_text(license))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !extras.depends.is_empty() {
        print_kv(
            "Depends",
            &extras
                .depends
                .iter()
                .map(|depend| style::sanitize_terminal_text(depend))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(maintainer) = extras.maintainer {
        print_kv("Maintainer", &style::sanitize_terminal_text(maintainer));
    }
    if let Some(votes) = extras.votes {
        print_kv("Votes", &votes.to_string());
    }
    if let Some(popularity) = extras.popularity {
        print_kv("Popularity", &format!("{popularity:.2}%"));
    }
}

/// Get a themed `ColorfulTheme` for dialoguer prompts.
/// Keeps using console/dialoguer themes as they are specific to that library.
pub fn prompt_theme() -> dialoguer::theme::ColorfulTheme {
    use dialoguer::theme::ColorfulTheme;
    let accent = crate::cli::chrome::palette().accent;
    ColorfulTheme {
        defaults_style: console::Style::new().dim(),
        prompt_style: console::Style::new().bold(),
        prompt_prefix: console::style("  ?".to_string())
            .true_color(accent.r, accent.g, accent.b)
            .bold(),
        success_prefix: console::style("  ✓".to_string()).green().bold(),
        active_item_style: console::Style::new()
            .true_color(accent.r, accent.g, accent.b)
            .bold(),
        active_item_prefix: console::style("  ❯".to_string())
            .true_color(accent.r, accent.g, accent.b)
            .bold(),
        inactive_item_prefix: console::style("   ".to_string()),
        ..ColorfulTheme::default()
    }
}

/// Wrap a block of text in a "Charm-style" bordered card.
pub fn print_card(title: &str, content: Vec<String>) {
    use comfy_table::Table;
    use comfy_table::modifiers::UTF8_ROUND_CORNERS;
    use comfy_table::presets::UTF8_FULL;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Style::new().bold(true).render(title)]);

    for line in content {
        table.add_row(vec![line]);
    }

    println!("\n{table}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colored_background_covers_component_padding() {
        temp_env::with_vars(
            [("NO_COLOR", None::<&str>), ("OMG_COLORS", Some("always"))],
            || {
                let rendered = Style::new()
                    .background(Color::Cyan)
                    .padding_left(1)
                    .padding_right(1)
                    .render("ok");
                assert!(rendered.starts_with('\u{1b}'), "{rendered:?}");
                assert!(rendered.contains(" ok "), "{rendered:?}");
                assert!(!rendered.ends_with(' '), "{rendered:?}");
            },
        );
    }

    #[test]
    fn colored_key_value_labels_keep_plain_text_alignment() {
        temp_env::with_vars(
            [("NO_COLOR", None::<&str>), ("OMG_COLORS", Some("always"))],
            || {
                let rendered = format_kv_key("Name");
                assert!(rendered.contains("        Name"), "{rendered:?}");
            },
        );
    }

    #[test]
    fn styles_are_plain_when_no_color_is_set() {
        temp_env::with_var("NO_COLOR", Some("1"), || {
            let rendered = Style::new()
                .foreground(Color::Green)
                .bold(true)
                .padding_left(1)
                .padding_right(1)
                .render("ok");
            assert_eq!(rendered, " ok ");
            assert!(!rendered.contains('\u{1b}'));
        });
    }
}
