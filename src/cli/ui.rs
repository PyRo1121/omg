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
        let s = text.to_string();

        // Apply colors and styles
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

        // Apply padding
        let left_pad = " ".repeat(self.padding_left);
        let right_pad = " ".repeat(self.padding_right);

        format!("{left_pad}{styled}{right_pad}")
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

/// A standard success message with a world-class icon.
pub fn print_success(msg: impl Display) {
    let icon_style = Style::new().foreground(Color::Green).bold(true);
    println!("  {} {}", icon_style.render("✓"), msg);
}

/// A standard error message with a world-class icon.
pub fn print_error(msg: impl Display) {
    let icon_style = Style::new().foreground(Color::Red).bold(true);
    println!("  {} {}", icon_style.render("✗"), msg);
}

/// A standard warning message with a world-class icon.
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

/// Print a step in a multi-step process.
pub fn print_step(step: usize, total: usize, msg: &str) {
    let step_str = format!(" {step:02}/{total:02} ");
    let step_style = Style::new()
        .background(Color::Gray) // approximate for bright black
        .foreground(Color::White);
    let msg_style = Style::new().foreground(Color::Gray);

    println!("{} {}", step_style.render(&step_str), msg_style.render(msg));
}

/// Print a key-value pair with consistent formatting.
pub fn print_kv(key: &str, value: &str) {
    let key_style = Style::new().foreground(Color::Gray);
    println!("  {:>12}: {}", key_style.render(key), value);
}

/// Get a themed `ColorfulTheme` for dialoguer prompts.
/// Keeps using console/dialoguer themes as they are specific to that library.
pub fn prompt_theme() -> dialoguer::theme::ColorfulTheme {
    use dialoguer::theme::ColorfulTheme;
    ColorfulTheme {
        defaults_style: console::Style::new().dim(),
        prompt_style: console::Style::new().bold(),
        prompt_prefix: console::style("  ?".to_string()).cyan().bold(),
        success_prefix: console::style("  ✓".to_string()).green().bold(),
        active_item_style: console::Style::new().cyan().bold(),
        active_item_prefix: console::style("  ❯".to_string()).cyan().bold(),
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
