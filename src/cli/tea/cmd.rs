//! Commands for side effects in the Elm Architecture
//!
//! In Bubble Tea's Elm Architecture, Commands represent side effects
//! like I/O operations, timers, or async work. They're returned from
//! `update()` and processed by the runtime.

use std::fmt;

/// A Command represents a side effect to execute
///
/// Commands are returned from `Model::update()` to trigger I/O,
/// output, or other side effects without breaking the pure functional
/// update cycle.
pub enum Cmd<M> {
    /// No operation - return this when there's no side effect
    None,

    /// Send a message back to the model
    Msg(M),

    /// Execute multiple commands in sequence
    Batch(Vec<Cmd<M>>),

    /// Execute a function that produces a message
    Exec(Box<dyn FnOnce() -> M>),

    /// Print raw output (no formatting)
    Print(String),

    /// Print output with newline
    PrintLn(String),

    /// Print an info message (styled)
    Info(String),

    /// Print a success message (styled)
    Success(String),

    /// Print a warning message (styled)
    Warning(String),

    /// Print an error message (styled)
    Error(String),

    /// Print a styled header
    Header(String, String),

    /// Print a styled card with content
    Card(String, Vec<String>),

    /// Create or update a progress bar
    Progress(ProgressConfig),

    /// Create a temporary spinner
    Spinner(SpinnerConfig),

    /// Render a styled table
    Table(TableConfig),

    /// Render styled text with lip-gloss styles
    StyledText(StyledTextConfig),

    /// Render a bordered panel/box
    Panel(PanelConfig),

    /// Print a blank line (spacer)
    Spacer,
}

impl<M> fmt::Debug for Cmd<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "Cmd::None"),
            Self::Msg(_) => write!(f, "Cmd::Msg(...)"),
            Self::Batch(batch) => f.debug_tuple("Batch").field(&batch.len()).finish(),
            Self::Exec(_) => write!(f, "Cmd::Exec(...)"),
            Self::Print(s) => f.debug_tuple("Print").field(&truncate(s, 20)).finish(),
            Self::PrintLn(s) => f.debug_tuple("PrintLn").field(&truncate(s, 20)).finish(),
            Self::Info(s) => f.debug_tuple("Info").field(&truncate(s, 20)).finish(),
            Self::Success(s) => f.debug_tuple("Success").field(&truncate(s, 20)).finish(),
            Self::Warning(s) => f.debug_tuple("Warning").field(&truncate(s, 20)).finish(),
            Self::Error(s) => f.debug_tuple("Error").field(&truncate(s, 20)).finish(),
            Self::Header(t, _) => f.debug_tuple("Header").field(t).finish(),
            Self::Card(t, _) => f.debug_tuple("Card").field(t).finish(),
            Self::Progress(_) => write!(f, "Cmd::Progress(...)"),
            Self::Spinner(_) => write!(f, "Cmd::Spinner(...)"),
            Self::Table(_) => write!(f, "Cmd::Table(...)"),
            Self::StyledText(_) => write!(f, "Cmd::StyledText(...)"),
            Self::Panel(_) => write!(f, "Cmd::Panel(...)"),
            Self::Spacer => write!(f, "Cmd::Spacer"),
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Configuration for progress bars
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// Unique identifier for this progress bar
    pub id: String,
    /// Message to display
    pub message: String,
    /// Current progress (0-100)
    pub percent: usize,
    /// Total length if known
    pub length: Option<usize>,
    /// Style of the progress bar
    pub style: ProgressStyle,
}

/// Progress bar visual styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStyle {
    /// Default progress bar
    Default,
    /// Download style with speed indicator
    Download,
    /// Install style with package count
    Install,
    /// Spinner only (no bar)
    Spinner,
}

/// Configuration for spinners
#[derive(Debug, Clone)]
pub struct SpinnerConfig {
    /// Unique identifier
    pub id: String,
    /// Message to display
    pub message: String,
    /// Spinner style
    pub style: SpinnerStyle,
}

/// Spinner visual styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// Dots spinning
    Dots,
    /// Arrows
    Arrows,
    /// Simple pipe rotation
    Pipe,
    /// Moon phases
    Moon,
}

/// Configuration for tables
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Table headers
    pub headers: Vec<String>,
    /// Table rows (each row is a vector of cells)
    pub rows: Vec<Vec<String>>,
    /// Column alignments
    pub alignments: Vec<TableAlignment>,
    /// Border style
    pub border_style: BorderStyle,
}

/// Table column alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

/// Border styles for tables and panels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// No border
    None,
    /// Light single-line border
    Light,
    /// Heavy double-line border
    Heavy,
    /// Rounded corners
    Rounded,
    /// Double lines
    Double,
}

/// Configuration for styled text
#[derive(Debug, Clone)]
pub struct StyledTextConfig {
    /// The text content
    pub text: String,
    /// Text style
    pub style: TextStyle,
}

/// Text styling options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// Plain text
    Plain,
    /// Bold text
    Bold,
    /// Dimmed/faint text
    Dim,
    /// Italic text
    Italic,
    /// Underlined text
    Underline,
    /// Primary color (accent)
    Primary,
    /// Success color (green)
    Success,
    /// Warning color (yellow)
    Warning,
    /// Error color (red)
    Error,
    /// Info color (blue)
    Info,
    /// Muted color (gray)
    Muted,
}

/// Configuration for bordered panels
#[derive(Debug, Clone)]
pub struct PanelConfig {
    /// Panel title (optional)
    pub title: Option<String>,
    /// Panel content
    pub content: Vec<String>,
    /// Border style
    pub border_style: BorderStyle,
    /// Padding inside the border
    pub padding: usize,
}

