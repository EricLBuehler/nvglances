//! Process table rendering for CPU and GPU processes.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
    Frame,
};

use crate::app::App;
use crate::types::{ActivePanel, SortColumn};
use crate::utils::{truncate_string, usage_color};

/// Render the CPU process table.
pub fn render_cpu_processes(frame: &mut Frame, area: Rect, app: &mut App) {
    // Save area for mouse tracking
    app.cpu_process_area = Some(area);

    let procs = app.get_sorted_cpu_processes();
    let is_active = app.active_panel == ActivePanel::CpuProcesses;

    let sort_indicator = |col: SortColumn| -> &str {
        if app.cpu_sort == col {
            if app.sort_ascending {
                "▲"
            } else {
                "▼"
            }
        } else {
            ""
        }
    };

    let header = Row::new(vec![
        format!("PID{}", sort_indicator(SortColumn::Pid)),
        format!("USER{}", sort_indicator(SortColumn::User)),
        format!("CPU%{}", sort_indicator(SortColumn::Cpu)),
        format!("MEM%{}", sort_indicator(SortColumn::Memory)),
        "MEM".into(),
        "STATUS".into(),
        format!("NAME{}", sort_indicator(SortColumn::Name)),
        "COMMAND".into(),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = procs
        .iter()
        .map(|p| {
            let cpu_color = usage_color(p.cpu_usage as f64);
            let mem_color = usage_color(p.memory_usage as f64);

            Row::new(vec![
                Cell::from(format!("{}", p.pid)),
                Cell::from(p.user.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{:.1}", p.cpu_usage)).style(Style::default().fg(cpu_color)),
                Cell::from(format!("{:.1}", p.memory_usage)).style(Style::default().fg(mem_color)),
                Cell::from(format_size(p.memory_bytes, BINARY)),
                Cell::from(p.status.clone()),
                Cell::from(p.name.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(truncate_string(&p.command, 40)),
            ])
        })
        .collect();

    let title = format!(
        "CPU Processes ({}) [{}]",
        procs.len(),
        if is_active { "ACTIVE" } else { "inactive" }
    );
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let table = Table::new(
        rows,
        [
            ratatui::layout::Constraint::Length(7),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(6),
            ratatui::layout::Constraint::Length(6),
            ratatui::layout::Constraint::Length(9),
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Min(20),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, area, &mut app.cpu_process_state);

    // Scrollbar
    if procs.len() > (area.height as usize).saturating_sub(3) {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(procs.len())
            .position(app.cpu_process_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

/// Render the GPU process table.
pub fn render_gpu_processes(frame: &mut Frame, area: Rect, app: &mut App) {
    // Save area for mouse tracking
    app.gpu_process_area = Some(area);

    let procs = app.get_sorted_gpu_processes();
    let is_active = app.active_panel == ActivePanel::GpuProcesses;

    let sort_indicator = |col: SortColumn| -> &str {
        if app.gpu_sort == col {
            if app.sort_ascending {
                "▲"
            } else {
                "▼"
            }
        } else {
            ""
        }
    };

    let header = Row::new(vec![
        format!("PID{}", sort_indicator(SortColumn::Pid)),
        "GPU".into(),
        "TYPE".into(),
        format!("USER{}", sort_indicator(SortColumn::User)),
        format!("GPU_MEM{}", sort_indicator(SortColumn::GpuMemory)),
        format!("NAME{}", sort_indicator(SortColumn::Name)),
        "COMMAND".into(),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = procs
        .iter()
        .map(|p| {
            let type_color = if p.process_type == "C" {
                Color::Green
            } else {
                Color::Blue
            };

            Row::new(vec![
                Cell::from(format!("{}", p.pid)),
                Cell::from(format!("{}", p.gpu_index)),
                Cell::from(p.process_type.clone()).style(Style::default().fg(type_color)),
                Cell::from(p.user.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format_size(p.gpu_memory, BINARY)),
                Cell::from(p.name.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(truncate_string(&p.command, 40)),
            ])
        })
        .collect();

    let title = format!(
        "GPU Processes ({}) [{}]",
        procs.len(),
        if is_active { "ACTIVE" } else { "inactive" }
    );
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let table = Table::new(
        rows,
        [
            ratatui::layout::Constraint::Length(7),
            ratatui::layout::Constraint::Length(4),
            ratatui::layout::Constraint::Length(5),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Min(20),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, area, &mut app.gpu_process_state);

    // Scrollbar
    if procs.len() > (area.height as usize).saturating_sub(3) {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(procs.len())
            .position(app.gpu_process_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}
