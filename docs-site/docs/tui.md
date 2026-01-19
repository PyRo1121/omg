---
title: TUI Dashboard
sidebar_label: TUI
sidebar_position: 26
description: Interactive terminal dashboard for system monitoring
---

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
| `Tab` | Switch view (planned) |

## Features

The TUI dashboard provides a real-time view of:

### System Status Panel (40% width)
- **Updates available**: Count of pending updates (yellow if > 0, green if up-to-date)
- **Total packages**: Number of installed packages
- **CVEs**: Known security vulnerabilities (red if > 0)
- **Runtimes**: Active versions for all managed runtimes (Node, Python, Go, Rust, Ruby, Java, Bun)

### Recent Activity Panel (60% width)
- **Transaction history**: Last 10 package operations
- **For each transaction**:
  - Timestamp (HH:MM:SS format)
  - Transaction type (Install, Remove, Update, Sync)
  - Success/failure indicator (✓ or ✗)
  - Affected packages (up to 3, then "...")

## Implementation

### Technology Stack

The TUI is built with:
- **ratatui**: Terminal UI framework (v0.29)
- **crossterm**: Cross-platform terminal manipulation (v0.28)

### Architecture

```
src/cli/tui/
├── mod.rs    # Entry point, terminal setup, main event loop
├── app.rs    # Application state (status, history, refresh logic)
└── ui.rs     # Layout and widget rendering
```

### Application State

```rust
pub struct App {
    pub status: Option<StatusResult>,  // System status from daemon
    pub history: Vec<Transaction>,      // Recent transactions
    pub last_tick: std::time::Instant,  // For auto-refresh timing
}
```

### Data Sources

1. **System Status**: Fetched from daemon via IPC
   ```rust
   let status = client.call(Request::Status { id: 0 }).await;
   ```

2. **Transaction History**: Loaded from local history file
   ```rust
   let entries = HistoryManager::new()?.load()?;
   ```

### Event Loop

The dashboard uses a 100ms polling interval:

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
        
        app.tick().await?;  // Auto-refresh every 10s
    }
}
```

### Auto-Refresh

The dashboard automatically refreshes every 10 seconds:
```rust
pub async fn tick(&mut self) -> Result<()> {
    if self.last_tick.elapsed() >= Duration::from_secs(10) {
        self.refresh().await?;
        self.last_tick = Instant::now();
    }
    Ok(())
}
```

## UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ OMG  Dashboard                                                  │
├─────────────────────────────┬───────────────────────────────────┤
│ System Status               │ Recent Activity                   │
│                             │                                   │
│ Updates: Up to date         │ [13:00:00] Install ✓              │
│ Packages: 1234              │   firefox, neovim                 │
│ CVEs: None                  │ [12:30:00] Update ✓               │
│                             │   linux, mesa                     │
│ Runtimes:                   │ [12:00:00] Remove ✓               │
│   • node     20.10.0        │   old-package                     │
│   • python   3.12.0         │                                   │
│   • rust     1.75.0         │                                   │
│                             │                                   │
├─────────────────────────────┴───────────────────────────────────┤
│ [q] Quit  [r] Refresh  [Tab] Switch View                        │
└─────────────────────────────────────────────────────────────────┘
```

## Terminal Requirements

The TUI requires:
- **Terminal**: Any modern terminal emulator (xterm, alacritty, kitty, wezterm, etc.)
- **Alternate Screen**: Uses alternate screen buffer to preserve shell history
- **Unicode Support**: Uses Unicode symbols (✓, ✗, •)
- **Color Support**: 256 colors recommended (`$TERM` should report color support)

## Troubleshooting

### Dashboard doesn't start

1. **Check terminal compatibility**:
   ```bash
   echo $TERM
   # Should show xterm-256color or similar
   ```

2. **Check daemon connection**: Dashboard needs the daemon for status
   ```bash
   omg daemon  # Start daemon if not running
   ```

### Display issues

- **Garbled output**: Try resizing your terminal window
- **Colors wrong**: Ensure `TERM` supports 256 colors
- **Missing characters**: Use a font with Unicode support (Nerd Fonts recommended)

### Data not updating

- Press `r` to force refresh
- Check if daemon is running: `omg status`
- Verify socket connectivity

### Status shows "Loading..."

The dashboard couldn't connect to the daemon. Start it with:
```bash
omg daemon
```

## Source Files

| File | Purpose |
|------|---------|
| [tui/mod.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/mod.rs) | Entry point, terminal setup/cleanup, main event loop |
| [tui/app.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/app.rs) | Application state management and refresh logic |
| [tui/ui.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/ui.rs) | Layout definitions and widget rendering |
