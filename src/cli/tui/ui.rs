//! World-class TUI for OMG Package Manager
//!
//! Inspired by bottom, lazygit, and k9s - modern, beautiful, and functional.

use crate::cli::tui::app::{App, Tab};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
    },
};

// Modern color palette (inspired by Catppuccin/Tokyo Night)
mod colors {
    use ratatui::style::Color;

    pub const BG_DARK: Color = Color::Rgb(26, 27, 38);
    pub const BG_MEDIUM: Color = Color::Rgb(36, 40, 59);
    pub const BG_LIGHT: Color = Color::Rgb(52, 59, 88);
    pub const BG_HIGHLIGHT: Color = Color::Rgb(41, 46, 66);

    pub const FG_PRIMARY: Color = Color::Rgb(192, 202, 245);
    pub const FG_SECONDARY: Color = Color::Rgb(130, 139, 184);
    pub const FG_MUTED: Color = Color::Rgb(86, 95, 137);

    pub const ACCENT_BLUE: Color = Color::Rgb(122, 162, 247);
    pub const ACCENT_CYAN: Color = Color::Rgb(125, 207, 255);
    pub const ACCENT_GREEN: Color = Color::Rgb(158, 206, 106);
    pub const ACCENT_YELLOW: Color = Color::Rgb(224, 175, 104);
    pub const ACCENT_ORANGE: Color = Color::Rgb(255, 158, 100);
    pub const ACCENT_RED: Color = Color::Rgb(247, 118, 142);
    pub const ACCENT_MAGENTA: Color = Color::Rgb(187, 154, 247);

    pub const BORDER_NORMAL: Color = Color::Rgb(61, 66, 91);
}

pub fn draw(f: &mut Frame, app: &App) {
    // Fill background
    let bg_block = Block::default().style(Style::default().bg(colors::BG_DARK));
    f.render_widget(bg_block, f.area());

    // Main layout with header, body, and footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with tabs
            Constraint::Min(0),    // Body
            Constraint::Length(2), // Status bar
        ])
        .split(f.area());

    let (Some(header), Some(body), Some(footer)) =
        (main_chunks.first(), main_chunks.get(1), main_chunks.get(2))
    else {
        return;
    };
    let (header, body, footer) = (*header, *body, *footer);

    draw_header(f, header, app);

    match app.current_tab {
        Tab::Dashboard => draw_dashboard(f, body, app),
        Tab::Packages => draw_packages(f, body, app),
        Tab::Runtimes => draw_runtimes(f, body, app),
        Tab::Security => draw_security(f, body, app),
        Tab::Activity => draw_activity(f, body, app),
        Tab::Team => draw_team(f, body, app),
    }

    draw_status_bar(f, footer, app);

    // Draw popup if active
    if app.show_popup {
        draw_popup(f, app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12), // Logo
            Constraint::Min(0),     // Tabs
            Constraint::Length(20), // Status indicators
        ])
        .split(area);

    let (Some(logo_area), Some(tabs_area), Some(status_area)) = (
        header_chunks.first(),
        header_chunks.get(1),
        header_chunks.get(2),
    ) else {
        return;
    };

    // Logo
    let logo = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "◆",
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " OMG",
            Style::default()
                .fg(colors::FG_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .style(Style::default().bg(colors::BG_MEDIUM))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colors::BORDER_NORMAL)),
    );
    f.render_widget(logo, *logo_area);

    // Tabs
    let tab_titles = vec![
        "󰕮 Dashboard",
        " Packages",
        " Runtimes",
        "󰒃 Security",
        " Activity",
        "󰃐 Team",
    ];
    let tabs = Tabs::new(tab_titles)
        .select(app.current_tab as usize)
        .style(Style::default().fg(colors::FG_MUTED))
        .highlight_style(
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(
            " │ ",
            Style::default().fg(colors::BORDER_NORMAL),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER_NORMAL))
                .style(Style::default().bg(colors::BG_MEDIUM)),
        );
    f.render_widget(tabs, *tabs_area);

    // Status indicators
    let status = Paragraph::new(Line::from(vec![
        if app.daemon_connected {
            Span::styled("● ", Style::default().fg(colors::ACCENT_GREEN))
        } else {
            Span::styled("● ", Style::default().fg(colors::ACCENT_RED))
        },
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(colors::FG_MUTED),
        ),
    ]))
    .alignment(Alignment::Right)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colors::BORDER_NORMAL))
            .style(Style::default().bg(colors::BG_MEDIUM)),
    );
    f.render_widget(status, *status_area);
}

