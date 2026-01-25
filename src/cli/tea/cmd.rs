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
