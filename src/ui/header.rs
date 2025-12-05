//! Header bar rendering.

use chrono::Local;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;
use crate::types::GpuBackend;
use crate::utils::format_duration;

/// Render the header bar with system and GPU info.
pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let uptime = format_duration(app.system_metrics.uptime);
    let now = Local::now();

    let gpu_info = if let Some(ref gm) = app.gpu_metrics {
        let api_label = match gm.backend {
            GpuBackend::Nvml => "CUDA",
            GpuBackend::Metal => "API",
            GpuBackend::None => "GPU",
        };
        format!(
            " | Driver: {} | {}: {}",
            gm.driver_version, api_label, gm.api_version
        )
    } else {
        String::new()
    };

    let header = Line::from(vec![
        Span::styled(
            "nvglances",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            &app.system_metrics.hostname,
            Style::default().fg(Color::Green),
        ),
        Span::raw(" | "),
        Span::styled(
            &app.system_metrics.os_name,
            Style::default().fg(Color::Blue),
        ),
        Span::raw(" | "),
        Span::styled(format!("up {}", uptime), Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled(
            format!(
                "Load: {:.2} {:.2} {:.2}",
                app.system_metrics.load_avg.0,
                app.system_metrics.load_avg.1,
                app.system_metrics.load_avg.2
            ),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(gpu_info, Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(
            now.format("%H:%M:%S").to_string(),
            Style::default().fg(Color::White),
        ),
    ]);

    frame.render_widget(Paragraph::new(header), area);
}