fn draw_dashboard(f: &mut Frame, area: Rect, app: &App) {
    // Two-column layout for dashboard
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let (Some(left), Some(right)) = (main_chunks.first(), main_chunks.get(1)) else {
        return;
    };

    // Left side: System stats and metrics
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // System health cards
            Constraint::Length(10), // CPU/Memory gauges
            Constraint::Min(0),     // Network & Disk
        ])
        .split(*left);

    let (Some(l0), Some(l1), Some(l2)) =
        (left_chunks.first(), left_chunks.get(1), left_chunks.get(2))
    else {
        return;
    };

    draw_health_cards(f, *l0, app);
    draw_system_gauges(f, *l1, app);
    draw_system_info(f, *l2, app);

    // Right side: Usage stats, quick actions and activity
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Usage stats
            Constraint::Min(0),     // Quick actions
            Constraint::Length(10), // Recent activity
        ])
        .split(*right);

    let (Some(r0), Some(r1), Some(r2)) = (
        right_chunks.first(),
        right_chunks.get(1),
        right_chunks.get(2),
    ) else {
        return;
    };

    draw_usage_stats(f, *r0, app);
    draw_quick_actions(f, *r1, app);
    draw_recent_activity(f, *r2, app);
}

fn draw_usage_stats(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.usage_stats;

    let usage_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Time Saved: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                stats.time_saved_human(),
                Style::default()
                    .fg(colors::ACCENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Commands: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{}", stats.total_commands),
                Style::default().fg(colors::ACCENT_CYAN),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Today: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{}", stats.queries_today),
                Style::default().fg(colors::ACCENT_BLUE),
            ),
            Span::styled(" │ Month: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{}", stats.queries_this_month),
                Style::default().fg(colors::ACCENT_BLUE),
            ),
        ]),
    ];

    let usage_widget = Paragraph::new(usage_lines)
        .block(styled_block("󰄉 Usage Stats"))
        .style(Style::default().bg(colors::BG_MEDIUM));
    f.render_widget(usage_widget, area);
}

fn styled_block(title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {title} "))
        .title_style(
            Style::default()
                .fg(colors::FG_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colors::BORDER_NORMAL))
        .style(Style::default().bg(colors::BG_MEDIUM))
}

