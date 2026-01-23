use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::Duration;

pub mod app;
mod ui;

// Constants for TUI behavior
const POLL_TIMEOUT_MS: u64 = 100;
const REFRESH_INTERVAL_SECS: u64 = 5;

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
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

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

    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events with timeout for animations
        if crossterm::event::poll(Duration::from_millis(POLL_TIMEOUT_MS))?
            && let Event::Key(key) = event::read()?
        {
            // Only process key press events, ignore release
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Exit on 'q' - check this first for quick exit
            if key.code == KeyCode::Char('q') {
                return Ok(());
            }

            // Handle special actions before key processing
            handle_special_key_actions(app, key.code).await;

            // Handle search updates
            if app.search_mode && app.search_query != last_search {
                last_search.clone_from(&app.search_query);
                // Clone to avoid borrow checker issues
                let query = last_search.clone();
                if let Err(e) = app.search_packages(&query).await {
                    eprintln!("Search failed: {e}");
                }
            }
        }

        // Update app state
        app.tick().await?;
    }
}

/// Handle special key actions that trigger async operations
async fn handle_special_key_actions(app: &mut app::App, key_code: KeyCode) {
    match key_code {
        KeyCode::Char('u') if app.current_tab == app::Tab::Dashboard => {
            if let Err(e) = app.update_system().await {
                eprintln!("Failed to update system: {e}");
            }
            force_refresh(app);
        }
        KeyCode::Char('c') if app.current_tab == app::Tab::Dashboard => {
            if let Err(e) = app.clean_cache().await {
                eprintln!("Failed to clean cache: {e}");
            }
        }
        KeyCode::Char('o') if app.current_tab == app::Tab::Dashboard => {
            if let Err(e) = app.remove_orphans() {
                eprintln!("Failed to remove orphans: {e}");
            }
        }
        KeyCode::Char('a') if app.current_tab == app::Tab::Security => {
            match app.run_security_audit().await {
                Ok(0) => eprintln!("No vulnerabilities found!"),
                Ok(vulns) => eprintln!("Found {vulns} vulnerabilities"),
                Err(e) => eprintln!("Failed to run audit: {e}"),
            }
        }
        KeyCode::Enter
            if app.current_tab == app::Tab::Packages
                && !app.search_results.is_empty()
                && !app.show_popup =>
        {
            if let Some(pkg) = app.search_results.get(app.selected_index) {
                let pkg_name = pkg.name.clone();
                if let Err(e) = app.install_package(&pkg_name).await {
                    eprintln!("Failed to install {pkg_name}: {e}");
                }
                force_refresh(app);
            }
        }
        _ => {
            // Normal key handling
            app.handle_key(key_code);
        }
    }
}

/// Force a refresh by setting `last_tick` to a past time
fn force_refresh(app: &mut app::App) {
    app.last_tick = std::time::Instant::now()
        .checked_sub(Duration::from_secs(REFRESH_INTERVAL_SECS + 1))
        .unwrap_or_else(std::time::Instant::now);
}
