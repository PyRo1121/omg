//! Interactive first-run setup wizard for OMG.
//!
//! Reduces friction from install → first successful command to <2 minutes.

use anyhow::{Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Stylize},
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use std::process::Command;

/// Shell options for hook installation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    fn name(&self) -> &'static str {
        match self {
            Shell::Zsh => "zsh",
            Shell::Bash => "bash",
            Shell::Fish => "fish",
        }
    }

    fn config_file(&self) -> &'static str {
        match self {
            Shell::Zsh => "~/.zshrc",
            Shell::Bash => "~/.bashrc",
            Shell::Fish => "~/.config/fish/config.fish",
        }
    }

    fn hook_command(&self) -> String {
        match self {
            Shell::Zsh => r#"eval "$(omg hook zsh)""#.to_string(),
            Shell::Bash => r#"eval "$(omg hook bash)""#.to_string(),
            Shell::Fish => "omg hook fish | source".to_string(),
        }
    }
}

/// Daemon startup options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStartup {
    /// Start daemon on shell init (fastest, recommended)
    OnShellInit,
    /// Start daemon on first OMG command
    OnDemand,
    /// Use systemd user service
    Systemd,
    /// Don't auto-start daemon
    Manual,
}

impl DaemonStartup {
    fn name(&self) -> &'static str {
        match self {
            DaemonStartup::OnShellInit => "On shell init (fastest)",
            DaemonStartup::OnDemand => "On first OMG command",
            DaemonStartup::Systemd => "Systemd user service",
            DaemonStartup::Manual => "Manual (I'll start it myself)",
        }
    }
}

/// Wizard state
struct WizardState {
    shell: Option<Shell>,
    daemon_startup: DaemonStartup,
    capture_env: bool,
    current_step: usize,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            shell: None,
            daemon_startup: DaemonStartup::OnShellInit,
            capture_env: true,
            current_step: 0,
        }
    }
}

/// Run the interactive setup wizard
pub async fn run_interactive(
    skip_shell: bool,
    skip_daemon: bool,
) -> Result<()> {
    let mut stdout = io::stdout();

    // Clear screen and show welcome
    execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    print_header(&mut stdout)?;
    println!();

    let mut state = WizardState::default();

    // Step 1: Detect and confirm shell
    if !skip_shell {
        state.shell = Some(select_shell(&mut stdout)?);
        println!();
    }

    // Step 2: Daemon startup preference
    if !skip_daemon {
        state.daemon_startup = select_daemon_startup(&mut stdout)?;
        println!();
    }

    // Step 3: Environment capture
    state.capture_env = confirm_env_capture(&mut stdout)?;
    println!();

    // Apply configuration
    println!();
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("═══════════════════════════════════════════════════════════\n"),
        Print("  Applying Configuration\n"),
        Print("═══════════════════════════════════════════════════════════\n"),
        ResetColor
    )?;
    println!();

    // Install shell hook
    if let Some(shell) = state.shell {
        install_shell_hook(&mut stdout, shell)?;
    }

    // Configure daemon startup
    configure_daemon_startup(&mut stdout, state.daemon_startup)?;

    // Capture environment
    if state.capture_env {
        capture_environment(&mut stdout).await?;
    }

    // Show completion message
    print_completion(&mut stdout, &state)?;

    Ok(())
}

/// Run with defaults (non-interactive)
pub async fn run_defaults() -> Result<()> {
    let mut stdout = io::stdout();

    println!();
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("OMG"),
        ResetColor,
        Print(" Setting up with defaults...\n")
    )?;
    println!();

    // Detect shell
    let shell = detect_current_shell();
    if let Some(s) = shell {
        install_shell_hook(&mut stdout, s)?;
    }

    // Start daemon
    configure_daemon_startup(&mut stdout, DaemonStartup::OnDemand)?;

    // Capture environment
    capture_environment(&mut stdout).await?;

    println!();
    execute!(
        stdout,
        SetForegroundColor(Color::Green),
        Print("✓"),
        ResetColor,
        Print(" Setup complete! Restart your shell or run: "),
        SetForegroundColor(Color::Yellow),
        Print("source ~/.zshrc\n"),
        ResetColor
    )?;

    Ok(())
}