fn draw_health_cards(f: &mut Frame, area: Rect, app: &App) {
    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let total_packages = app.get_total_packages();
    let updates = app.get_updates_available();
    let orphans = app.get_orphan_packages();
    let vulns = app.get_security_vulnerabilities();

    // Packages card
    if let Some(c) = cards.first() {
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("󰏗 ", Style::default().fg(colors::ACCENT_BLUE)),
                Span::styled(
                    format!("{total_packages}"),
                    Style::default()
                        .fg(colors::FG_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "System Packages",
                Style::default().fg(colors::FG_MUTED),
            )),
        ])
        .alignment(Alignment::Center)
        .block(styled_block("Inventory"));
        f.render_widget(card, *c);
    }

    // Updates card
    if let Some(c) = cards.get(1) {
        let color = if updates > 0 {
            colors::ACCENT_YELLOW
        } else {
            colors::ACCENT_GREEN
        };
        let status_icon = if updates > 0 { "󰚰 " } else { "󰄬 " };
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(status_icon, Style::default().fg(color)),
                Span::styled(
                    format!("{updates}"),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                if updates > 0 {
                    "Updates Available"
                } else {
                    "System Up-to-date"
                },
                Style::default().fg(colors::FG_MUTED),
            )),
        ])
        .alignment(Alignment::Center)
        .block(styled_block("Maintainability"));
        f.render_widget(card, *c);
    }

    // Orphans card
    if let Some(c) = cards.get(2) {
        let color = if orphans > 0 {
            colors::ACCENT_ORANGE
        } else {
            colors::ACCENT_GREEN
        };
        let status_icon = if orphans > 0 { "󰃤 " } else { "󰄬 " };
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(status_icon, Style::default().fg(color)),
                Span::styled(
                    format!("{orphans}"),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "Orphan Packages",
                Style::default().fg(colors::FG_MUTED),
            )),
        ])
        .alignment(Alignment::Center)
        .block(styled_block("Hygiene"));
        f.render_widget(card, *c);
    }

    // Security card
    if let Some(c) = cards.get(3) {
        let color = if vulns == 0 {
            colors::ACCENT_GREEN
        } else {
            colors::ACCENT_RED
        };
        let status_icon = if vulns == 0 { "󰒃 " } else { "󰀦 " };
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(status_icon, Style::default().fg(color)),
                Span::styled(
                    if vulns == 0 {
                        "Secure".to_string()
                    } else {
                        format!("{vulns} CVEs")
                    },
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "Compliance Status",
                Style::default().fg(colors::FG_MUTED),
            )),
        ])
        .alignment(Alignment::Center)
        .block(styled_block("Security"));
        f.render_widget(card, *c);
    }
}

fn draw_system_gauges(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (Some(cpu_area), Some(mem_area)) = (chunks.first(), chunks.get(1)) else {
        return;
    };

    // CPU Gauge
    let cpu_percent = app.system_metrics.cpu_usage.min(100.0) as u16;
    let cpu_color = match cpu_percent {
        0..=50 => colors::ACCENT_GREEN,
        51..=80 => colors::ACCENT_YELLOW,
        _ => colors::ACCENT_RED,
    };

    let cpu_gauge = Gauge::default()
        .block(styled_block(" CPU"))
        .gauge_style(Style::default().fg(cpu_color).bg(colors::BG_LIGHT))
        .percent(cpu_percent)
        .label(format!("{:.1}%", app.system_metrics.cpu_usage));
    f.render_widget(cpu_gauge, *cpu_area);

    // Memory Gauge
    let mem_percent = app.system_metrics.memory_usage.min(100.0) as u16;
    let mem_color = match mem_percent {
        0..=60 => colors::ACCENT_GREEN,
        61..=85 => colors::ACCENT_YELLOW,
        _ => colors::ACCENT_RED,
    };

    let mem_gauge = Gauge::default()
        .block(styled_block(" Memory"))
        .gauge_style(Style::default().fg(mem_color).bg(colors::BG_LIGHT))
        .percent(mem_percent)
        .label(format!("{:.1}%", app.system_metrics.memory_usage));
    f.render_widget(mem_gauge, *mem_area);
}

fn draw_system_info(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (Some(disk_area), Some(net_area)) = (chunks.first(), chunks.get(1)) else {
        return;
    };

    // Disk info
    let disk_used_gb = app.system_metrics.disk_usage / 1024 / 1024;
    let disk_free_gb = app.system_metrics.disk_free / 1024 / 1024;
    let disk_total = disk_used_gb + disk_free_gb;
    let disk_percent = if disk_total > 0 {
        (disk_used_gb * 100 / disk_total) as u16
    } else {
        0
    };

    let disk_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Used: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{disk_used_gb} GB"),
                Style::default()
                    .fg(colors::ACCENT_BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Free: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{disk_free_gb} GB"),
                Style::default().fg(colors::ACCENT_GREEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("Usage: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                format!("{disk_percent}%"),
                Style::default().fg(if disk_percent > 90 {
                    colors::ACCENT_RED
                } else {
                    colors::FG_PRIMARY
                }),
            ),
        ]),
    ];

    let disk_widget = Paragraph::new(disk_lines).block(styled_block(" Disk"));
    f.render_widget(disk_widget, *disk_area);

    // Network info
    let net_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("↓ RX: ", Style::default().fg(colors::ACCENT_GREEN)),
            Span::styled(
                format_bytes(app.system_metrics.network_rx),
                Style::default().fg(colors::FG_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("↑ TX: ", Style::default().fg(colors::ACCENT_BLUE)),
            Span::styled(
                format_bytes(app.system_metrics.network_tx),
                Style::default().fg(colors::FG_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("Daemon: ", Style::default().fg(colors::FG_MUTED)),
            if app.daemon_connected {
                Span::styled("Connected", Style::default().fg(colors::ACCENT_GREEN))
            } else {
                Span::styled("Offline", Style::default().fg(colors::ACCENT_RED))
            },
        ]),
    ];

    let net_widget = Paragraph::new(net_lines).block(styled_block("󰛳 Network"));
    f.render_widget(net_widget, *net_area);
}

