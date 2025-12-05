//! System panel rendering (CPU, memory, disk, network gauges).

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
    Frame,
};

use super::graphs::render_cpu_mem_graph;
use super::processes::render_cpu_processes;
use crate::app::App;
use crate::utils::{create_bar, usage_color};

/// Render the system panel with CPU, memory, network, disk info.
pub fn render_system_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let height = area.height as i32;
    let width = area.width as i32;

    // Auto-compact if terminal is very small
    let auto_compact = height < 15 || width < 60;
    let use_compact = app.compact_mode || auto_compact;

    let show_swap = height >= 14 && !use_compact;
    let show_network = height >= 18 && !use_compact;
    let show_disk = height >= 22 && !use_compact;
    let show_graphs_actual = app.show_graphs && height >= 12;
    let graph_height = if height >= 28 { 6 } else { 4 };

    if use_compact {
        let mut constraints = vec![
            Constraint::Length(1), // CPU + Memory compact
            Constraint::Length(1), // Network compact
        ];
        if show_graphs_actual {
            constraints.push(Constraint::Length(graph_height as u16));
        }
        constraints.push(Constraint::Min(3)); // CPU Processes

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut idx = 0;
        render_compact_cpu_mem(frame, chunks[idx], app);
        idx += 1;
        render_compact_network(frame, chunks[idx], app);
        idx += 1;
        if show_graphs_actual {
            render_cpu_mem_graph(frame, chunks[idx], app);
            idx += 1;
        }
        render_cpu_processes(frame, chunks[idx], app);
    } else {
        let mut constraints = vec![
            Constraint::Length(3), // CPU gauge
            Constraint::Length(3), // Memory gauge
        ];

        if show_swap {
            constraints.push(Constraint::Length(3));
        }
        if show_graphs_actual {
            constraints.push(Constraint::Length(graph_height as u16));
        }
        if show_network {
            constraints.push(Constraint::Length(4));
        }
        if show_disk {
            constraints.push(Constraint::Length(4));
        }
        constraints.push(Constraint::Min(3)); // CPU Processes

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut chunk_idx = 0;

        render_cpu_gauge(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
        render_memory_gauge(frame, chunks[chunk_idx], app);
        chunk_idx += 1;

        if show_swap {
            render_swap_gauge(frame, chunks[chunk_idx], app);
            chunk_idx += 1;
        }

        if show_graphs_actual {
            render_cpu_mem_graph(frame, chunks[chunk_idx], app);
            chunk_idx += 1;
        }

        if show_network {
            render_network(frame, chunks[chunk_idx], app);
            chunk_idx += 1;
        }

        if show_disk {
            render_disk(frame, chunks[chunk_idx], app);
            chunk_idx += 1;
        }

        render_cpu_processes(frame, chunks[chunk_idx], app);
    }
}