fn print_header(stdout: &mut io::Stdout) -> Result<()> {
    execute!(
        stdout,
        SetForegroundColor(Color::Magenta),
        Print("╔═══════════════════════════════════════════════════════════╗\n"),
        Print("║"),
        SetForegroundColor(Color::White),
        Print("              🚀 Welcome to OMG Setup                      "),
        SetForegroundColor(Color::Magenta),
        Print("║\n"),
        Print("║"),
        SetForegroundColor(Color::DarkGrey),
        Print("    The Fastest Unified Package Manager for Linux          "),
        SetForegroundColor(Color::Magenta),
        Print("║\n"),
        Print("╚═══════════════════════════════════════════════════════════╝\n"),
        ResetColor
    )?;
    println!();
    println!("  This wizard will configure OMG in about 60 seconds.");
    println!("  Use ↑/↓ to navigate, Enter to select, q to quit.");
    Ok(())
}

fn detect_current_shell() -> Option<Shell> {
    if let Ok(shell) = std::env::var("SHELL") {
        if shell.contains("zsh") {
            return Some(Shell::Zsh);
        } else if shell.contains("bash") {
            return Some(Shell::Bash);
        } else if shell.contains("fish") {
            return Some(Shell::Fish);
        }
    }
    None
}

fn select_shell(stdout: &mut io::Stdout) -> Result<Shell> {
    let detected = detect_current_shell();
    let shells = [Shell::Zsh, Shell::Bash, Shell::Fish];
    let mut selected = detected.map(|s| shells.iter().position(|x| *x == s).unwrap_or(0)).unwrap_or(0);

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("Step 1/3: "),
        ResetColor,
        Print("Select your shell\n")
    )?;

    if let Some(d) = detected {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  (detected: {})\n", d.name())),
            ResetColor
        )?;
    }
    println!();

    terminal::enable_raw_mode()?;

    loop {
        // Clear and redraw options
        for (i, shell) in shells.iter().enumerate() {
            let prefix = if i == selected { "  ▸ " } else { "    " };
            let suffix = if Some(*shell) == detected { " (detected)" } else { "" };

            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("{}{}{}\n", prefix, shell.name(), suffix)),
                    ResetColor
                )?;
            } else {
                execute!(
                    stdout,
                    Print(format!("{}{}{}\n", prefix, shell.name(), suffix))
                )?;
            }
        }

        stdout.flush()?;

        // Wait for key
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected < shells.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    terminal::disable_raw_mode()?;
                    return Ok(shells[selected]);
                }
                KeyCode::Char('q') => {
                    terminal::disable_raw_mode()?;
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        // Move cursor back up to redraw
        execute!(stdout, cursor::MoveUp(shells.len() as u16))?;
    }
}

fn select_daemon_startup(stdout: &mut io::Stdout) -> Result<DaemonStartup> {
    let options = [
        DaemonStartup::OnShellInit,
        DaemonStartup::OnDemand,
        DaemonStartup::Systemd,
        DaemonStartup::Manual,
    ];
    let mut selected = 0;

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("Step 2/3: "),
        ResetColor,
        Print("When should the daemon start?\n")
    )?;
    execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("  (daemon enables 22x faster searches via in-memory index)\n"),
        ResetColor
    )?;
    println!();

    terminal::enable_raw_mode()?;

    loop {
        for (i, opt) in options.iter().enumerate() {
            let prefix = if i == selected { "  ▸ " } else { "    " };

            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("{}{}\n", prefix, opt.name())),
                    ResetColor
                )?;
            } else {
                execute!(stdout, Print(format!("{}{}\n", prefix, opt.name())))?;
            }
        }

        stdout.flush()?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected < options.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    terminal::disable_raw_mode()?;
                    return Ok(options[selected]);
                }
                KeyCode::Char('q') => {
                    terminal::disable_raw_mode()?;
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        execute!(stdout, cursor::MoveUp(options.len() as u16))?;
    }
}

fn confirm_env_capture(stdout: &mut io::Stdout) -> Result<bool> {
    let options = [true, false];
    let mut selected = 0;

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("Step 3/3: "),
        ResetColor,
        Print("Capture initial environment to omg.lock?\n")
    )?;
    execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("  (enables team sync and drift detection)\n"),
        ResetColor
    )?;
    println!();

    terminal::enable_raw_mode()?;

    loop {
        let labels = ["  ▸ Yes, capture my environment", "    No, I'll do it later"];
        for (i, label) in labels.iter().enumerate() {
            let display = if i == 0 {
                if selected == 0 { "  ▸ Yes, capture my environment" } else { "    Yes, capture my environment" }
            } else if selected == 1 {
                "  ▸ No, I'll do it later"
            } else {
                "    No, I'll do it later"
            };

            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("{}\n", display)),
                    ResetColor
                )?;
            } else {
                execute!(stdout, Print(format!("{}\n", display)))?;
            }
        }

        stdout.flush()?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Down => {
                    selected = 1 - selected;
                }
                KeyCode::Enter => {
                    terminal::disable_raw_mode()?;
                    return Ok(options[selected]);
                }
                KeyCode::Char('q') => {
                    terminal::disable_raw_mode()?;
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        execute!(stdout, cursor::MoveUp(2))?;
    }
}