fn draw_quick_actions(f: &mut Frame, area: Rect, app: &App) {
    let updates = app.get_updates_available();
    let orphans = app.get_orphan_packages();

    let actions = vec![
        ListItem::new(Line::from(vec![
            Span::styled(
                " u ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_YELLOW),
            ),
            Span::styled(" Update System", Style::default().fg(colors::FG_PRIMARY)),
            if updates > 0 {
                Span::styled(
                    format!(" ({updates})"),
                    Style::default()
                        .fg(colors::ACCENT_YELLOW)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(" ✓", Style::default().fg(colors::ACCENT_GREEN))
            },
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                " 2 ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_CYAN),
            ),
            Span::styled(" Search Packages", Style::default().fg(colors::FG_PRIMARY)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                " c ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_MAGENTA),
            ),
            Span::styled(" Clean Cache", Style::default().fg(colors::FG_PRIMARY)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                " o ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_ORANGE),
            ),
            Span::styled(" Remove Orphans", Style::default().fg(colors::FG_PRIMARY)),
            if orphans > 0 {
                Span::styled(
                    format!(" ({orphans})"),
                    Style::default().fg(colors::ACCENT_ORANGE),
                )
            } else {
                Span::styled(" ✓", Style::default().fg(colors::ACCENT_GREEN))
            },
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                " r ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_BLUE),
            ),
            Span::styled(" Refresh", Style::default().fg(colors::FG_PRIMARY)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                " a ",
                Style::default().bg(colors::BG_LIGHT).fg(colors::ACCENT_RED),
            ),
            Span::styled(" Security Audit", Style::default().fg(colors::FG_PRIMARY)),
        ])),
    ];

    let actions_list = List::new(actions)
        .block(styled_block("󰌌 Quick Actions"))
        .style(Style::default().bg(colors::BG_MEDIUM));

    f.render_widget(actions_list, area);
}

fn draw_recent_activity(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .take(5)
        .map(|t| {
            let time = t.timestamp.strftime("%H:%M").to_string();
            let type_color = match t.transaction_type.to_string().as_str() {
                "Install" => colors::ACCENT_GREEN,
                "Remove" => colors::ACCENT_RED,
                "Update" => colors::ACCENT_YELLOW,
                "Sync" => colors::ACCENT_CYAN,
                _ => colors::FG_MUTED,
            };

            let icon = match t.transaction_type.to_string().as_str() {
                "Install" | "Remove" | "Update" => "",
                "Sync" => "󰓦",
                _ => "•",
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{time} "), Style::default().fg(colors::FG_MUTED)),
                Span::styled(format!("{icon} "), Style::default().fg(type_color)),
                Span::styled(
                    format!("{}", t.transaction_type),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                if t.success {
                    Span::styled(" ✓", Style::default().fg(colors::ACCENT_GREEN))
                } else {
                    Span::styled(" ✗", Style::default().fg(colors::ACCENT_RED))
                },
            ]))
        })
        .collect();

    let activity_list = if items.is_empty() {
        List::new(vec![ListItem::new(Line::from(Span::styled(
            "No recent activity",
            Style::default().fg(colors::FG_MUTED),
        )))])
    } else {
        List::new(items)
    }
    .block(styled_block(" Recent"))
    .style(Style::default().bg(colors::BG_MEDIUM));

    f.render_widget(activity_list, area);
}

