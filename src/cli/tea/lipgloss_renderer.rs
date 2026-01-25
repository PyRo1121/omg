//! Modern renderer using lipgloss for beautiful CLI output
//!
//! This renderer provides rich styling with borders, colors, and layout
//! primitives powered by the lipgloss library.

use super::cmd::{
    BorderStyle, PanelConfig, ProgressConfig, ProgressStyle, SpinnerConfig,
    SpinnerStyle, StyledTextConfig, TableAlignment, TableConfig, TextStyle,
};
use crate::cli::tea::Cmd;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle as IndiStyle};
use std::collections::HashMap;
use std::io::{self, BufWriter, Write};
use std::sync::{Arc, Mutex};

// Import lipgloss items correctly for v0.1
use lipgloss::{
    normal_border, rounded_border, heavy_border, double_border,
    align_horizontal::{Center, Left, Right},
    Style, Text,
};

/// Modern renderer with lipgloss styling
pub struct LipGlossRenderer<W: Write = BufWriter<io::Stdout>> {
    writer: W,
    no_color: bool,
    terminal_size: (u16, u16),
    progress_bars: Arc<Mutex<HashMap<String, ProgressBar>>>,
    theme: Theme,
}

/// Color theme for the CLI (stores color as string for lipgloss)
#[derive(Debug, Clone)]
pub struct Theme {
    /// Primary accent color (ANSI code or hex)
    pub primary: String,
    /// Success color
    pub success: String,
    /// Warning color
    pub warning: String,
    /// Error color
    pub error: String,
    /// Info color
    pub info: String,
    /// Muted/subtle color
    pub muted: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme (default)
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            primary: "86".to_string(),      // Cyan
            success: "142".to_string(),     // Green
            warning: "226".to_string(),     // Yellow
            error: "167".to_string(),       // Red
            info: "69".to_string(),         // Blue
            muted: "245".to_string(),       // Gray
        }
    }

    /// Light theme
    #[must_use]
    pub const fn light() -> Self {
        Self {
            primary: "27".to_string(),      // Blue
            success: "28".to_string(),      // Green
            warning: "208".to_string(),     // Yellow/Orange
            error: "124".to_string(),       // Dark red
            info: "39".to_string(),         // Cyan
            muted: "243".to_string(),       // Dark gray
        }
    }
}

impl Default for LipGlossRenderer<BufWriter<io::Stdout>> {
    fn default() -> Self {
        Self::new()
    }
}

impl LipGlossRenderer<BufWriter<io::Stdout>> {
    /// Create a new renderer with auto-detected terminal size
    #[must_use]
    pub fn new() -> Self {
        let (width, height) = terminal_size::terminal_size()
            .map(|(w, h)| (w.0, h.1))
            .unwrap_or((80, 24));

        Self {
            writer: BufWriter::new(io::stdout()),
            no_color: std::env::var("NO_COLOR").is_ok()
                || std::env::var("OMG_NO_COLOR").is_ok(),
            terminal_size: (width, height),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
            theme: Theme::default(),
        }
    }

    /// Create with a custom theme
    #[must_use]
    pub fn with_theme(theme: Theme) -> Self {
        let (width, height) = terminal_size::terminal_size()
            .map(|(w, h)| (w.0, h.1))
            .unwrap_or((80, 24));

        Self {
            writer: BufWriter::new(io::stdout()),
            no_color: std::env::var("NO_COLOR").is_ok()
                || std::env::var("OMG_NO_COLOR").is_ok(),
            terminal_size: (width, height),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
            theme,
        }
    }
}

impl<W: Write> LipGlossRenderer<W> {
    /// Create with a custom writer
    #[must_use]
    pub fn with_writer(writer: W) -> Self {
        let (width, height) = terminal_size::terminal_size()
            .map(|(w, h)| (w.0, h.1))
            .unwrap_or((80, 24));

        Self {
            writer,
            no_color: std::env::var("NO_COLOR").is_ok()
                || std::env::var("OMG_NO_COLOR").is_ok(),
            terminal_size: (width, height),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
            theme: Theme::default(),
        }
    }

