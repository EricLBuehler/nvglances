//! Footer bar rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

/// Render the footer bar with keyboard shortcuts.
pub fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let refresh_ms = app.refresh_rate.as_millis();

    let footer = Line::from(vec![
        Span::styled(
            " ?",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Help "),
        Span::styled(
            "Del",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Kill "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Switch "),
        Span::styled(
            "1-6",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Sort "),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":All "),
        Span::styled(
            "g",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Graphs "),
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Compact "),
        Span::styled(
            "+/-",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(":{}ms ", refresh_ms)),
        Span::styled(
            "q",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Quit"),
    ]);

    frame.render_widget(Paragraph::new(footer), area);
}
