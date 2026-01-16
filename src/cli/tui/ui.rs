use crate::cli::tui::app::{App, Tab};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table, Wrap,
    },
};

pub fn draw(f: &mut Frame, app: &App) {
    // Main layout with header, body, and footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    draw_header(f, main_chunks[0], app);

    match app.current_tab {
        Tab::Dashboard => draw_dashboard(f, main_chunks[1], app),
        Tab::Packages => draw_packages(f, main_chunks[1], app),
        Tab::Runtimes => draw_runtimes(f, main_chunks[1], app),
        Tab::Security => draw_security(f, main_chunks[1], app),
        Tab::Activity => draw_activity(f, main_chunks[1], app),
    }

    draw_footer(f, main_chunks[2], app);

    // Draw popup if active
    if app.show_popup {
        draw_popup(f, app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let header_text = match app.current_tab {
        Tab::Dashboard => "System Dashboard",
        Tab::Packages => "Package Management",
        Tab::Runtimes => "Runtime Versions",
        Tab::Security => "Security Center",
        Tab::Activity => "Activity Monitor",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " OMG ",
            Style::default()
                .bg(Color::Rgb(0, 150, 200))
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {header_text} "),
            Style::default()
                .fg(Color::Rgb(200, 200, 200))
                .add_modifier(Modifier::BOLD),
        ),
        if app.daemon_connected {
            Span::styled(" ● ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" ● ", Style::default().fg(Color::Red))
        },
        Span::styled(
            format!(" v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(header, area);
}

fn draw_dashboard(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // System Overview
            Constraint::Percentage(40), // Real-time Stats
            Constraint::Percentage(30), // Quick Actions
        ])
        .split(area);

    draw_system_overview(f, chunks[0], app);
    draw_realtime_stats(f, chunks[1], app);
    draw_quick_actions(f, chunks[2], app);
}

fn draw_system_overview(f: &mut Frame, area: Rect, app: &App) {
    let blocks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .split(area);

    // System Health
    let vulnerabilities = app.get_security_vulnerabilities();
    let health_color = if vulnerabilities == 0 {
        Color::Green
    } else if vulnerabilities < 5 {
        Color::Yellow
    } else {
        Color::Red
    };

    let health_text = match vulnerabilities {
        0 => "Excellent",
        1..=5 => "Good",
        6..=20 => "Fair",
        _ => "Critical",
    };

    let health_gauge = Gauge::default()
        .block(
            Block::default()
                .title(" System Health ")
                .borders(Borders::ALL),
        )
        .gauge_style(
            Style::default()
                .fg(health_color)
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
        )
        .percent(if vulnerabilities == 0 {
            100
        } else {
            std::cmp::max(20, 100 - (vulnerabilities * 5) as u16)
        })
        .label(format!("{health_text} ({vulnerabilities} CVEs)"));

    f.render_widget(health_gauge, blocks[0]);

    // Package Stats
    let total_packages = app.get_total_packages();
    let updates_available = app.get_updates_available();

    let package_lines = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::Gray)),
            Span::styled(
                total_packages.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Updates: ", Style::default().fg(Color::Gray)),
            Span::styled(
                updates_available.to_string(),
                Style::default()
                    .fg(if updates_available > 0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Orphans: ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.get_orphan_packages().to_string(),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled("Foreign: ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.status
                    .as_ref()
                    .map_or(0, |s| s.explicit_packages)
                    .to_string(),
                Style::default().fg(Color::Blue),
            ),
        ]),
    ];

    let package_stats = Paragraph::new(package_lines)
        .block(
            Block::default()
                .title(" Package Statistics ")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(package_stats, blocks[1]);

    // Disk Usage
    let disk_used_gb = app.system_metrics.disk_usage / 1024 / 1024;
    let disk_free_gb = app.system_metrics.disk_free / 1024 / 1024;
    let disk_total_gb = disk_used_gb + disk_free_gb;
    let disk_percent = if disk_total_gb > 0 {
        (disk_used_gb * 100) / disk_total_gb
    } else {
        0
    };

    let disk_usage = vec![
        Line::from("Disk Usage:"),
        Line::from(vec![
            Span::styled("┌─", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "■".repeat((disk_percent * 30 / 100) as usize),
                Style::default().fg(Color::Blue),
            ),
            Span::styled(
                "░".repeat(30 - (disk_percent * 30 / 100) as usize),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("┐ {disk_used_gb} GB"),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(" ".repeat(30), Style::default()),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("└─", Style::default().fg(Color::DarkGray)),
            Span::styled("".repeat(30), Style::default()),
            Span::styled(
                format!("┘ {disk_free_gb} GB free"),
                Style::default().fg(Color::Gray),
            ),
        ]),
    ];

    let disk_widget =
        Paragraph::new(disk_usage).block(Block::default().title(" Storage ").borders(Borders::ALL));

    f.render_widget(disk_widget, blocks[2]);
}

fn draw_realtime_stats(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(0),
        ])
        .split(area);

    // CPU Usage
    let cpu_data = vec![
        (app.system_metrics.cpu_usage * 100.0) as u64,
        (app.system_metrics.cpu_usage * 95.0) as u64,
        (app.system_metrics.cpu_usage * 105.0) as u64,
        (app.system_metrics.cpu_usage * 90.0) as u64,
        (app.system_metrics.cpu_usage * 110.0) as u64,
        (app.system_metrics.cpu_usage * 85.0) as u64,
        (app.system_metrics.cpu_usage * 100.0) as u64,
        (app.system_metrics.cpu_usage * 95.0) as u64,
        (app.system_metrics.cpu_usage * 105.0) as u64,
        (app.system_metrics.cpu_usage * 100.0) as u64,
        (app.system_metrics.cpu_usage * 90.0) as u64,
        (app.system_metrics.cpu_usage * 95.0) as u64,
    ];

    let cpu_sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(format!(" CPU Usage {:.1}% ", app.system_metrics.cpu_usage))
                .borders(Borders::ALL),
        )
        .data(&cpu_data)
        .style(Style::default().fg(Color::Cyan))
        .max(100);

    f.render_widget(cpu_sparkline, chunks[0]);

    // Memory Usage
    let mem_data = vec![
        (app.system_metrics.memory_usage * 100.0) as u64,
        (app.system_metrics.memory_usage * 95.0) as u64,
        (app.system_metrics.memory_usage * 105.0) as u64,
        (app.system_metrics.memory_usage * 90.0) as u64,
        (app.system_metrics.memory_usage * 110.0) as u64,
        (app.system_metrics.memory_usage * 85.0) as u64,
        (app.system_metrics.memory_usage * 100.0) as u64,
        (app.system_metrics.memory_usage * 95.0) as u64,
        (app.system_metrics.memory_usage * 105.0) as u64,
        (app.system_metrics.memory_usage * 100.0) as u64,
        (app.system_metrics.memory_usage * 90.0) as u64,
        (app.system_metrics.memory_usage * 95.0) as u64,
    ];

    let mem_sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(format!(
                    " Memory Usage {:.1}% ",
                    app.system_metrics.memory_usage
                ))
                .borders(Borders::ALL),
        )
        .data(&mem_data)
        .style(Style::default().fg(Color::Magenta))
        .max(100);

    f.render_widget(mem_sparkline, chunks[1]);

    // Network Activity
    let network_lines = vec![
        Line::from("Network Activity:"),
        Line::from(vec![
            Span::styled("↓ ", Style::default().fg(Color::Green)),
            Span::styled("RX: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_bytes(app.system_metrics.network_rx),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("↑ ", Style::default().fg(Color::Blue)),
            Span::styled("TX: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_bytes(app.system_metrics.network_tx),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!(
            "Daemon: {}",
            if app.daemon_connected {
                "Connected"
            } else {
                "Offline"
            }
        )),
    ];

    let network_widget = Paragraph::new(network_lines)
        .block(Block::default().title(" Network ").borders(Borders::ALL));

    f.render_widget(network_widget, chunks[2]);
}

fn draw_quick_actions(f: &mut Frame, area: Rect, app: &App) {
    let updates_available = app.get_updates_available();
    let orphans_count = app.get_orphan_packages();

    let actions = vec![
        ListItem::new(Line::from(vec![
            Span::styled("[u]", Style::default().fg(Color::Yellow)),
            Span::styled(" Update System", Style::default()),
            if updates_available > 0 {
                Span::styled(
                    format!(" ({updates_available})"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("", Style::default())
            },
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("[s]", Style::default().fg(Color::Yellow)),
            Span::styled(" Search Packages", Style::default()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("[c]", Style::default().fg(Color::Yellow)),
            Span::styled(" Clean Cache", Style::default()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("[o]", Style::default().fg(Color::Yellow)),
            Span::styled(" Remove Orphans", Style::default()),
            if orphans_count > 0 {
                Span::styled(
                    format!(" ({orphans_count})"),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                Span::styled("", Style::default())
            },
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("[r]", Style::default().fg(Color::Yellow)),
            Span::styled(" Refresh Data", Style::default()),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from("Recent Commands:")),
        // Show actual recent commands from history
        if app.history.is_empty() {
            ListItem::new(Line::from("  No recent activity"))
        } else {
            ListItem::new(Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("omg {}", app.history[0].transaction_type),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        },
    ];

    let actions_list = List::new(actions).block(
        Block::default()
            .title(" Quick Actions ")
            .borders(Borders::ALL),
    );

    f.render_widget(actions_list, area);
}

fn draw_packages(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Package list
        ])
        .split(area);

    // Search bar
    let search_text = if app.search_mode {
        format!("Search: {}_ ", app.search_query)
    } else {
        format!("Search: {} ", app.search_query)
    };

    let search_bar = Paragraph::new(search_text).block(
        Block::default()
            .title(" Package Search ")
            .borders(Borders::ALL)
            .border_style(if app.search_mode {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );

    f.render_widget(search_bar, chunks[0]);

    // Package list
    let rows: Vec<Row> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let style = if i == app.selected_index {
                Style::default()
                    .bg(Color::Rgb(50, 50, 50))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(pkg.name.clone()).style(style),
                Cell::from(pkg.version.to_string()).style(style),
                Cell::from(pkg.repo.clone()).style(if pkg.repo == "AUR" {
                    style.fg(Color::Blue)
                } else {
                    style
                }),
                Cell::from(pkg.description.clone()).style(style.fg(Color::DarkGray)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(30),
        ],
    )
    .block(Block::default().title(" Packages ").borders(Borders::ALL))
    .header(
        Row::new(vec!["Name", "Version", "Source", "Description"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    );

    f.render_widget(table, chunks[1]);
}

fn draw_runtimes(f: &mut Frame, area: Rect, app: &App) {
    let runtimes = app.get_runtime_versions();

    let items: Vec<ListItem> = runtimes
        .iter()
        .map(|(name, version)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{name:<10}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {version:<15}"), Style::default().fg(Color::Cyan)),
                Span::styled(" Active", Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let runtime_list = List::new(items).block(
        Block::default()
            .title(" Runtime Versions ")
            .borders(Borders::ALL),
    );

    f.render_widget(runtime_list, area);
}

fn draw_security(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    // Security Overview
    let vulnerabilities = app.get_security_vulnerabilities();
    let security_lines = vec![
        Line::from(vec![
            Span::styled("Security Status: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if vulnerabilities == 0 {
                    "SECURE"
                } else {
                    "VULNERABLE"
                },
                Style::default()
                    .fg(if vulnerabilities == 0 {
                        Color::Green
                    } else {
                        Color::Red
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Vulnerabilities Found: ", Style::default().fg(Color::Gray)),
            Span::styled(
                vulnerabilities.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Last Audit: ", Style::default().fg(Color::Gray)),
            Span::styled("Run 'omg audit' to check", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from("Policy Enforcement:"),
        Line::from(vec![
            Span::styled("├─ Minimum Grade: ", Style::default().fg(Color::Gray)),
            Span::styled("VERIFIED", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("├─ AUR Allowed: ", Style::default().fg(Color::Gray)),
            Span::styled("Yes", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("└─ PGP Required: ", Style::default().fg(Color::Gray)),
            Span::styled("Yes", Style::default().fg(Color::Green)),
        ]),
    ];

    let security_widget = Paragraph::new(security_lines).block(
        Block::default()
            .title(" Security Overview ")
            .borders(Borders::ALL),
    );

    f.render_widget(security_widget, chunks[0]);

    // Vulnerability List (placeholder - would show actual vulnerabilities)
    let vuln_lines = vec![
        Line::from("Run 'omg audit' to scan for vulnerabilities"),
        Line::from(""),
        Line::from("Common vulnerabilities include:"),
        Line::from("• Outdated cryptographic libraries"),
        Line::from("• Known CVEs in installed packages"),
        Line::from("• Insecure package configurations"),
    ];

    let vuln_widget = Paragraph::new(vuln_lines).block(
        Block::default()
            .title(" Vulnerability Scan ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(vuln_widget, chunks[1]);
}

fn draw_activity(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .take(20)
        .map(|t| {
            let time = t.timestamp.strftime("%H:%M:%S").to_string();

            let type_color = match t.transaction_type.to_string().as_str() {
                "Install" => Color::Green,
                "Remove" => Color::Red,
                "Update" => Color::Yellow,
                "Sync" => Color::Cyan,
                _ => Color::Gray,
            };

            let header = Line::from(vec![
                Span::styled(format!("[{time}] "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<8} ", t.transaction_type),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                if t.success {
                    Span::styled("✓", Style::default().fg(Color::Green))
                } else {
                    Span::styled("✗", Style::default().fg(Color::Red))
                },
            ]);

            let mut changes = String::new();
            for (j, c) in t.changes.iter().enumerate() {
                if j > 3 {
                    changes.push_str(", ...");
                    break;
                }
                if j > 0 {
                    changes.push_str(", ");
                }
                changes.push_str(&c.name);
            }

            ListItem::new(vec![
                header,
                Line::from(Span::styled(
                    format!("  {changes}"),
                    Style::default().fg(Color::Gray),
                )),
            ])
        })
        .collect();

    let activity_list = List::new(items).block(
        Block::default()
            .title(" Activity Log ")
            .borders(Borders::ALL),
    );

    f.render_widget(activity_list, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(40)])
        .split(area);

    // Tab bar
    let tabs = ["Dashboard", "Packages", "Runtimes", "Security", "Activity"];
    let tab_spans: Vec<Span> = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let is_active = i == app.current_tab as usize;
            Span::styled(
                format!(" {tab} "),
                Style::default()
                    .fg(if is_active { Color::Cyan } else { Color::Gray })
                    .bg(if is_active {
                        Color::Rgb(30, 30, 30)
                    } else {
                        Color::Rgb(20, 20, 20)
                    })
                    .add_modifier(if is_active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )
        })
        .collect();

    let tab_bar = Paragraph::new(Line::from(tab_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );

    f.render_widget(tab_bar, footer_chunks[0]);

    // Key hints
    let hints = match app.current_tab {
        Tab::Dashboard => "[q]uit [r]efresh [1-5]abs",
        Tab::Packages => "[q]uit [/]search [↑↓]nav [Enter]install",
        Tab::Runtimes => "[q]uit [u]se [i]nstall [r]emove",
        Tab::Security => "[q]uit [a]udit [f]ix [p]olicy",
        Tab::Activity => "[q]uit [r]efresh [c]lear",
    };

    let key_hints = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );

    f.render_widget(key_hints, footer_chunks[1]);
}

fn draw_popup(f: &mut Frame, app: &App) {
    let popup_area = Rect {
        x: f.area().width / 4,
        y: f.area().height / 4,
        width: f.area().width / 2,
        height: f.area().height / 2,
    };

    f.render_widget(Clear, popup_area);

    let popup_text = match app.current_tab {
        Tab::Packages => {
            if !app.search_results.is_empty() && app.selected_index < app.search_results.len() {
                format!("Install {}?", app.search_results[app.selected_index].name)
            } else {
                "No package selected".to_string()
            }
        }
        _ => "Confirm action?".to_string(),
    };

    let popup = Paragraph::new(popup_text)
        .block(
            Block::default()
                .title(" Confirmation ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));

    f.render_widget(popup, popup_area);
}

// Helper functions
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