fn draw_packages(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Package list
        ])
        .split(area);

    let (Some(search_area), Some(list_area)) = (chunks.first(), chunks.get(1)) else {
        return;
    };

    // Search bar with modern styling
    let search_text = if app.search_mode {
        format!("  {}▏", app.search_query)
    } else if app.search_query.is_empty() {
        "  Type / to search packages...".to_string()
    } else {
        format!("  {}", app.search_query)
    };

    let search_bar = Paragraph::new(Line::from(vec![Span::styled(
        search_text,
        Style::default().fg(if app.search_mode {
            colors::FG_PRIMARY
        } else {
            colors::FG_MUTED
        }),
    )]))
    .block(
        Block::default()
            .title(" 󰍉 Search ")
            .title_style(Style::default().fg(colors::FG_PRIMARY))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if app.search_mode {
                colors::ACCENT_CYAN
            } else {
                colors::BORDER_NORMAL
            }))
            .style(Style::default().bg(colors::BG_MEDIUM)),
    );
    f.render_widget(search_bar, *search_area);

    // Package table with modern styling
    let rows: Vec<Row> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let is_selected = i == app.selected_index;
            let base_style = if is_selected {
                Style::default().bg(colors::BG_HIGHLIGHT)
            } else {
                Style::default()
            };

            let source_color = if pkg.repo == "AUR" {
                colors::ACCENT_MAGENTA
            } else {
                colors::ACCENT_BLUE
            };

            Row::new(vec![
                Cell::from(Span::styled(
                    pkg.name.clone(),
                    base_style
                        .fg(colors::FG_PRIMARY)
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                )),
                Cell::from(Span::styled(
                    #[allow(clippy::implicit_clone)]
                    pkg.version.to_string(),
                    base_style.fg(colors::ACCENT_GREEN),
                )),
                Cell::from(Span::styled(pkg.repo.clone(), base_style.fg(source_color))),
                Cell::from(Span::styled(
                    pkg.description.chars().take(50).collect::<String>(),
                    base_style.fg(colors::FG_MUTED),
                )),
            ])
            .style(base_style)
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "Name",
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Version",
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Source",
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Description",
            Style::default()
                .fg(colors::ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .style(Style::default().bg(colors::BG_LIGHT));

    let table = Table::new(
        rows,
        [
            Constraint::Min(25),
            Constraint::Length(15),
            Constraint::Length(12),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(styled_block(" Packages"))
    .row_highlight_style(Style::default().bg(colors::BG_HIGHLIGHT));

    f.render_widget(table, *list_area);
}

fn draw_runtimes(f: &mut Frame, area: Rect, app: &App) {
    let runtimes = app.get_runtime_versions();

    let runtime_icons: std::collections::HashMap<&str, &str> = [
        ("node", ""),
        ("python", ""),
        ("rust", ""),
        ("go", ""),
        ("ruby", ""),
        ("java", ""),
        ("bun", "󰟈"),
        ("deno", "󰛦"),
        ("zig", ""),
    ]
    .into_iter()
    .collect();

    let items: Vec<ListItem> = runtimes
        .iter()
        .map(|(name, version)| {
            let icon = runtime_icons
                .get(name.to_lowercase().as_str())
                .unwrap_or(&"󰏗");
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {icon} "),
                    Style::default().fg(colors::ACCENT_CYAN),
                ),
                Span::styled(
                    format!("{name:<12}"),
                    Style::default()
                        .fg(colors::FG_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{version:<15}"),
                    Style::default().fg(colors::ACCENT_GREEN),
                ),
                Span::styled("● Active", Style::default().fg(colors::ACCENT_GREEN)),
            ]))
        })
        .collect();

    let runtime_list = if items.is_empty() {
        List::new(vec![ListItem::new(Line::from(Span::styled(
            "No runtimes detected. Use 'omg use <runtime> <version>' to install.",
            Style::default().fg(colors::FG_MUTED),
        )))])
    } else {
        List::new(items)
    }
    .block(styled_block(" Runtimes"))
    .style(Style::default().bg(colors::BG_MEDIUM));

    f.render_widget(runtime_list, area);
}

