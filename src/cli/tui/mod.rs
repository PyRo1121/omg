use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::future::Future;
use std::io;
use std::time::Duration;

pub mod app;
mod ui;

// Constants for TUI behavior
const POLL_TIMEOUT_MS: u64 = 100;
const REFRESH_INTERVAL_SECS: u64 = 5;
/// Minimum idle time after the last keystroke before a search is dispatched,
/// so typing does not fire one daemon/apt query per character.
const SEARCH_DEBOUNCE_MS: u64 = 250;

/// Outcome of a background action: `(label, Ok(summary-or-empty))`.
type ActionResult = (&'static str, anyhow::Result<String>);

fn should_quit(key: KeyEvent, search_mode: bool) -> bool {
    (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        || (key.code == KeyCode::Char('q') && key.modifiers.is_empty() && !search_mode)
}

fn should_dispatch_key(key: KeyEvent) -> bool {
    !key.modifiers.contains(KeyModifiers::CONTROL)
}

pub async fn run() -> Result<()> {
    let app = app::App::new().await?;
    run_tui_with_app(app).await
}

pub async fn run_with_tab(tab: app::Tab) -> Result<()> {
    let app = app::App::new().await?.with_tab(tab);
    run_tui_with_app(app).await
}

/// Centralized TUI setup and teardown to avoid code duplication
async fn run_tui_with_app(mut app: app::App) -> Result<()> {
    // Setup terminal. Every fallible step must restore previously-acquired
    // terminal state on failure, or the user's shell is left in raw mode.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
        disable_raw_mode()?;
        return Err(err.into());
    }

    let terminal = match Terminal::new(CrosstermBackend::new(stdout))
        .and_then(|mut terminal| terminal.hide_cursor().map(|()| terminal))
    {
        Ok(terminal) => terminal,
        Err(err) => {
            disable_raw_mode()?;
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
            return Err(err.into());
        }
    };
    let mut terminal = terminal;

    // Run the app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal - always execute cleanup even if app failed
    let cleanup_result = cleanup_terminal(&mut terminal);

    // Return the first error if any occurred
    res.and(cleanup_result)
}

/// Cleanup terminal state - extracted to ensure consistent cleanup
fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    terminal.clear()?;
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::App,
) -> Result<()> {
    let mut last_search = String::new();
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<ActionResult>();

    loop {
        // Apply completed background actions before drawing. Messages arrive
        // FIFO from a single in-flight action, so state updates stay ordered
        // and never race with the render pass.
        while let Ok((label, result)) = action_rx.try_recv() {
            app.action_in_flight = false;
            report_action_result(app, result, label);
        }

        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events with timeout for animations
        let was_search_mode = app.search_mode;
        if crossterm::event::poll(Duration::from_millis(POLL_TIMEOUT_MS))?
            && let Event::Key(key) = event::read()?
        {
            // Only process key press events, ignore release
            if key.kind == KeyEventKind::Press {
                if should_quit(key, app.search_mode) {
                    return Ok(());
                }
                // Control-modified characters must never fall through to a
                // destructive single-key action or become search text.
                if !should_dispatch_key(key) {
                    continue;
                }

                // While typing a query, global shortcuts must not fire AND
                // every key must reach App::handle_key's search-input block.
                // handle_special_key_actions is where normal keys are routed
                // to handle_key via its catch-all, so skipping it during a
                // query would freeze all input (audit sec2 F-09).
                if app.search_mode {
                    app.handle_key(key.code);
                } else {
                    handle_special_key_actions(app, key.code, &action_tx);
                }

                // A query committed with Enter before the debounce elapsed
                // must be fetched immediately so the shown results match the
                // final query. (Esc clears the query, so a cancelled search
                // can never satisfy this condition.)
                let committed = was_search_mode
                    && !app.search_mode
                    && !app.search_query.is_empty()
                    && app.search_query != last_search;
                if committed {
                    last_search.clone_from(&app.search_query);
                    run_search(app, &last_search).await;
                }
            }
        }

        // Debounce: once typing has paused, fetch even without another key
        // event. This must run on every loop iteration — inside the key-event
        // branch it would never fire after the final keystroke.
        if app.search_mode
            && app.search_query != last_search
            && app.last_query_change.elapsed() >= Duration::from_millis(SEARCH_DEBOUNCE_MS)
        {
            last_search.clone_from(&app.search_query);
            run_search(app, &last_search).await;
        }

        // Update app state
        app.tick().await?;
    }
}

