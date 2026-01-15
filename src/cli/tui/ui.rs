use crate::cli::tui::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    draw_header(f, chunks[0]);
    draw_body(f, chunks[1], app);
    draw_footer(f, chunks[2]);
}

fn draw_header(f: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " OMG ",
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Dashboard ", Style::default().add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Status
            Constraint::Percentage(60), // History
        ])
        .split(area);

    draw_status(f, body_chunks[0], app);
    draw_history(f, body_chunks[1], app);
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();

    if let Some(status) = &app.status {
        lines.push(Line::from(vec![
            Span::raw("Updates: "),
            if status.updates_available > 0 {
                Span::styled(
                    format!("{} available", status.updates_available),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::styled("Up to date", Style::default().fg(Color::Green))
            },
        ]));

        lines.push(Line::from(vec![
            Span::raw("Packages: "),
            Span::styled(
                format!("{}", status.total_packages),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::raw("CVEs: "),
            if status.security_vulnerabilities > 0 {
                Span::styled(
                    format!("{}", status.security_vulnerabilities),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("None", Style::default().fg(Color::Green))
            },
        ]));

        lines.push(Line::from("\n"));
        lines.push(Line::from(Span::styled(
            "Runtimes:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for (name, version) in &status.runtime_versions {
            lines.push(Line::from(vec![
                Span::raw(format!("  • {name:<8} ")),
                Span::styled(version, Style::default().fg(Color::Magenta)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Loading status...",
            Style::default().fg(Color::Gray),
        )));
    }

    let status_block = Paragraph::new(lines).block(
        Block::default()
            .title(" System Status ")
            .borders(Borders::ALL),
    );
    f.render_widget(status_block, area);
}

fn draw_history(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .map(|t| {
            let time = t.timestamp.format("%H:%M:%S").to_string();
            let _status_color = if t.success { Color::Green } else { Color::Red };

            let header = Line::from(vec![
                Span::styled(format!("[{time}] "), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:?} ", t.transaction_type),
                    Style::default().fg(Color::Yellow),
                ),
                if t.success {
                    Span::styled("✓", Style::default().fg(Color::Green))
                } else {
                    Span::styled("✗", Style::default().fg(Color::Red))
                },
            ]);

            let mut changes = String::new();
            for (i, c) in t.changes.iter().enumerate() {
                if i > 2 {
                    changes.push_str(", ...");
                    break;
                }
                if i > 0 {
                    changes.push_str(", ");
                }
                changes.push_str(&c.name);
            }

            ListItem::new(vec![
                header,
                Line::from(Span::styled(
                    format!("  {changes}"),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let history_list = List::new(items).block(
        Block::default()
            .title(" Recent Activity ")
            .borders(Borders::ALL),
    );
    f.render_widget(history_list, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![Span::raw(
        " [q] Quit  [r] Refresh  [Tab] Switch View ",
    )]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}