fn draw_security(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (Some(left), Some(right)) = (chunks.first(), chunks.get(1)) else {
        return;
    };

    let vulnerabilities = app.get_security_vulnerabilities();
    let status_color = if vulnerabilities == 0 {
        colors::ACCENT_GREEN
    } else {
        colors::ACCENT_RED
    };
    let status_icon = if vulnerabilities == 0 { "󰒃" } else { "󰀦" };
    let status_text = if vulnerabilities == 0 {
        "SECURE"
    } else {
        "VULNERABLE"
    };

    // Security Overview
    let security_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" {status_icon} "),
                Style::default().fg(status_color),
            ),
            Span::styled("Status: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   CVEs Found: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled(
                vulnerabilities.to_string(),
                Style::default()
                    .fg(if vulnerabilities > 0 {
                        colors::ACCENT_RED
                    } else {
                        colors::ACCENT_GREEN
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "   Policy:",
            Style::default()
                .fg(colors::FG_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("   ├─ Min Grade: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled("VERIFIED", Style::default().fg(colors::ACCENT_GREEN)),
        ]),
        Line::from(vec![
            Span::styled("   ├─ AUR: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled("Allowed", Style::default().fg(colors::ACCENT_YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("   └─ PGP: ", Style::default().fg(colors::FG_MUTED)),
            Span::styled("Required", Style::default().fg(colors::ACCENT_GREEN)),
        ]),
    ];

    let security_widget = Paragraph::new(security_lines)
        .block(styled_block("󰒃 Security Status"))
        .style(Style::default().bg(colors::BG_MEDIUM));
    f.render_widget(security_widget, *left);

    // Actions panel
    let action_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " a ",
                Style::default().bg(colors::BG_LIGHT).fg(colors::ACCENT_RED),
            ),
            Span::styled(
                " Run Security Audit",
                Style::default().fg(colors::FG_PRIMARY),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " f ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_YELLOW),
            ),
            Span::styled(
                " Fix Vulnerabilities",
                Style::default().fg(colors::FG_PRIMARY),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " p ",
                Style::default()
                    .bg(colors::BG_LIGHT)
                    .fg(colors::ACCENT_BLUE),
            ),
            Span::styled(" Edit Policy", Style::default().fg(colors::FG_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Press 'a' to scan for vulnerabilities",
            Style::default().fg(colors::FG_MUTED),
        )),
    ];

    let actions_widget = Paragraph::new(action_lines)
        .block(styled_block("󰌌 Actions"))
        .style(Style::default().bg(colors::BG_MEDIUM));
    f.render_widget(actions_widget, *right);
}

fn draw_activity(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .take(20)
        .map(|t| {
            let time = t.timestamp.strftime("%H:%M:%S").to_string();

            let type_color = match t.transaction_type.to_string().as_str() {
                "Install" => colors::ACCENT_GREEN,
                "Remove" => colors::ACCENT_RED,
                "Update" => colors::ACCENT_YELLOW,
                "Sync" => colors::ACCENT_CYAN,
                _ => colors::FG_MUTED,
            };

            let icon = match t.transaction_type.to_string().as_str() {
                "Install" | "Remove" | "Update" => "",
                "Sync" => "󰓦",
                _ => "•",
            };

            let header = Line::from(vec![
                Span::styled(format!(" {time} "), Style::default().fg(colors::FG_MUTED)),
                Span::styled(format!("{icon} "), Style::default().fg(type_color)),
                Span::styled(
                    format!("{:<8}", t.transaction_type),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                if t.success {
                    Span::styled(" ✓", Style::default().fg(colors::ACCENT_GREEN))
                } else {
                    Span::styled(" ✗", Style::default().fg(colors::ACCENT_RED))
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
                    format!("    {changes}"),
                    Style::default().fg(colors::FG_SECONDARY),
                )),
            ])
        })
        .collect();

    let activity_list = if items.is_empty() {
        List::new(vec![ListItem::new(Line::from(Span::styled(
            "No activity recorded yet",
            Style::default().fg(colors::FG_MUTED),
        )))])
    } else {
        List::new(items)
    }
    .block(styled_block(" Activity Log"))
    .style(Style::default().bg(colors::BG_MEDIUM));

    f.render_widget(activity_list, area);
}

