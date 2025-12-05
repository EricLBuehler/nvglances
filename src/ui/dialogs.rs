//! Dialog rendering (help, kill confirmation, status).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use sysinfo::Signal;

use crate::app::App;
use super::layout::centered_rect;

/// Render the status message bar.
pub fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    if let Some((msg, _)) = &app.status_message {
        let status = Line::from(vec![
            Span::styled(
                " STATUS: ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {} ", msg), Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(status), area);
    }
}

/// Render the kill confirmation dialog.
pub fn render_kill_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref confirm) = app.kill_confirm else {
        return;
    };

    let signal_name = match confirm.signal {
        Signal::Kill => "SIGKILL (force)",
        Signal::Term => "SIGTERM (graceful)",
        Signal::Interrupt => "SIGINT (interrupt)",
        _ => "signal",
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Kill process?",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  PID: "),
            Span::styled(
                format!("{}", confirm.pid),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Name: "),
            Span::styled(&confirm.name, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  Signal: "),
            Span::styled(signal_name, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [Y]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes, kill it   "),
            Span::styled(
                "[N]",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No, cancel"),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Confirm Kill")
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

    let confirm_area = centered_rect(40, 40, area);

    frame.render_widget(Clear, confirm_area);
    frame.render_widget(paragraph, confirm_area);
}

/// Render the help dialog.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![
            Span::styled(
                "nvglances",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - System and GPU Monitor"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab          Switch between CPU and GPU process panels"),
        Line::from("  j/↓          Move selection down"),
        Line::from("  k/↑          Move selection up"),
        Line::from("  PgDn/PgUp    Move selection by page"),
        Line::from("  Home/End     Jump to first/last item"),
        Line::from("  Mouse        Click to select, scroll to navigate"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Process Control:",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Red),
        )]),
        Line::from("  Del/Ctrl-T   Send SIGTERM (graceful termination)"),
        Line::from("  Ctrl-K       Send SIGKILL (force kill)"),
        Line::from("  Ctrl-I       Send SIGINT (interrupt)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sorting:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  1            Sort by PID"),
        Line::from("  2            Sort by Name"),
        Line::from("  3            Sort by User"),
        Line::from("  4            Sort by CPU%"),
        Line::from("  5            Sort by Memory%"),
        Line::from("  6            Sort by GPU Memory"),
        Line::from("  r            Reverse sort order"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Display:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  a            Toggle show all processes"),
        Line::from("  g            Toggle graphs"),
        Line::from("  c            Toggle compact mode"),
        Line::from("  +/-          Adjust refresh rate"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Other:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ?/F1         Show this help"),
        Line::from("  q/Esc        Quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    // Center the help window
    let help_area = centered_rect(60, 80, area);

    // Clear the area first
    frame.render_widget(Clear, help_area);
    frame.render_widget(paragraph, help_area);
}
