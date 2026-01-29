---
title: "TUI Dashboard"
description: "Interactive terminal dashboard for system monitoring"
---

# TUI Dashboard

**Real-Time System Monitoring and Management**

The OMG interactive terminal dashboard (`omg dash`) provides a unified view of your system health, package updates, active runtime versions, and recent activity.

---

## 🚀 Quick Start

```bash
omg dash
```

---

## 🎹 Keyboard Controls

| Key | Action |
|-----|--------|
| `q` | Quit the dashboard |
| `r` | Refresh all data |
| `Tab` | Switch between views |
| `↑/↓` | Scroll through lists |
| `Enter` | Select/expand item |
| `?` | Show help |

---

## 📊 Dashboard Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ OMG Dashboard                                             [r]efresh [q]uit│
├─────────────────────────────────┬───────────────────────────────────────┤
│ System Status                   │ Recent Activity                       │
│                                 │                                       │
│ ┌─────────────────────────────┐ │ ┌───────────────────────────────────┐ │
│ │ Packages                    │ │ │ [13:45:30] Install ✓             │ │
│ │   Total:      1,847         │ │ │   firefox, neovim                 │ │
│ │   Explicit:   423           │ │ │ [13:30:15] Update ✓               │ │
│ │   Orphans:    12            │ │ │   linux, mesa, nvidia-dkms        │ │
│ │                             │ │ │ [12:00:00] Remove ✓               │ │
│ │ Updates                     │ │ │   old-package                     │ │
│ │   Available:  5     ▼       │ │ │ [11:45:22] Sync ✓                 │ │
│ │                             │ │ │                                   │ │
│ │ Security                    │ │ │                                   │ │
│ │   CVEs:       0     ✓       │ │ │                                   │ │
│ │   Grade:      VERIFIED      │ │ │                                   │ │
│ └─────────────────────────────┘ │ └───────────────────────────────────┘ │
│                                 │                                       │
│ Active Runtimes                 │                                       │
│ ┌─────────────────────────────┐ │                                       │
│ │ • node     20.10.0          │ │                                       │
│ │ • python   3.12.0           │ │                                       │
│ │ • rust     1.75.0           │ │                                       │
│ │ • go       1.21.5           │ │                                       │
│ │ • bun      1.0.25           │ │                                       │
│ └─────────────────────────────┘ │                                       │
├─────────────────────────────────┴───────────────────────────────────────┤
│ [q] Quit  [r] Refresh  [Tab] Switch View  [?] Help                      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 📋 Panel Details

### System Status Panel (Left, 40%)

Displays core system metrics:

| Metric | Description | Color Coding |
|--------|-------------|--------------|
| **Total** | All installed packages | White |
| **Explicit** | Explicitly installed (not deps) | Cyan |
| **Orphans** | Unused dependencies | Yellow if > 0 |
| **Updates** | Available updates | Yellow if > 0, Green if 0 |
| **CVEs** | Known vulnerabilities | Red if > 0, Green if 0 |
| **Grade** | Overall security grade | Varies by grade |

### Active Runtimes Section

Shows currently active version for each runtime:
- **Node.js** — From `.nvmrc` or `current` symlink
- **Python** — From `.python-version` or `current` symlink
- **Rust** — From `rust-toolchain.toml` or `current` symlink
- **Go, Ruby, Java, Bun** — From respective version files

Runtimes without an active version are dimmed.

### Recent Activity Panel (Right, 60%)

Shows the last 10 package transactions:

| Field | Format |
|-------|--------|
| **Time** | HH:MM:SS |
| **Type** | Install, Remove, Update, Sync |
| **Status** | ✓ (success) or ✗ (failure) |
| **Packages** | First 3 packages, then "..." |

---

## 🔄 Data Sources

### System Status

Fetched from daemon via IPC:

```rust
let status = client.call(Request::Status { id: 0 }).await?;
```

Contains:
- Package counts
- Update availability
- Vulnerability counts
- Runtime versions

### Transaction History

Loaded from local history file:

```rust
let entries = HistoryManager::new()?.load()?;
self.history = entries.into_iter().rev().take(10).collect();
```

---

## ⏱️ Auto-Refresh

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

Manual refresh with `r` key is instant.

---

## 🎨 Visual Indicators

### Status Colors

| Color | Meaning |
|-------|---------|
| 🟢 Green | Healthy / No issues |
| 🟡 Yellow | Warning / Action recommended |
| 🔴 Red | Critical / Immediate attention |
| ⚪ White | Informational |
| 🔵 Cyan | Highlight / Active |

### Icons

| Icon | Meaning |
|------|---------|
| ✓ | Success |
| ✗ | Failure |
| • | List item |
| ▲ | Increase |
| ▼ | Available |

---

## ⚙️ Technical Implementation

### Technology Stack

| Component | Technology |
|-----------|------------|
| **TUI Framework** | ratatui v0.29 |
| **Terminal Backend** | crossterm v0.28 |
| **Layout** | Constraint-based (40%/60%) |

### Architecture

```
src/cli/tui/
├── mod.rs     # Entry point, terminal setup, event loop
├── app.rs     # Application state, refresh logic
└── ui.rs      # Layout definitions, widget rendering
```

### Application State

```rust
pub struct App {
    pub status: Option<StatusResult>,   // System status from daemon
    pub history: Vec<Transaction>,       // Recent transactions
    pub last_tick: std::time::Instant,   // For auto-refresh timing
}
```

### Event Loop

```rust
async fn run_app(terminal: &mut Terminal, app: &mut App) -> Result<()> {
    loop {
        // Render
        terminal.draw(|f| ui::draw(f, app))?;
        
        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') => app.refresh().await?,
                    _ => {}
                }
            }
        }
        
        // Auto-refresh
        app.tick().await?;
    }
}
```

---

## 📱 Terminal Requirements

### Minimum Requirements

| Requirement | Details |
|-------------|---------|
| **Terminal** | Any modern terminal (xterm, alacritty, kitty, wezterm) |
| **Alternate Screen** | Must support alternate screen buffer |
| **Colors** | 256 colors recommended |
| **Unicode** | Full Unicode support required |
| **Size** | Minimum 80x24 characters |

### Recommended Setup

```bash
# Ensure TERM is set correctly
export TERM=xterm-256color

# Use a Nerd Font for icons
# Recommended: JetBrains Mono Nerd Font
```

---

## 🔧 Troubleshooting

### Dashboard Won't Start

```bash
# 1. Check terminal compatibility
echo $TERM
# Should show xterm-256color or similar

# 2. Check daemon is running
omg status
# If not running:
omg daemon

# 3. Test alternate screen
tput smcup
tput rmcup
```

### Display Issues

| Issue | Solution |
|-------|----------|
| Garbled characters | Use Unicode-capable font |
| Wrong colors | Set `TERM=xterm-256color` |
| Layout broken | Resize terminal window |
| Missing icons | Install Nerd Fonts |

### Reset Terminal

If the terminal is garbled after exit:

```bash
reset
# or
stty sane
```

### Data Not Updating

```bash
# 1. Manual refresh
# Press 'r' in dashboard

# 2. Check daemon
omg status

# 3. Restart daemon
pkill omgd
omg daemon
```

---

## 🎯 Best Practices

### 1. Keep Running in Dedicated Terminal

For monitoring, keep the dashboard running in a dedicated terminal pane/tab.

### 2. Use with tmux/Screen

```bash
# Create dedicated session
tmux new-session -d -s omg-dash 'omg dash'

# Attach when needed
tmux attach -t omg-dash
```

### 3. Combine with Notifications

Use alongside system notifications for critical alerts:

```bash
# In a cron job or systemd timer
omg audit scan 2>&1 | grep -q "high_severity" && notify-send "OMG: Security Alert"
```

---

## 📚 See Also

- [Quick Start](./quickstart.md) — Initial setup
- [CLI Reference](./cli.md) — All commands
- [History & Rollback](./history.md) — Transaction history
- [Troubleshooting](./troubleshooting.md) — Common issues