fn draw_team(f: &mut Frame, area: Rect, app: &App) {
    if let Some(status) = &app.team_status {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let (Some(left), Some(right)) = (chunks.first(), chunks.get(1)) else {
            return;
        };

        // Team Info
        let info_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Team: ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    &status.config.name,
                    Style::default()
                        .fg(colors::ACCENT_CYAN)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ID: ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    &status.config.team_id,
                    Style::default().fg(colors::FG_PRIMARY),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Remote: ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    status.config.remote_url.as_deref().unwrap_or("None"),
                    Style::default().fg(colors::ACCENT_BLUE),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Lock Hash: ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    if status.lock_hash.is_empty() {
                        "none"
                    } else {
                        &status.lock_hash[..8]
                    },
                    Style::default().fg(colors::FG_PRIMARY),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Status: ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    if status.in_sync_count() == status.members.len() {
                        "All Systems Operational"
                    } else {
                        "Drift Detected"
                    },
                    Style::default().fg(if status.in_sync_count() == status.members.len() {
                        colors::ACCENT_GREEN
                    } else {
                        colors::ACCENT_YELLOW
                    }),
                ),
            ]),
        ];

        let info_widget = Paragraph::new(info_lines)
            .block(styled_block("󰃐 Team Info"))
            .style(Style::default().bg(colors::BG_MEDIUM));
        f.render_widget(info_widget, *left);

        // Members List
        let rows: Vec<Row> = status
            .members
            .iter()
            .map(|member| {
                let status_color = if member.in_sync {
                    colors::ACCENT_GREEN
                } else {
                    colors::ACCENT_YELLOW
                };

                Row::new(vec![
                    Cell::from(Span::styled(
                        if member.in_sync { "✓" } else { "⚠" },
                        Style::default().fg(status_color),
                    )),
                    Cell::from(Span::styled(
                        &member.name,
                        Style::default()
                            .fg(colors::FG_PRIMARY)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(Span::styled(
                        &member.id,
                        Style::default().fg(colors::FG_MUTED),
                    )),
                    Cell::from(Span::styled(
                        if member.in_sync {
                            "Synced".to_string()
                        } else {
                            member.drift_summary.clone().unwrap_or_default()
                        },
                        Style::default().fg(status_color),
                    )),
                ])
                .style(Style::default().bg(colors::BG_MEDIUM))
            })
            .collect();

        let header = Row::new(vec![
            Cell::from(""),
            Cell::from(Span::styled(
                "Name",
                Style::default()
                    .fg(colors::ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "ID",
                Style::default()
                    .fg(colors::ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Status",
                Style::default()
                    .fg(colors::ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .style(Style::default().bg(colors::BG_LIGHT));

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Percentage(30),
                Constraint::Percentage(20),
                Constraint::Percentage(40),
            ],
        )
        .header(header)
        .block(styled_block(" Members"))
        .row_highlight_style(Style::default().bg(colors::BG_HIGHLIGHT));

        f.render_widget(table, *right);
    } else {
        // No team workspace
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Not in a team workspace",
                Style::default()
                    .fg(colors::ACCENT_RED)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Run ", Style::default().fg(colors::FG_MUTED)),
                Span::styled(
                    "omg team init <team-id>",
                    Style::default().fg(colors::ACCENT_CYAN),
                ),
                Span::styled(" to get started.", Style::default().fg(colors::FG_MUTED)),
            ]),
        ];

        let widget = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(styled_block("󰃐 Team Dashboard"))
            .style(Style::default().bg(colors::BG_MEDIUM));
        f.render_widget(widget, area);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    // Key hints based on current tab
    let hints = match app.current_tab {
        Tab::Dashboard => vec![
            ("q", "Quit"),
            ("u", "Update"),
            ("c", "Clean"),
            ("r", "Refresh"),
            ("1-5", "Tabs"),
        ],
        Tab::Packages => vec![
            ("q", "Quit"),
            ("/", "Search"),
            ("↑↓", "Navigate"),
            ("Enter", "Install"),
            ("Esc", "Cancel"),
        ],
        Tab::Runtimes => vec![
            ("q", "Quit"),
            ("u", "Use"),
            ("i", "Install"),
            ("r", "Remove"),
        ],
        Tab::Security => vec![("q", "Quit"), ("a", "Audit"), ("f", "Fix"), ("p", "Policy")],
        Tab::Activity => vec![("q", "Quit"), ("r", "Refresh"), ("c", "Clear")],
        Tab::Team => vec![
            ("q", "Quit"),
            ("r", "Refresh"),
            ("p", "Pull"),
            ("P", "Push"),
        ],
    };

    let mut spans = vec![Span::styled(" ", Style::default())];
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " │ ",
                Style::default().fg(colors::BORDER_NORMAL),
            ));
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .bg(colors::BG_LIGHT)
                .fg(colors::ACCENT_CYAN),
        ));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(colors::FG_MUTED),
        ));
    }

    let status_bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(colors::BG_DARK));

    f.render_widget(status_bar, area);
}

