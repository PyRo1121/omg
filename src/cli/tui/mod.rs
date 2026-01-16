use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::Duration;

mod app;
mod ui;

pub async fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Hide cursor
    terminal.hide_cursor()?;

    // Create app and run it
    let mut app = app::App::new().await?;
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    terminal.clear()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut app::App,
) -> Result<()> {
    let mut last_search = String::new();

    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events with timeout for animations
        if crossterm::event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Handle special actions before key processing
                    match key.code {
                        KeyCode::Char('u') if app.current_tab == app::Tab::Dashboard => {
                            // Update system
                            if let Err(e) = app.update_system().await {
                                eprintln!("Failed to update system: {e}");
                            }
                            // Refresh after update
                            app.last_tick = std::time::Instant::now()
                                .checked_sub(Duration::from_secs(10))
                                .unwrap_or_else(std::time::Instant::now);
                        }
                        KeyCode::Char('c') if app.current_tab == app::Tab::Dashboard => {
                            // Clean cache
                            if let Err(e) = app.clean_cache().await {
                                eprintln!("Failed to clean cache: {e}");
                            }
                        }
                        KeyCode::Char('o') if app.current_tab == app::Tab::Dashboard => {
                            // Remove orphans
                            if let Err(e) = app.remove_orphans().await {
                                eprintln!("Failed to remove orphans: {e}");
                            }
                        }
                        KeyCode::Char('a') if app.current_tab == app::Tab::Security => {
                            // Run security audit
                            match app.run_security_audit().await {
                                Ok(vulns) => {
                                    if vulns == 0 {
                                        println!("No vulnerabilities found!");
                                    } else {
                                        println!("Found {vulns} vulnerabilities");
                                    }
                                }
                                Err(e) => eprintln!("Failed to run audit: {e}"),
                            }
                        }
                        KeyCode::Enter
                            if app.current_tab == app::Tab::Packages
                                && !app.search_results.is_empty()
                                && !app.show_popup =>
                        {
                            // Install selected package
                            let Some(pkg) = app.search_results.get(app.selected_index) else {
                                continue;
                            };
                            let pkg_name = &pkg.name;
                            if let Err(e) = app.install_package(pkg_name).await {
                                eprintln!("Failed to install {pkg_name}: {e}");
                            }
                            // Refresh after install
                            app.last_tick = std::time::Instant::now()
                                .checked_sub(Duration::from_secs(10))
                                .unwrap_or_else(std::time::Instant::now);
                        }
                        _ => {
                            // Normal key handling
                            app.handle_key(key.code);
                        }
                    }

                    // Handle search
                    if app.search_mode && app.search_query != last_search {
                        last_search.clone_from(&app.search_query);
                        let query = app.search_query.clone();
                        if let Err(e) = app.search_packages(&query).await {
                            eprintln!("Search failed: {e}");
                        }
                    }

                    // Exit on 'q'
                    if key.code == KeyCode::Char('q') {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        // Update app state
        app.tick().await?;
    }
}