    /// Set the color theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Disable colors
    pub fn set_no_color(&mut self, no_color: bool) {
        self.no_color = no_color;
    }

    /// Flush the buffer
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Get terminal width
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.terminal_size.0
    }

    /// Get terminal height
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.terminal_size.1
    }

    /// Print raw text
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        write!(self.writer, "{text}")
    }

    /// Print with newline
    pub fn println(&mut self, text: &str) -> io::Result<()> {
        writeln!(self.writer, "{text}")
    }

    /// Render a full view
    pub fn render(&mut self, view: &str) -> io::Result<()> {
        self.println(view)?;
        self.flush()
    }

    /// Print an info message
    pub fn info(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ℹ {msg}")
        } else {
            writeln!(self.writer, "  ℹ {msg}")
        }
    }

    /// Print a success message
    pub fn success(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ✓ {msg}")
        } else {
            writeln!(self.writer, "  ✓ {msg}")
        }
    }

    /// Print a warning message
    pub fn warning(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ⚠ {msg}")
        } else {
            writeln!(self.writer, "  ⚠ {msg}")
        }
    }

    /// Print an error message
    pub fn error(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ✗ {msg}")
        } else {
            writeln!(self.writer, "  ✗ {msg}")
        }
    }

    /// Print a styled header
    pub fn header(&mut self, title: &str, body: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "\n[{title}] {body}")
        } else {
            let title_style = Style::new()
                .foreground(self.theme.primary.clone().into())
                .bold();

            let body_style = Style::new().bold();

            writeln!(
                self.writer,
                "\n{} {}",
                title_style.render(title),
                body_style.render(body)
            )
        }
    }

    /// Print a styled card with content
    pub fn card(&mut self, title: &str, content: &[String]) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "\n[{title}]")?;
            for line in content {
                writeln!(self.writer, "  {line}")?;
            }
            return Ok(());
        }

        let title_style = Style::new()
            .foreground(self.theme.primary.clone().into())
            .bold();

        let content_style = Style::new()
            .foreground(self.theme.muted.clone().into());

        // Build the card content
        let mut card_lines = vec![title_style.render(title).to_string()];
        for line in content {
            card_lines.push(content_style.render(line).to_string());
        }

        let card = Style::new()
            .border_style(rounded_border())
            .border_foreground(self.theme.primary.clone().into())
            .padding(1, 1, 1, 1)
            .render(card_lines.join("\n"));

        writeln!(self.writer, "\n{card}")
    }

    /// Render a progress bar
    pub fn progress(&mut self, config: &ProgressConfig) -> io::Result<()> {
        let mut bars = self.progress_bars.lock().unwrap();

        let bar = if let Some(existing) = bars.get(&config.id) {
            existing.clone()
        } else {
            // Determine progress style template
            let template = match config.style {
                ProgressStyle::Default => "{wide_bar} {pos}/{len} {msg}",
                ProgressStyle::Download => "{wide_bar} {bytes}/{total_bytes} ({eta}) {msg}",
                ProgressStyle::Install => "{wide_bar} {pos}/{len} packages {msg}",
                ProgressStyle::Spinner => "{spinner:.dim} {msg}",
            };

            let style = IndiStyle::with_template(template)
                .unwrap()
                .progress_chars("=> ");

            let pb = ProgressBar::new(config.length.unwrap_or(100) as u64);
            pb.set_style(style);
            pb.set_message(config.message.clone());
            pb.set_draw_target(ProgressDrawTarget::stderr());

            bars.insert(config.id.clone(), pb.clone());
            pb
        };

        bar.set_position(config.percent as u64);
        Ok(())
    }

    /// Render a spinner
    pub fn spinner(&mut self, config: &SpinnerConfig) -> io::Result<()> {
        let mut bars = self.progress_bars.lock().unwrap();

        if !bars.contains_key(&config.id) {
            let template = match config.style {
                SpinnerStyle::Dots => "{spinner:.dim} {msg}",
                SpinnerStyle::Arrows => "{spinner:.cyan} {msg}",
                SpinnerStyle::Pipe => "{spinner:.blue} {msg}",
                SpinnerStyle::Moon => "{spinner:.yellow} {msg}",
            };

            let style = IndiStyle::with_template(template)
                .unwrap()
                .tick_strings(&["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"]);

            let pb = ProgressBar::new_spinner();
            pb.set_style(style);
            pb.set_message(config.message.clone());
            pb.set_draw_target(ProgressDrawTarget::stderr());
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            bars.insert(config.id.clone(), pb);
        }

        Ok(())
    }

    /// Finish and remove a progress bar/spinner
    pub fn finish_progress(&mut self, id: &str) -> io::Result<()> {
        let mut bars = self.progress_bars.lock().unwrap();
        if let Some(bar) = bars.remove(id) {
            bar.finish_and_clear();
        }
        Ok(())
    }

    /// Render a styled table
    pub fn table(&mut self, config: &TableConfig) -> io::Result<()> {
        if self.no_color {
            // Simple fallback
            for (i, row) in config.rows.iter().enumerate() {
                let row_str = row.join(" | ");
                writeln!(self.writer, "{row_str}")?;
                if i == 0 && !config.headers.is_empty() {
                    let sep = "-".repeat(row_str.len());
                    writeln!(self.writer, "{sep}")?;
                }
            }
            return Ok(());
        }

        // Calculate column widths
        let mut col_widths = vec![0usize; config.headers.len()];
        for (i, header) in config.headers.iter().enumerate() {
            col_widths[i] = col_widths[i].max(header.len());
        }
        for row in &config.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }

        // Build border style
        let border_style = match config.border_style {
            BorderStyle::None => return Ok(()),
            BorderStyle::Light => normal_border(),
            BorderStyle::Heavy => heavy_border(),
            BorderStyle::Rounded => rounded_border(),
            BorderStyle::Double => double_border(),
        };

        // Render headers
        let header_style = Style::new()
            .foreground(self.theme.primary.clone().into())
            .bold();

        let header_cells: Vec<String> = config.headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let aligned = align_text(h, col_widths[i], config.alignments.get(i).copied());
                header_style.render(aligned).to_string()
            })
            .collect();

        writeln!(
            self.writer,
            "\n{}",
            Style::new()
                .border_style(border_style)
                .border_foreground(self.theme.muted.clone().into())
                .render(header_cells.join(" │ "))
        )?;

        // Render data rows
        let data_style = Style::new();
        for row in &config.rows {
            let row_cells: Vec<String> = row.iter()
                .enumerate()
                .filter(|(i, _)| *i < col_widths.len())
                .map(|(i, cell)| {
                    let aligned = align_text(cell, col_widths[i], config.alignments.get(i).copied());
                    data_style.render(aligned).to_string()
                })
                .collect();

            writeln!(self.writer, "{}", row_cells.join(" │ "))?;
        }

        Ok(())
    }

    /// Render styled text
    pub fn styled_text(&mut self, config: &StyledTextConfig) -> io::Result<()> {
        if self.no_color {
            return writeln!(self.writer, "{}", config.text);
        }

        let style = match config.style {
            TextStyle::Plain => Style::new(),
            TextStyle::Bold => Style::new().bold(),
            TextStyle::Dim => Style::new().dim(),
            TextStyle::Italic => Style::new().italic(),
            TextStyle::Underline => Style::new().underline(),
            TextStyle::Primary => Style::new().foreground(self.theme.primary.clone().into()),
            TextStyle::Success => Style::new().foreground(self.theme.success.clone().into()),
            TextStyle::Warning => Style::new().foreground(self.theme.warning.clone().into()),
            TextStyle::Error => Style::new().foreground(self.theme.error.clone().into()),
            TextStyle::Info => Style::new().foreground(self.theme.info.clone().into()),
            TextStyle::Muted => Style::new().foreground(self.theme.muted.clone().into()),
        };

        writeln!(self.writer, "{}", style.render(&config.text))
    }

    /// Render a bordered panel
    pub fn panel(&mut self, config: &PanelConfig) -> io::Result<()> {
        if self.no_color {
            if let Some(title) = &config.title {
                writeln!(self.writer, "\n[{title}]")?;
            }
            for line in &config.content {
                writeln!(self.writer, "{}{}", " ".repeat(config.padding), line)?;
            }
            return Ok(());
        }

        let border_style = match config.border_style {
            BorderStyle::None => {
                for line in &config.content {
                    writeln!(self.writer, "{line}")?;
                }
                return Ok(());
            }
            BorderStyle::Light => normal_border(),
            BorderStyle::Heavy => heavy_border(),
            BorderStyle::Rounded => rounded_border(),
            BorderStyle::Double => double_border(),
        };

        let title_style = Style::new()
            .foreground(self.theme.primary.clone().into())
            .bold();

        let content_style = Style::new();

        let mut lines = Vec::new();
        if let Some(title) = &config.title {
            lines.push(title_style.render(title).to_string());
        }
        for line in &config.content {
            lines.push(content_style.render(line).to_string());
        }

        let panel = Style::new()
            .border_style(border_style)
            .border_foreground(self.theme.muted.clone().into())
            .padding(config.padding, config.padding, config.padding, config.padding)
            .render(lines.join("\n"));

        writeln!(self.writer, "\n{panel}")
    }

    /// Print a spacer (blank line)
    pub fn spacer(&mut self) -> io::Result<()> {
        writeln!(self.writer)
    }

    /// Process a command with rendering
    pub fn process_cmd<M>(&mut self, cmd: &Cmd<M>) -> io::Result<()> {
        match cmd {
            Cmd::None => {}
            Cmd::Print(s) => self.print(s)?,
            Cmd::PrintLn(s) => self.println(s)?,
            Cmd::Info(s) => self.info(s)?,
            Cmd::Success(s) => self.success(s)?,
            Cmd::Warning(s) => self.warning(s)?,
            Cmd::Error(s) => self.error(s)?,
            Cmd::Header(t, b) => self.header(t, b)?,
            Cmd::Card(t, c) => self.card(t, c)?,
            Cmd::Progress(c) => self.progress(c)?,
            Cmd::Spinner(c) => self.spinner(c)?,
            Cmd::Table(c) => self.table(c)?,
            Cmd::StyledText(c) => self.styled_text(c)?,
            Cmd::Panel(c) => self.panel(c)?,
            Cmd::Msg(_) | Cmd::Batch(_) | Cmd::Exec(_) => {
                // These are handled by the Program runtime
            }
        }
        Ok(())
    }
}

/// Helper function to align text within a column
fn align_text(text: &str, width: usize, alignment: Option<TableAlignment>) -> String {
    match alignment {
        Some(TableAlignment::Center) => {
            let total_padding = width.saturating_sub(text.len());
            if total_padding == 0 {
                return text.to_string();
            }
            let left_pad = total_padding / 2;
            let right_pad = total_padding - left_pad;
            format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
        }
        Some(TableAlignment::Right) => format!("{text:>width$}"),
        _ => format!("{text:<width$}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme.primary, "86");
    }

    #[test]
    fn test_theme_light() {
        let theme = Theme::light();
        assert_eq!(theme.primary, "27");
    }

    #[test]
    fn test_align_text() {
        assert_eq!(align_text("test", 10, None), "test      ");
        assert_eq!(align_text("test", 10, Some(TableAlignment::Center)), "   test   ");
        assert_eq!(align_text("test", 10, Some(TableAlignment::Right)), "      test");
    }

    #[test]
    fn test_renderer_width() {
        let renderer = LipGlossRenderer::new();
        assert!(renderer.width() > 0);
    }
}
