# TUI Dashboard

OMG includes an interactive terminal user interface (TUI) for system monitoring and management via the `omg dash` command.

## Quick Start

```bash
omg dash
```

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` | Quit the dashboard |
| `r` | Refresh all data |

## Features

The TUI dashboard provides a real-time view of:

- **System Status**: Package counts, updates available, vulnerabilities
- **Runtime Versions**: Active versions for all managed runtimes (Node, Python, Go, Rust, Ruby, Java, Bun)
- **Package Information**: Quick access to package search and info

## Implementation

### Technology Stack

The TUI is built with:
- **ratatui**: Terminal UI framework (v0.29)
- **crossterm**: Cross-platform terminal manipulation (v0.28)

### Architecture

```rust
// Entry point: src/cli/tui/mod.rs
pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut app = App::new().await?;
    run_app(&mut terminal, &mut app).await
}
```

### Components

The TUI consists of three main modules:

| Module | Purpose |
|--------|---------|
| `mod.rs` | Entry point, terminal setup/cleanup, main event loop |
| `app.rs` | Application state management and business logic |
| `ui.rs` | UI layout and widget rendering |

### Event Loop

The dashboard uses a polling-based event loop:

```rust
async fn run_app(terminal: &mut Terminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') => app.refresh().await?,
                    _ => App::handle_key(key.code),
                }
            }
        }
        
        app.tick().await?;
    }
}
```

Key characteristics:
- **100ms poll interval**: Responsive but not CPU-intensive
- **Async refresh**: Data fetches don't block UI
- **Auto-tick**: Periodic background updates

## Data Sources

The dashboard pulls data from:

1. **Daemon IPC**: System status, package counts, vulnerabilities
2. **Runtime Probes**: Active runtime versions from `<data_dir>/versions/*/current` symlinks
3. **libalpm**: Package database queries (if daemon unavailable)

## Terminal Requirements

The TUI requires:
- **Terminal**: Any modern terminal emulator (xterm, alacritty, kitty, etc.)
- **Alternate Screen**: Uses alternate screen buffer to preserve shell history
- **Mouse Support**: Optional, can be enabled for future enhancements

## Troubleshooting

### Dashboard doesn't start

1. Check terminal compatibility:
   ```bash
   echo $TERM
   # Should show xterm-256color or similar
   ```

2. Ensure raw mode is supported:
   ```bash
   stty -a
   ```

### Display issues

- **Garbled output**: Try resizing your terminal window
- **Colors wrong**: Ensure `TERM` supports 256 colors
- **Missing characters**: Use a font with Unicode support (Nerd Fonts recommended)

### Data not refreshing

- Press `r` to force refresh
- Check if daemon is running: `omg daemon`
- Verify socket connectivity

## Future Enhancements

Planned features include:
- **Package search**: Interactive fuzzy search
- **Install/Remove**: Direct package management
- **Tabs**: Multiple views (packages, runtimes, history)
- **Mouse support**: Click navigation
- **Themes**: Customizable color schemes

## Source Files

- Entry point: [tui/mod.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/mod.rs)
- App state: [tui/app.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/app.rs)
- UI rendering: [tui/ui.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/ui.rs)