impl<M> Cmd<M> {
    /// Create a no-op command
    #[must_use]
    pub const fn none() -> Self {
        Self::None
    }

    /// Create a message command
    #[must_use]
    pub fn msg(msg: M) -> Self {
        Self::Msg(msg)
    }

    /// Batch multiple commands together
    #[must_use]
    pub fn batch(cmds: impl IntoIterator<Item = Cmd<M>>) -> Self {
        Self::Batch(cmds.into_iter().collect())
    }

    /// Execute a function that returns a message
    #[must_use]
    pub fn exec<F>(f: F) -> Self
    where
        F: FnOnce() -> M + 'static,
    {
        Self::Exec(Box::new(f))
    }

    /// Print raw output
    #[must_use]
    pub fn print(s: impl Into<String>) -> Self {
        Self::Print(s.into())
    }

    /// Print with newline
    #[must_use]
    pub fn println(s: impl Into<String>) -> Self {
        Self::PrintLn(s.into())
    }

    /// Print an info message
    #[must_use]
    pub fn info(s: impl Into<String>) -> Self {
        Self::Info(s.into())
    }

    /// Print a success message
    #[must_use]
    pub fn success(s: impl Into<String>) -> Self {
        Self::Success(s.into())
    }

    /// Print a warning message
    #[must_use]
    pub fn warning(s: impl Into<String>) -> Self {
        Self::Warning(s.into())
    }

    /// Print an error message
    #[must_use]
    pub fn error(s: impl Into<String>) -> Self {
        Self::Error(s.into())
    }

    /// Print a styled header
    #[must_use]
    pub fn header(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::Header(title.into(), body.into())
    }

    /// Print a styled card with content
    #[must_use]
    pub fn card(title: impl Into<String>, content: Vec<String>) -> Self {
        Self::Card(title.into(), content)
    }

    /// Create or update a progress bar
    #[must_use]
    pub fn progress(config: ProgressConfig) -> Self {
        Self::Progress(config)
    }

    /// Create a simple progress bar
    #[must_use]
    pub fn simple_progress(id: impl Into<String>, message: impl Into<String>, percent: usize) -> Self {
        Self::Progress(ProgressConfig {
            id: id.into(),
            message: message.into(),
            percent,
            length: None,
            style: ProgressStyle::Default,
        })
    }

    /// Create a temporary spinner
    #[must_use]
    pub fn spinner(config: SpinnerConfig) -> Self {
        Self::Spinner(config)
    }

    /// Create a simple spinner
    #[must_use]
    pub fn simple_spinner(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Spinner(SpinnerConfig {
            id: id.into(),
            message: message.into(),
            style: SpinnerStyle::Dots,
        })
    }

    /// Render a styled table
    #[must_use]
    pub fn table(config: TableConfig) -> Self {
        Self::Table(config)
    }

    /// Render styled text
    #[must_use]
    pub fn styled_text(config: StyledTextConfig) -> Self {
        Self::StyledText(config)
    }

    /// Render simple bold text
    #[must_use]
    pub fn bold(text: impl Into<String>) -> Self {
        Self::StyledText(StyledTextConfig {
            text: text.into(),
            style: TextStyle::Bold,
        })
    }

    /// Render a bordered panel
    #[must_use]
    pub fn panel(config: PanelConfig) -> Self {
        Self::Panel(config)
    }

    /// Print a blank line (spacer)
    #[must_use]
    pub fn spacer() -> Self {
        Self::Spacer
    }
}

/// Helper function to create commands
///
/// This allows ergonomic `cmd(Cmd::none())` syntax
/// or `cmd(msg)` to convert a message to a command.
pub fn cmd<M>(c: Cmd<M>) -> Cmd<M> {
    c
}

/// Convert a message directly to a command
impl<M> From<M> for Cmd<M> {
    fn from(msg: M) -> Self {
        Self::Msg(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_none() {
        let cmd: Cmd<()> = Cmd::none();
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn test_cmd_msg() {
        let cmd: Cmd<String> = Cmd::msg("hello".to_string());
        assert!(matches!(cmd, Cmd::Msg(_)));
    }

    #[test]
    fn test_cmd_from() {
        let cmd: Cmd<String> = "hello".to_string().into();
        assert!(matches!(cmd, Cmd::Msg(_)));
    }

    #[test]
    fn test_cmd_batch() {
        let cmd: Cmd<()> = Cmd::batch([Cmd::print("a"), Cmd::print("b"), Cmd::none()]);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_cmd_exec() {
        let cmd: Cmd<String> = Cmd::exec(|| "result".to_string());
        assert!(matches!(cmd, Cmd::Exec(_)));
    }

    #[test]
    fn test_cmd_print_variants() {
        let _: Cmd<()> = Cmd::print("test");
        let _: Cmd<()> = Cmd::println("test");
        let _: Cmd<()> = Cmd::info("test");
        let _: Cmd<()> = Cmd::success("test");
        let _: Cmd<()> = Cmd::warning("test");
        let _: Cmd<()> = Cmd::error("test");
    }

    #[test]
    fn test_cmd_header() {
        let cmd: Cmd<()> = Cmd::header("Title", "Body");
        assert!(matches!(cmd, Cmd::Header(_, _)));
    }

    #[test]
    fn test_cmd_card() {
        let cmd: Cmd<()> = Cmd::card("Title", vec!["line1".to_string(), "line2".to_string()]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }
}
