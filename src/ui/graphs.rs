//! History graph rendering for CPU and GPU metrics.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    symbols,
    text::Line,
    widgets::{Axis, Block, Borders, Chart, Dataset},
    Frame,
};

use crate::app::App;
use crate::types::GpuBackend;

/// Render CPU and memory history graph.
pub fn render_cpu_mem_graph(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_data: Vec<(f64, f64)> = app
        .history
        .cpu_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let mem_data: Vec<(f64, f64)> = app
        .history
        .memory_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let datasets = vec![
        Dataset::default()
            .name("CPU")
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&cpu_data),
        Dataset::default()
            .name("MEM")
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(Color::Magenta))
            .data(&mem_data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("CPU History (CPU=cyan, MEM=magenta)"),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, 59.0])
                .labels::<Vec<Line>>(vec![]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 100.0])
                .labels(vec![Line::from("0"), Line::from("50"), Line::from("100")]),
        );

    frame.render_widget(chart, area);
}

/// Render GPU utilization history graph.
pub fn render_gpu_graphs(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else {
        return;
    };
    if gpu_metrics.gpus.is_empty() {
        return;
    }

    let is_metal = gpu_metrics.backend == GpuBackend::Metal;
    let mut datasets = Vec::new();
    let colors = [Color::Cyan, Color::Magenta, Color::Green, Color::Yellow];

    if is_metal {
        // On Metal, show memory usage history instead of GPU utilization
        let mem_data: Vec<Vec<(f64, f64)>> = app
            .history
            .gpu_mem_history
            .iter()
            .map(|h| h.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect())
            .collect();

        for (i, data) in mem_data.iter().enumerate() {
            if i >= 4 {
                break;
            }
            datasets.push(
                Dataset::default()
                    .name(format!("GPU{}", i))
                    .marker(symbols::Marker::Braille)
                    .graph_type(ratatui::widgets::GraphType::Line)
                    .style(Style::default().fg(colors[i % colors.len()]))
                    .data(data),
            );
        }

        let gpu_legend: Vec<String> = (0..mem_data.len().min(4))
            .map(|i| {
                let color_name = match i {
                    0 => "cyan",
                    1 => "magenta",
                    2 => "green",
                    3 => "yellow",
                    _ => "?",
                };
                format!("GPU{}={}", i, color_name)
            })
            .collect();
        let legend = gpu_legend.join(", ");

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("GPU Memory History ({})", legend)),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, 59.0])
                    .labels::<Vec<Line>>(vec![]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 100.0])
                    .labels(vec![Line::from("0"), Line::from("50"), Line::from("100")]),
            );

        frame.render_widget(chart, area);
    } else {
        // On NVML, show GPU utilization history
        let util_data: Vec<Vec<(f64, f64)>> = app
            .history
            .gpu_util_history
            .iter()
            .map(|h| h.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect())
            .collect();

        for (i, data) in util_data.iter().enumerate() {
            if i >= 4 {
                break;
            }
            datasets.push(
                Dataset::default()
                    .name(format!("GPU{}", i))
                    .marker(symbols::Marker::Braille)
                    .graph_type(ratatui::widgets::GraphType::Line)
                    .style(Style::default().fg(colors[i % colors.len()]))
                    .data(data),
            );
        }

        let gpu_legend: Vec<String> = (0..util_data.len().min(4))
            .map(|i| {
                let color_name = match i {
                    0 => "cyan",
                    1 => "magenta",
                    2 => "green",
                    3 => "yellow",
                    _ => "?",
                };
                format!("GPU{}={}", i, color_name)
            })
            .collect();
        let legend = gpu_legend.join(", ");

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("GPU History ({})", legend)),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, 59.0])
                    .labels::<Vec<Line>>(vec![]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 100.0])
                    .labels(vec![Line::from("0"), Line::from("50"), Line::from("100")]),
            );

        frame.render_widget(chart, area);
    }
}
