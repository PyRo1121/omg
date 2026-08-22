//! Bubble Tea-inspired Elm Architecture for CLI commands
//!
//! This module implements the Elm Architecture pattern (Model → Update → View)
//! for building world-class CLI experiences in Rust.
//!
//! ## The Architecture
//!
//! ```text
//!     ┌───────────────────────────────────────┐
//!     │             User Input                │
//!     └─────────────────┬─────────────────────┘
//!                       │
//!                       ▼
//!     ┌───────────────────────────────────────┐
//!     │          Update (Msg → Model)          │
//!     └─────────────────┬─────────────────────┘
//!                       │
//!                       ▼
//!     ┌───────────────────────────────────────┐
//!     │               Model                   │
//!     └─────────────────┬─────────────────────┘
//!                       │
//!                       ▼
//!     ┌───────────────────────────────────────┐
//!     │          View (Model → String)         │
//!     └─────────────────┬─────────────────────┘
//!                       │
//!                       ▼
//!     ┌───────────────────────────────────────┐
//!     │           Terminal Output             │
//!     └───────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use omg_lib::cli::tea::{Program, Model, Msg, Cmd};
//!
//! struct MyModel {
//!     count: usize,
//! }
//!
//! #[derive(Debug)]
//! enum MyMsg {
//!     Increment,
//!     Decrement,
//! }
//!
//! impl Model for MyModel {
//!     type Msg = MyMsg;
//!
//!     fn init(&self) -> Cmd<MyMsg> {
//!         Cmd::none()
//!     }
//!
//!     fn update(&mut self, msg: MyMsg) -> Cmd<MyMsg> {
//!         match msg {
//!             MyMsg::Increment => self.count += 1,
//!             MyMsg::Decrement => self.count = self.count.saturating_sub(1),
//!         }
//!         Cmd::none()
//!     }
//!
//!     fn view(&self) -> String {
//!         format!("Count: {}", self.count)
//!     }
//! }
//!
//! // Run the program
//! let model = MyModel { count: 0 };
//! Program::new(model).run();
//! ```

mod async_bridge;
mod cmd;
mod renderer;

// Model implementations
mod info_model;
mod search_model;
mod status_model;
mod update_model;
mod wrappers;

pub use cmd::Cmd;
pub use renderer::Renderer;

// Re-export configuration types for convenience
pub use cmd::{
    BorderStyle, PanelConfig, ProgressConfig, ProgressStyle, SpinnerConfig, SpinnerStyle,
    StyledTextConfig, TableAlignment, TableConfig, TextStyle,
};

// Re-export models
pub use info_model::{InfoModel, InfoMsg, InfoSource};
pub use search_model::{PackageSource, SearchModel, SearchMsg, SearchResult, SearchState};
pub use status_model::{StatusData, StatusModel, StatusMsg};
pub use update_model::{UpdateModel, UpdateMsg, UpdatePackage, UpdateState, UpdateType};

// Re-export wrappers for easy integration
pub use wrappers::{run_info_elm, run_status_elm};

use std::fmt;
use std::io;

/// The core Model trait - implements the Elm Architecture
///
/// Your application state should implement this trait to define
/// how it responds to messages and renders itself.
pub trait Model: Sized {
    /// The message type for this model
    type Msg: Msg;

    /// Initialize the model - return an optional command to run
    #[must_use]
    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::none()
    }

    /// Update the model in response to a message
    ///
    /// Returns an optional command to run after updating
    #[must_use]
    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg>;

    /// Render the model to a string for display
    fn view(&self) -> String;

    /// Optional subscription for continuous events (e.g., timers, file watchers)
    #[must_use]
    fn subscription(&self) -> Cmd<Self::Msg> {
        Cmd::none()
    }
}

/// Message trait - all messages must implement this
///
/// This is a marker trait to ensure type safety and enable
/// downcasting in the future if needed.
pub trait Msg: Send + fmt::Debug + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Msg for T where T: Send + fmt::Debug + 'static {}

/// A Bubble Tea-inspired Program
///
/// Programs run a Model through the Elm Architecture lifecycle,
/// handling initialization, updates, and rendering.
pub struct Program<M: Model> {
    model: M,
    renderer: Renderer,
}

impl<M: Model> Program<M> {
    /// Create a new Program with the given Model
    #[must_use]
    pub fn new(model: M) -> Self {
        Self {
            model,
            renderer: Renderer::new(),
        }
    }

    /// Run the program to completion
    ///
    /// This will:
    /// 1. Call `init()` on the model
    /// 2. Execute any initial commands
    /// 3. Process all messages until completion
    /// 4. Render the final view
    pub fn run(mut self) -> io::Result<()> {
        // Initialize
        let init_cmd = self.model.init();

        // Render initial view to show loading state
        self.render()?;
        self.renderer.flush()?;

        self.process_cmd(init_cmd)?;

        // Process subscriptions
        let sub_cmd = self.model.subscription();
        self.process_cmd(sub_cmd)?;

        // Render final view
        self.render()?;

        Ok(())
    }

    /// Get a mutable reference to the model
    pub fn model(&mut self) -> &mut M {
        &mut self.model
    }

