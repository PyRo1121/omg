//! Renderer for Bubble Tea-style CLI output
//!
//! Handles styled output using the existing UI primitives.

use crate::cli::ui;
use owo_colors::OwoColorize;
use std::io::{self, BufWriter, Write};

/// Renderer for CLI output with styling
///
/// The renderer handles all output operations using the existing
/// UI primitives for consistent styling across the application.
pub struct Renderer<W: Write = BufWriter<io::Stdout>> {
    writer: W,
    no_color: bool,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer<BufWriter<io::Stdout>> {
    /// Create a new renderer writing to stdout
    #[must_use]
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
            no_color: std::env::var("NO_COLOR").is_ok() || std::env::var("OMG_NO_COLOR").is_ok(),
        }
    }
}

impl<W: Write> Renderer<W> {
    /// Create a renderer with a custom writer
    #[must_use]
    pub fn with_writer(writer: W) -> Self {
        Self {
            writer,
            no_color: std::env::var("NO_COLOR").is_ok() || std::env::var("OMG_NO_COLOR").is_ok(),
        }
    }

    /// Set whether to disable colors
    pub fn set_no_color(&mut self, no_color: bool) {
        self.no_color = no_color;
    }

    /// Flush the buffer
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Print raw text (no styling, no newline)
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        write!(self.writer, "{text}")
    }

    /// Print text with newline
    pub fn println(&mut self, text: &str) -> io::Result<()> {
        writeln!(self.writer, "{text}")
    }

    /// Render a view (full output string)
    pub fn render(&mut self, view: &str) -> io::Result<()> {
        self.println(view)?;
        self.flush()
    }

    /// Print an info message
    pub fn info(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ℹ {msg}")
        } else {
            writeln!(self.writer, "  {} {}", "ℹ".blue().bold(), msg)
        }
    }

    /// Print a success message
    pub fn success(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ✓ {msg}")
        } else {
            writeln!(self.writer, "  {} {}", "✓".green().bold(), msg)
        }
    }

    /// Print a warning message
    pub fn warning(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ⚠ {msg}")
        } else {
            writeln!(self.writer, "  {} {}", "⚠".yellow().bold(), msg)
        }
    }

    /// Print an error message
    pub fn error(&mut self, msg: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "  ✗ {msg}")
        } else {
            writeln!(self.writer, "  {} {}", "✗".red().bold(), msg)
        }
    }

    /// Print a styled header
    pub fn header(&mut self, title: &str, body: &str) -> io::Result<()> {
        if self.no_color {
            writeln!(self.writer, "\n[{title}] {body}")
        } else {
            writeln!(
                self.writer,
                "\n{} {}",
                ui::Style::new()
                    .background(ui::Color::Cyan)
                    .foreground(ui::Color::Black)
                    .bold(true)
                    .padding_left(1)
                    .padding_right(1)
                    .render(title),
                ui::Style::new().bold(true).render(body)
            )
        }
    }

    /// Print a styled card with content
    pub fn card(&mut self, title: &str, content: &[String]) -> io::Result<()> {
        ui::print_card(title, content.to_vec());
        Ok(())
    }

    /// Print a spacer (blank line)
    pub fn spacer(&mut self) -> io::Result<()> {
        writeln!(self.writer)
    }

    /// Print a step in a process
    pub fn step(&mut self, step: usize, total: usize, msg: &str) -> io::Result<()> {
        ui::print_step(step, total, msg);
        Ok(())
    }

    /// Print a key-value pair
    pub fn kv(&mut self, key: &str, value: &str) -> io::Result<()> {
        ui::print_kv(key, value);
        Ok(())
    }

    /// Print a list item
    pub fn list_item(&mut self, item: &str, metadata: Option<&str>) -> io::Result<()> {
        ui::print_list_item(item, metadata);
        Ok(())
    }

    /// Print a tip
    pub fn tip(&mut self, msg: &str) -> io::Result<()> {
        ui::print_tip(msg);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_renderer_print() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.print("hello").unwrap();
        renderer.print(" world").unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_renderer_println() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.println("hello").unwrap();
        renderer.println("world").unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert_eq!(output, "hello\nworld\n");
    }

    #[test]
    fn test_renderer_flush() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.print("test").unwrap();
        renderer.flush().unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert_eq!(output, "test");
    }

    #[test]
    fn test_render_view() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.render("Current count: 42").unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert_eq!(output, "Current count: 42\n");
    }

    #[test]
    fn test_info_message() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.info("Processing...").unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert!(output.contains("Processing"));
    }

    #[test]
    fn test_success_message() {
        let mut cursor = Cursor::new(Vec::new());
        let mut renderer = Renderer::with_writer(&mut cursor);

        renderer.success("Done!").unwrap();

        let output = String::from_utf8(cursor.into_inner()).unwrap();
        assert!(output.contains("Done"));
    }
}