/// Render the CPU usage gauge.
pub fn render_cpu_gauge(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_pct = app.system_metrics.cpu_global;
    let color = usage_color(cpu_pct as f64);

    let label = format!(
        "CPU: {:.1}% | {} cores @ {} MHz | Procs: {} | Threads: {}",
        cpu_pct,
        app.system_metrics.cpus.len(),
        app.system_metrics
            .cpus
            .first()
            .map(|c| c.frequency)
            .unwrap_or(0),
        app.system_metrics.process_count,
        app.system_metrics.thread_count,
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(cpu_pct as f64 / 100.0)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    frame.render_widget(gauge, area);
}

/// Render the memory usage gauge.
pub fn render_memory_gauge(frame: &mut Frame, area: Rect, app: &App) {
    let mem = &app.system_metrics.memory;
    let mem_pct = if mem.total > 0 {
        (mem.used as f64 / mem.total as f64) * 100.0
    } else {
        0.0
    };
    let color = usage_color(mem_pct);

    let label = format!(
        "MEM: {} / {} ({:.1}%)",
        format_size(mem.used, BINARY),
        format_size(mem.total, BINARY),
        mem_pct,
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory"))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(mem_pct / 100.0)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    frame.render_widget(gauge, area);
}

/// Render the swap usage gauge.
pub fn render_swap_gauge(frame: &mut Frame, area: Rect, app: &App) {
    let mem = &app.system_metrics.memory;
    let swap_pct = if mem.swap_total > 0 {
        (mem.swap_used as f64 / mem.swap_total as f64) * 100.0
    } else {
        0.0
    };
    let color = usage_color(swap_pct);

    let label = format!(
        "SWAP: {} / {} ({:.1}%)",
        format_size(mem.swap_used, BINARY),
        format_size(mem.swap_total, BINARY),
        swap_pct,
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Swap"))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(swap_pct / 100.0)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    frame.render_widget(gauge, area);
}

/// Render compact CPU and memory info in one line.
pub fn render_compact_cpu_mem(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_pct = app.system_metrics.cpu_global;
    let mem = &app.system_metrics.memory;
    let mem_pct = if mem.total > 0 {
        (mem.used as f64 / mem.total as f64) * 100.0
    } else {
        0.0
    };

    let cpu_bar = create_bar(cpu_pct as f64, 20);
    let mem_bar = create_bar(mem_pct, 20);

    let text = vec![Line::from(vec![
        Span::styled("CPU ", Style::default().fg(Color::Cyan)),
        Span::styled(cpu_bar, Style::default().fg(usage_color(cpu_pct as f64))),
        Span::raw(format!(" {:5.1}%", cpu_pct)),
        Span::raw("  "),
        Span::styled("MEM ", Style::default().fg(Color::Cyan)),
        Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
        Span::raw(format!(" {:5.1}%", mem_pct)),
    ])];

    frame.render_widget(Paragraph::new(text), area);
}

/// Render compact network info in one line.
pub fn render_compact_network(frame: &mut Frame, area: Rect, app: &App) {
    let total_rx: f64 = app.system_metrics.networks.iter().map(|n| n.rx_rate).sum();
    let total_tx: f64 = app.system_metrics.networks.iter().map(|n| n.tx_rate).sum();

    let text = Line::from(vec![
        Span::styled("NET ", Style::default().fg(Color::Cyan)),
        Span::styled("▼", Style::default().fg(Color::Green)),
        Span::raw(format!(" {}/s ", format_size(total_rx as u64, BINARY))),
        Span::styled("▲", Style::default().fg(Color::Red)),
        Span::raw(format!(" {}/s", format_size(total_tx as u64, BINARY))),
    ]);

    frame.render_widget(Paragraph::new(text), area);
}

/// Render network interfaces table.
pub fn render_network(frame: &mut Frame, area: Rect, app: &App) {
    let mut rows: Vec<Row> = Vec::new();

    for net in &app.system_metrics.networks {
        let row = Row::new(vec![
            Cell::from(net.interface.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(format!("▼ {}/s", format_size(net.rx_rate as u64, BINARY)))
                .style(Style::default().fg(Color::Green)),
            Cell::from(format!("▲ {}/s", format_size(net.tx_rate as u64, BINARY)))
                .style(Style::default().fg(Color::Red)),
            Cell::from(format!(
                "Total: {} / {}",
                format_size(net.rx_bytes, BINARY),
                format_size(net.tx_bytes, BINARY)
            )),
        ]);
        rows.push(row);
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    )
    .block(Block::default().borders(Borders::ALL).title("Network"))
    .header(
        Row::new(vec!["Interface", "Download", "Upload", "Total"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    );

    frame.render_widget(table, area);
}

/// Render disk usage table.
pub fn render_disk(frame: &mut Frame, area: Rect, app: &App) {
    let mut rows: Vec<Row> = Vec::new();

    for disk in &app.system_metrics.disks {
        if disk.total == 0 {
            continue;
        }
        let pct = (disk.used as f64 / disk.total as f64) * 100.0;
        let bar = create_bar(pct, 10);

        let row = Row::new(vec![
            Cell::from(disk.mount_point.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(disk.fs_type.clone()),
            Cell::from(format!(
                "{} / {}",
                format_size(disk.used, BINARY),
                format_size(disk.total, BINARY)
            )),
            Cell::from(bar).style(Style::default().fg(usage_color(pct))),
            Cell::from(format!("{:.1}%", pct)),
        ]);
        rows.push(row);
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Length(12),
            Constraint::Length(6),
        ],
    )
    .block(Block::default().borders(Borders::ALL).title("Disk"))
    .header(
        Row::new(vec!["Mount", "FS", "Used/Total", "Usage", "%"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    );

    frame.render_widget(table, area);
}