    /// Process a single command
    fn process_cmd(&mut self, cmd: Cmd<M::Msg>) -> io::Result<()> {
        match cmd {
            Cmd::None => {}
            Cmd::Msg(msg) => {
                let next_cmd = self.model.update(msg);
                self.process_cmd(next_cmd)?;
            }
            Cmd::Batch(cmds) => {
                for cmd in cmds {
                    self.process_cmd(cmd)?;
                }
            }
            Cmd::Exec(f) => {
                let msg = f();
                let next_cmd = self.model.update(msg);
                self.process_cmd(next_cmd)?;
            }
            Cmd::Print(output) => {
                self.renderer.print(&output)?;
            }
            Cmd::PrintLn(output) => {
                self.renderer.println(&output)?;
            }
            Cmd::Info(msg) => {
                self.renderer.info(&msg)?;
            }
            Cmd::Success(msg) => {
                self.renderer.success(&msg)?;
            }
            Cmd::Warning(msg) => {
                self.renderer.warning(&msg)?;
            }
            Cmd::Error(msg) => {
                self.renderer.error(&msg)?;
                return Err(io::Error::other(msg));
            }
            Cmd::Header(title, body) => {
                self.renderer.header(&title, &body)?;
            }
            Cmd::Card(title, content) => {
                self.renderer.card(&title, &content)?;
            }
            Cmd::Progress(_) | Cmd::Spinner(_) => {
                // NOT SUPPORTED by this synchronous renderer: progress bars and
                // spinners require an event loop this runtime does not have.
                // Models must use `Cmd::Info`/`Cmd::Card` for progress feedback.
                tracing::debug!("Progress/Spinner command ignored by synchronous renderer");
            }
            Cmd::Table(config) => {
                // No table renderer exists; emit rows as plain lines so the
                // content is never silently swallowed.
                self.renderer.header(&config.headers.join(" | "), "")?;
                for row in &config.rows {
                    self.renderer.println(&row.join(" | "))?;
                }
            }
            Cmd::StyledText(config) => {
                // No style renderer in this path; print the text so it is not
                // silently dropped (matches the fallback renderer behavior).
                self.renderer.println(&config.text)?;
            }
            Cmd::Panel(config) => {
                if let Some(title) = &config.title {
                    self.renderer.println(title)?;
                }
                let pad = " ".repeat(config.padding);
                for line in &config.content {
                    self.renderer.println(&format!("{pad}{line}"))?;
                }
            }
            Cmd::Spacer => {
                self.renderer.println("")?;
            }
        }
        Ok(())
    }

    /// Render the current view
    fn render(&mut self) -> io::Result<()> {
        let view = self.model.view();
        if view.trim().is_empty() {
            Ok(())
        } else {
            self.renderer.render(&view)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default)]
    struct CounterModel {
        count: usize,
    }

    #[derive(Debug, Clone)]
    enum CounterMsg {
        Increment,
        Decrement,
        Double,
    }

    impl Model for CounterModel {
        type Msg = CounterMsg;

        fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
            match msg {
                CounterMsg::Increment => {
                    self.count += 1;
                    if self.count.is_multiple_of(5) {
                        Cmd::info(format!("Reached {}!", self.count))
                    } else {
                        Cmd::none()
                    }
                }
                CounterMsg::Decrement => {
                    self.count = self.count.saturating_sub(1);
                    Cmd::none()
                }
                CounterMsg::Double => {
                    self.count *= 2;
                    Cmd::batch([
                        Cmd::info(format!("Doubled to {}", self.count)),
                        Cmd::success("Doubling complete!".to_string()),
                    ])
                }
            }
        }

        fn view(&self) -> String {
            format!("Current count: {}", self.count)
        }
    }

    #[test]
    fn test_counter_increment() {
        let mut model = CounterModel::default();
        assert_eq!(model.count, 0);

        let cmd = model.update(CounterMsg::Increment);
        assert_eq!(model.count, 1);
        assert!(matches!(cmd, Cmd::None));

        let cmd = model.update(CounterMsg::Increment);
        assert_eq!(model.count, 2);
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn test_counter_info_at_milestone() {
        let mut model = CounterModel { count: 4 };
        let cmd = model.update(CounterMsg::Increment);
        assert_eq!(model.count, 5);
        // Should emit info command at milestone
        assert!(matches!(cmd, Cmd::Info(_)));
    }

    #[test]
    fn test_counter_double() {
        let mut model = CounterModel { count: 3 };
        let cmd = model.update(CounterMsg::Double);
        assert_eq!(model.count, 6);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_counter_decrement() {
        let mut model = CounterModel { count: 1 };
        let cmd = model.update(CounterMsg::Decrement);
        assert_eq!(model.count, 0);
        assert!(matches!(cmd, Cmd::None));

        // Should saturate at 0
        let cmd = model.update(CounterMsg::Decrement);
        assert_eq!(model.count, 0);
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn test_view() {
        let model = CounterModel { count: 42 };
        assert_eq!(model.view(), "Current count: 42");
    }

    struct ErrorModel;

    impl Model for ErrorModel {
        type Msg = ();

        fn init(&self) -> Cmd<Self::Msg> {
            Cmd::error("package not found")
        }

        fn update(&mut self, (): Self::Msg) -> Cmd<Self::Msg> {
            Cmd::none()
        }

        fn view(&self) -> String {
            String::new()
        }
    }

    #[test]
    fn program_run_returns_err_on_cmd_error() {
        let err = Program::new(ErrorModel)
            .run()
            .expect_err("Cmd::Error must fail the program so the CLI exits non-zero");
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert!(
            err.to_string().contains("package not found"),
            "original command error must be preserved, got: {err}"
        );
    }
}