async fn run_search(app: &mut app::App, query: &str) {
    if let Err(e) = app.search_packages(query).await {
        tracing::error!("Search failed: {e}");
        app.search_results.clear();
        app.search_error = Some(e.to_string());
    }
}

/// Spawn a long-running package operation on a background task.
///
/// Ordering guarantees: actions are serialized by `App::action_in_flight`
/// (at most one runs at a time), each sends exactly one completion message,
/// and the event loop drains those messages FIFO before the next draw. Model
/// state is therefore only ever mutated from the loop task itself.
fn spawn_action(
    app: &mut app::App,
    label: &'static str,
    action_tx: &tokio::sync::mpsc::UnboundedSender<ActionResult>,
    fut: impl Future<Output = anyhow::Result<String>> + Send + 'static,
) {
    if app.action_in_flight {
        app.action_error = Some("another action is already running".to_string());
        return;
    }
    app.action_in_flight = true;
    let sender = action_tx.clone();
    tokio::spawn(async move {
        let outcome = fut.await;
        let _ = sender.send((label, outcome));
    });
}

/// Handle special key actions that trigger async operations.
/// Long-running work is spawned off the UI task so the loop keeps drawing.
fn handle_special_key_actions(
    app: &mut app::App,
    key_code: KeyCode,
    action_tx: &tokio::sync::mpsc::UnboundedSender<ActionResult>,
) {
    match key_code {
        KeyCode::Char('u') if app.current_tab == app::Tab::Dashboard => {
            spawn_action(app, "update system", action_tx, async {
                app::App::update_system().await.map(|()| String::new())
            });
        }
        KeyCode::Char('c') if app.current_tab == app::Tab::Dashboard => {
            spawn_action(app, "clean cache", action_tx, async {
                app::App::clean_cache().await.map(|()| String::new())
            });
        }
        KeyCode::Char('o') if app.current_tab == app::Tab::Dashboard => {
            spawn_action(app, "remove orphans", action_tx, async {
                app::App::remove_orphans().await.map(|()| String::new())
            });
        }
        KeyCode::Char('a') if app.current_tab == app::Tab::Security => {
            spawn_action(app, "security audit", action_tx, async {
                let vulns = app::App::run_security_audit().await?;
                match vulns {
                    0 => tracing::info!("No vulnerabilities found!"),
                    found => tracing::warn!("Found {found} vulnerabilities"),
                }
                Ok(String::new())
            });
        }
        KeyCode::Enter
            if app.current_tab == app::Tab::Packages
                && app.show_popup
                && !app.search_results.is_empty() =>
        {
            // Confirmation popup accepted by a second Enter; clear it before
            // spawning so a slow install cannot stack popups.
            app.show_popup = false;
            if let Some(pkg) = app.search_results.get(app.selected_index) {
                let pkg_name = pkg.name.clone();
                spawn_action(app, "install", action_tx, async move {
                    app::App::install_package(&pkg_name)
                        .await
                        .map(|()| format!("installed {pkg_name}"))
                });
                force_refresh(app);
            }
        }
        _ => {
            // Normal key handling
            app.handle_key(key_code);
        }
    }
}

fn report_action_result(app: &mut app::App, result: anyhow::Result<String>, action: &str) {
    match result {
        Ok(summary) => {
            app.action_error = None;
            if !summary.is_empty() {
                tracing::info!("{action}: {summary}");
            }
        }
        Err(e) => {
            tracing::error!("Failed to {action}: {e}");
            app.action_error = Some(format!("{action} failed: {e}"));
        }
    }
}

/// Force a refresh by setting `last_tick` to a past time
fn force_refresh(app: &mut app::App) {
    app.last_tick = std::time::Instant::now()
        .checked_sub(Duration::from_secs(REFRESH_INTERVAL_SECS + 1))
        .unwrap_or_else(std::time::Instant::now);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_c_quits_instead_of_dispatching_a_dashboard_action() {
        let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let update = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);

        assert!(should_quit(interrupt, false));
        assert!(should_quit(interrupt, true));
        assert!(!should_dispatch_key(update));
    }
}