fn draw_popup(f: &mut Frame, app: &App) {
    let popup_width = 50.min(f.area().width.saturating_sub(4));
    let popup_height = 10.min(f.area().height.saturating_sub(4));
    let popup_area = Rect {
        x: (f.area().width.saturating_sub(popup_width)) / 2,
        y: (f.area().height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let (title, content) = match app.current_tab {
        Tab::Packages => {
            if !app.search_results.is_empty() && app.selected_index < app.search_results.len() {
                let pkg = app.search_results.get(app.selected_index);
                let name = pkg.map_or("package", |p| p.name.as_str());
                (
                    "󰏗 Install Package",
                    vec![
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("Install ", Style::default().fg(colors::FG_PRIMARY)),
                            Span::styled(
                                name,
                                Style::default()
                                    .fg(colors::ACCENT_CYAN)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("?", Style::default().fg(colors::FG_PRIMARY)),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled(
                                " Enter ",
                                Style::default()
                                    .bg(colors::ACCENT_GREEN)
                                    .fg(colors::BG_DARK),
                            ),
                            Span::styled(" Confirm  ", Style::default().fg(colors::FG_MUTED)),
                            Span::styled(
                                " Esc ",
                                Style::default().bg(colors::ACCENT_RED).fg(colors::BG_DARK),
                            ),
                            Span::styled(" Cancel", Style::default().fg(colors::FG_MUTED)),
                        ]),
                    ],
                )
            } else {
                ("󰀨 No Selection", vec![Line::from("No package selected")])
            }
        }
        _ => (
            "󰀨 Confirm",
            vec![
                Line::from(""),
                Line::from("Confirm this action?"),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " Enter ",
                        Style::default()
                            .bg(colors::ACCENT_GREEN)
                            .fg(colors::BG_DARK),
                    ),
                    Span::styled(" Yes  ", Style::default().fg(colors::FG_MUTED)),
                    Span::styled(
                        " Esc ",
                        Style::default().bg(colors::ACCENT_RED).fg(colors::BG_DARK),
                    ),
                    Span::styled(" No", Style::default().fg(colors::FG_MUTED)),
                ]),
            ],
        ),
    };

    let popup = Paragraph::new(content).alignment(Alignment::Center).block(
        Block::default()
            .title(format!(" {title} "))
            .title_style(
                Style::default()
                    .fg(colors::ACCENT_YELLOW)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colors::ACCENT_YELLOW))
            .style(Style::default().bg(colors::BG_MEDIUM)),
    );

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

    let unit = UNITS.get(unit_index).unwrap_or(&"B");
    if unit_index == 0 {
        format!("{bytes} {unit}")
    } else {
        format!("{size:.1} {unit}")
    }
}
