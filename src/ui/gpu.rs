//! GPU panel rendering.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::types::GpuInfo;
use crate::utils::{create_bar, temp_color, usage_color};
use super::processes::render_gpu_processes;
use super::graphs::render_gpu_graphs;

/// Render the GPU panel (or no-GPU message if no GPU available).
pub fn render_gpu_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else {
        render_no_gpu_panel(frame, area);
        return;
    };

    let height = area.height as i32;
    let width = area.width as i32;
    let gpu_count = gpu_metrics.gpus.len();

    // Auto-compact if terminal is very small
    let auto_compact = height < 15 || width < 50;
    let use_compact = app.compact_mode || auto_compact;

    // Adaptive GPU card height based on available space
    let gpu_height = if use_compact {
        1
    } else if height < 20 {
        3
    } else {
        5
    };

    let show_graphs_actual = app.show_graphs && height >= 12;
    let graph_height = if height >= 25 { 6 } else { 4 };

    // Calculate how many GPU cards we can show
    let reserved_height = 5 + if show_graphs_actual { graph_height } else { 0 };
    let available_for_gpus = (height - reserved_height).max(gpu_height as i32) as usize;
    let max_gpus_to_show = (available_for_gpus / gpu_height).max(1);
    let gpus_to_show = gpu_count.min(max_gpus_to_show);
    let total_gpu_height = gpu_height * gpus_to_show;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if show_graphs_actual {
            vec![
                Constraint::Length(total_gpu_height as u16),
                Constraint::Length(graph_height as u16),
                Constraint::Min(3),
            ]
        } else {
            vec![
                Constraint::Length(total_gpu_height as u16),
                Constraint::Min(3),
            ]
        })
        .split(area);

    render_gpu_cards_limited(frame, chunks[0], app, gpus_to_show, use_compact);

    let mut chunk_idx = 1;
    if show_graphs_actual {
        render_gpu_graphs(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
    }

    render_gpu_processes(frame, chunks[chunk_idx], app);
}

/// Render the "no GPU detected" message panel.
pub fn render_no_gpu_panel(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "No NVIDIA GPU Detected",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Possible reasons:",
            Style::default().fg(Color::Cyan),
        )]),
        Line::from("  • No NVIDIA GPU installed"),
        Line::from("  • NVIDIA drivers not installed"),
        Line::from("  • NVML library not available"),
        Line::from("  • GPU in use by another process exclusively"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "To install NVIDIA drivers:",
            Style::default().fg(Color::Cyan),
        )]),
        Line::from("  Ubuntu/Debian: sudo apt install nvidia-driver-XXX"),
        Line::from("  Fedora: sudo dnf install akmod-nvidia"),
        Line::from("  Arch: sudo pacman -S nvidia"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "System monitoring is fully functional.",
            Style::default().fg(Color::Green),
        )]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("GPU Panel")
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Render GPU cards with a limit on number shown.
fn render_gpu_cards_limited(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    max_gpus: usize,
    compact: bool,
) {
    let Some(ref gpu_metrics) = app.gpu_metrics else {
        return;
    };

    let gpu_count = gpu_metrics.gpus.len().min(max_gpus);
    if gpu_count == 0 {
        return;
    }

    let height_per_gpu = area.height as usize / gpu_count;
    let constraints: Vec<Constraint> = (0..gpu_count)
        .map(|_| Constraint::Length(height_per_gpu as u16))
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, gpu) in gpu_metrics.gpus.iter().take(gpu_count).enumerate() {
        if i >= chunks.len() {
            break;
        }
        render_gpu_card(frame, chunks[i], gpu, compact);
    }
}

/// Render a single GPU card.
pub fn render_gpu_card(frame: &mut Frame, area: Rect, gpu: &GpuInfo, compact: bool) {
    let gpu_pct = gpu.gpu_utilization as f64;
    let mem_pct = if gpu.memory_total > 0 {
        (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0
    } else {
        0.0
    };

    let card_height = area.height;

    if compact || card_height <= 1 {
        // Single line compact mode
        let gpu_bar = create_bar(gpu_pct, 10);
        let mem_bar = create_bar(mem_pct, 10);

        let text = Line::from(vec![
            Span::styled(format!("GPU{} ", gpu.index), Style::default().fg(Color::Cyan)),
            Span::styled(gpu_bar, Style::default().fg(usage_color(gpu_pct))),
            Span::raw(format!(" {:3}%", gpu.gpu_utilization)),
            Span::raw(" "),
            Span::styled("MEM ", Style::default().fg(Color::Magenta)),
            Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
            Span::raw(format!(" {:3}%", mem_pct as u32)),
            Span::raw(format!(" {}°C {}W", gpu.temperature, gpu.power_usage)),
        ]);

        frame.render_widget(Paragraph::new(text), area);
    } else if card_height <= 3 {
        // Minimal mode with border
        let title = format!("GPU {} - {} [{}]", gpu.index, gpu.name, gpu.pstate);
        let gpu_bar = create_bar(gpu_pct, 12);
        let mem_bar = create_bar(mem_pct, 12);

        let line = Line::from(vec![
            Span::styled(gpu_bar, Style::default().fg(usage_color(gpu_pct))),
            Span::raw(format!(" {:3}% ", gpu.gpu_utilization)),
            Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
            Span::raw(format!(" {:3}% ", mem_pct as u32)),
            Span::styled(
                format!("{}°C ", gpu.temperature),
                Style::default().fg(temp_color(gpu.temperature)),
            ),
            Span::raw(format!("{}W", gpu.power_usage)),
        ]);

        let block = Block::default().borders(Borders::ALL).title(title);
        let paragraph = Paragraph::new(line).block(block);
        frame.render_widget(paragraph, area);
    } else {
        // Full mode
        let title = format!("GPU {} - {} [{}]", gpu.index, gpu.name, gpu.pstate);

        let gpu_bar = create_bar(gpu_pct, 20);
        let mem_bar = create_bar(mem_pct, 20);

        let lines = vec![
            Line::from(vec![
                Span::styled("GPU  ", Style::default().fg(Color::Cyan)),
                Span::styled(gpu_bar, Style::default().fg(usage_color(gpu_pct))),
                Span::raw(format!(" {:3}%  ", gpu.gpu_utilization)),
                Span::styled("Temp: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}°C", gpu.temperature),
                    Style::default().fg(temp_color(gpu.temperature)),
                ),
                Span::raw("  "),
                Span::styled("Fan: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}%", gpu.fan_speed)),
            ]),
            Line::from(vec![
                Span::styled("MEM  ", Style::default().fg(Color::Magenta)),
                Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
                Span::raw(format!(" {:3}%  ", mem_pct as u32)),
                Span::raw(format!(
                    "{} / {}",
                    format_size(gpu.memory_used, BINARY),
                    format_size(gpu.memory_total, BINARY)
                )),
            ]),
            Line::from(vec![
                Span::styled("Power: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}W / {}W  ", gpu.power_usage, gpu.power_limit)),
                Span::styled("Clocks: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{} MHz / {} MHz  ", gpu.sm_clock, gpu.mem_clock)),
                Span::styled("Enc/Dec: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!(
                    "{}% / {}%",
                    gpu.encoder_utilization, gpu.decoder_utilization
                )),
            ]),
        ];

        let block = Block::default().borders(Borders::ALL).title(title);
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
