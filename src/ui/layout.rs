//! Main layout and UI coordination.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::app::App;
use super::dialogs::{render_help, render_kill_confirm, render_status};
use super::footer::render_footer;
use super::gpu::render_gpu_panel;
use super::header::render_header;
use super::system::render_system_panel;

/// Main UI rendering function.
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    // Handle kill confirmation dialog first (modal)
    if app.kill_confirm.is_some() {
        render_kill_confirm(frame, frame.area(), app);
        return;
    }

    if app.show_help {
        render_help(frame, frame.area());
        return;
    }

    // Main layout - add extra row for status message if present
    let has_status = app.status_message.is_some();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_status {
            vec![
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Status
                Constraint::Length(1), // Footer
            ]
        } else {
            vec![
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Footer
            ]
        })
        .split(frame.area());

    render_header(frame, main_chunks[0], app);

    if has_status {
        render_status(frame, main_chunks[2], app);
        render_footer(frame, main_chunks[3], app);
    } else {
        render_footer(frame, main_chunks[2], app);
    }

    // Content area layout - always split to show both system and GPU panels
    let content_area = main_chunks[1];

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(content_area);

    render_system_panel(frame, h_chunks[0], app);
    render_gpu_panel(frame, h_chunks[1], app);
}

/// Create a centered rectangle for dialogs.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