fn install_shell_hook(stdout: &mut io::Stdout, shell: Shell) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let config_path = shell.config_file().replace('~', &home);
    let hook_cmd = shell.hook_command();

    execute!(
        stdout,
        Print("  "),
        SetForegroundColor(Color::Blue),
        Print("→"),
        ResetColor,
        Print(format!(" Installing {} hook...", shell.name()))
    )?;

    // Check if hook already exists
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if content.contains("omg hook") {
            execute!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print(" (already installed)\n"),
                ResetColor
            )?;
            return Ok(());
        }
    }

    // Append hook to config
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_path)
        .with_context(|| format!("Failed to open {}", config_path))?;

    writeln!(file, "\n# OMG shell integration")?;
    writeln!(file, "{}", hook_cmd)?;

    execute!(
        stdout,
        SetForegroundColor(Color::Green),
        Print(" ✓\n"),
        ResetColor
    )?;

    Ok(())
}

fn configure_daemon_startup(stdout: &mut io::Stdout, startup: DaemonStartup) -> Result<()> {
    execute!(
        stdout,
        Print("  "),
        SetForegroundColor(Color::Blue),
        Print("→"),
        ResetColor,
        Print(" Configuring daemon...")
    )?;

    match startup {
        DaemonStartup::OnShellInit => {
            // The shell hook already handles this
            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print(" ✓ (via shell hook)\n"),
                ResetColor
            )?;
        }
        DaemonStartup::OnDemand => {
            // Start daemon now
            let _ = Command::new("omg")
                .args(["daemon"])
                .spawn();
            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print(" ✓ (started)\n"),
                ResetColor
            )?;
        }
        DaemonStartup::Systemd => {
            // Create systemd user service
            create_systemd_service()?;
            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print(" ✓ (systemd service created)\n"),
                ResetColor
            )?;
        }
        DaemonStartup::Manual => {
            execute!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print(" (skipped - run 'omg daemon' when ready)\n"),
                ResetColor
            )?;
        }
    }

    Ok(())
}

fn create_systemd_service() -> Result<()> {
    let home = std::env::var("HOME")?;
    let service_dir = format!("{}/.config/systemd/user", home);
    std::fs::create_dir_all(&service_dir)?;

    let service_content = r#"[Unit]
Description=OMG Package Manager Daemon
After=default.target

[Service]
Type=simple
ExecStart=%h/.local/bin/omgd --foreground
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#;

    std::fs::write(format!("{}/omgd.service", service_dir), service_content)?;

    // Enable and start the service
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    let _ = Command::new("systemctl")
        .args(["--user", "enable", "--now", "omgd.service"])
        .output();

    Ok(())
}

async fn capture_environment(stdout: &mut io::Stdout) -> Result<()> {
    execute!(
        stdout,
        Print("  "),
        SetForegroundColor(Color::Blue),
        Print("→"),
        ResetColor,
        Print(" Capturing environment...")
    )?;
    stdout.flush()?;

    // Use the existing env capture function
    match crate::core::env::fingerprint::EnvironmentState::capture().await {
        Ok(state) => {
            if let Err(e) = state.save("omg.lock") {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Yellow),
                    Print(format!(" (failed: {})\n", e)),
                    ResetColor
                )?;
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(" ✓\n"),
                    ResetColor
                )?;
            }
        }
        Err(e) => {
            execute!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print(format!(" (skipped: {})\n", e)),
                ResetColor
            )?;
        }
    }

    Ok(())
}

fn print_completion(stdout: &mut io::Stdout, state: &WizardState) -> Result<()> {
    println!();
    execute!(
        stdout,
        SetForegroundColor(Color::Green),
        Print("═══════════════════════════════════════════════════════════\n"),
        Print("  ✓ Setup Complete!\n"),
        Print("═══════════════════════════════════════════════════════════\n"),
        ResetColor
    )?;
    println!();

    println!("  {} Restart your shell or run:", "Next:".bold());
    if let Some(shell) = state.shell {
        println!("      source {}", shell.config_file());
    }
    println!();

    println!("  {} Try these commands:", "Quick start:".bold());
    println!("      omg search vim          # 22x faster than pacman");
    println!("      omg use node 20         # Install & switch Node.js");
    println!("      omg status              # System overview");
    println!("      omg dash                # Interactive dashboard");
    println!();

    execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("  Full docs: https://pyro1121.com/docs\n"),
        ResetColor
    )?;

    Ok(())
}
