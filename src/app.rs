//! Application state and core logic.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use nvml_wrapper::Nvml;
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use sysinfo::{Components, Disks, Networks, Pid, Signal, System, Users};

use crate::metrics::{collect_gpu_metrics, collect_system_metrics};
use crate::types::{
    ActivePanel, GpuMetrics, GpuProcessInfo, HistoryData, KillConfirmation, ProcessInfo,
    SortColumn, SystemMetrics,
};

/// Main application state.
pub struct App {
    // System data sources
    pub system: System,
    pub networks: Networks,
    pub disks: Disks,
    pub components: Components,
    pub users: Users,
    pub nvml: Option<Nvml>,

    // Collected metrics
    pub system_metrics: SystemMetrics,
    pub gpu_metrics: Option<GpuMetrics>,
    pub history: HistoryData,

    // State tracking
    pub last_network_stats: HashMap<String, (u64, u64)>,
    pub last_update: Instant,

    // UI state
    pub running: bool,
    pub show_help: bool,
    pub active_panel: ActivePanel,
    pub cpu_process_state: TableState,
    pub gpu_process_state: TableState,
    pub cpu_sort: SortColumn,
    pub gpu_sort: SortColumn,
    pub sort_ascending: bool,
    pub process_filter: String,
    pub show_all_processes: bool,
    pub compact_mode: bool,
    pub show_graphs: bool,

    // Settings
    pub refresh_rate: Duration,

    // Kill confirmation dialog
    pub kill_confirm: Option<KillConfirmation>,
    // Status message (shown briefly after actions)
    pub status_message: Option<(String, Instant)>,
    // Track panel areas for mouse support
    pub cpu_process_area: Option<Rect>,
    pub gpu_process_area: Option<Rect>,
}

impl App {
    /// Create a new App instance.
    pub fn new() -> anyhow::Result<Self> {
        let mut system = System::new_all();
        system.refresh_all();

        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        let users = Users::new_with_refreshed_list();

        let nvml = Nvml::init().ok();

        let mut app = Self {
            system,
            networks,
            disks,
            components,
            users,
            nvml,
            system_metrics: SystemMetrics::default(),
            gpu_metrics: None,
            history: HistoryData::new(),
            last_network_stats: HashMap::new(),
            last_update: Instant::now(),
            running: true,
            show_help: false,
            active_panel: ActivePanel::CpuProcesses,
            cpu_process_state: TableState::default(),
            gpu_process_state: TableState::default(),
            cpu_sort: SortColumn::Cpu,
            gpu_sort: SortColumn::GpuMemory,
            sort_ascending: false,
            process_filter: String::new(),
            show_all_processes: false,
            compact_mode: false,
            show_graphs: true,
            refresh_rate: Duration::from_millis(1000),
            kill_confirm: None,
            status_message: None,
            cpu_process_area: None,
            gpu_process_area: None,
        };

        app.cpu_process_state.select(Some(0));
        app.gpu_process_state.select(Some(0));
        app.refresh_all()?;

        Ok(app)
    }

    /// Refresh all metrics.
    pub fn refresh_all(&mut self) -> anyhow::Result<()> {
        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();

        self.system.refresh_all();
        self.networks.refresh();
        self.disks.refresh();
        self.components.refresh();

        self.system_metrics = collect_system_metrics(
            &self.system,
            &self.networks,
            &self.disks,
            &self.components,
            &self.users,
            &mut self.last_network_stats,
            elapsed,
        );

        self.gpu_metrics = collect_gpu_metrics(&self.nvml, &self.system, &self.users);

        self.update_history();

        Ok(())
    }

    /// Update history data for graphs.
    fn update_history(&mut self) {
        self.history.push_cpu(self.system_metrics.cpu_global as f64);

        let mem = &self.system_metrics.memory;
        let mem_pct = if mem.total > 0 {
            (mem.used as f64 / mem.total as f64) * 100.0
        } else {
            0.0
        };
        self.history.push_memory(mem_pct);

        if let Some(ref gpu_metrics) = self.gpu_metrics {
            for (i, gpu) in gpu_metrics.gpus.iter().enumerate() {
                self.history.push_gpu_util(i, gpu.gpu_utilization as f64);
                let mem_pct = if gpu.memory_total > 0 {
                    (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0
                } else {
                    0.0
                };
                self.history.push_gpu_mem(i, mem_pct);
            }
        }

        let total_rx: f64 = self.system_metrics.networks.iter().map(|n| n.rx_rate).sum();
        let total_tx: f64 = self.system_metrics.networks.iter().map(|n| n.tx_rate).sum();
        self.history
            .push_network(total_rx / 1024.0 / 1024.0, total_tx / 1024.0 / 1024.0);
    }

    /// Get sorted CPU processes based on current sort settings.
    pub fn get_sorted_cpu_processes(&self) -> Vec<ProcessInfo> {
        let mut procs = if self.show_all_processes {
            self.system_metrics.processes.clone()
        } else {
            self.system_metrics
                .processes
                .iter()
                .filter(|p| p.cpu_usage > 0.0 || p.memory_usage > 0.1)
                .cloned()
                .collect()
        };

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter)
                    || p.user.to_lowercase().contains(&filter)
                    || p.command.to_lowercase().contains(&filter)
            });
        }

        procs.sort_by(|a, b| {
            let cmp = match self.cpu_sort {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                SortColumn::Cpu => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Memory | SortColumn::GpuMemory => a
                    .memory_usage
                    .partial_cmp(&b.memory_usage)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        procs
    }

    /// Get sorted GPU processes based on current sort settings.
    pub fn get_sorted_gpu_processes(&self) -> Vec<GpuProcessInfo> {
        let Some(ref gpu_metrics) = self.gpu_metrics else {
            return Vec::new();
        };

        let mut procs = gpu_metrics.processes.clone();

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter)
                    || p.user.to_lowercase().contains(&filter)
                    || p.command.to_lowercase().contains(&filter)
            });
        }

        procs.sort_by(|a, b| {
            let cmp = match self.gpu_sort {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                SortColumn::GpuMemory | SortColumn::Memory => a.gpu_memory.cmp(&b.gpu_memory),
                SortColumn::Cpu => a.sm_utilization.cmp(&b.sm_utilization),
            };
            if self.sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        procs
    }

    /// Handle keyboard input.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Handle kill confirmation dialog
        if let Some(ref confirm) = self.kill_confirm.clone() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_kill(confirm.pid, confirm.signal);
                    self.kill_confirm = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.kill_confirm = None;
                    self.set_status("Kill cancelled".to_string());
                }
                _ => {}
            }
            return;
        }

        if self.show_help {
            self.show_help = false;
            return;
        }

        // Check for ctrl-modified keys first
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') => {
                    self.running = false;
                    return;
                }
                KeyCode::Char('k') => {
                    self.request_kill(Signal::Kill);
                    return;
                }
                KeyCode::Char('t') => {
                    self.request_kill(Signal::Term);
                    return;
                }
                KeyCode::Char('i') => {
                    self.request_kill(Signal::Interrupt);
                    return;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    ActivePanel::CpuProcesses => ActivePanel::GpuProcesses,
                    ActivePanel::GpuProcesses => ActivePanel::CpuProcesses,
                };
            }
            KeyCode::Char('a') => self.show_all_processes = !self.show_all_processes,
            KeyCode::Char('g') => self.show_graphs = !self.show_graphs,
            KeyCode::Char('c') => self.compact_mode = !self.compact_mode,
            KeyCode::Char('1') => self.set_sort(SortColumn::Pid),
            KeyCode::Char('2') => self.set_sort(SortColumn::Name),
            KeyCode::Char('3') => self.set_sort(SortColumn::User),
            KeyCode::Char('4') => self.set_sort(SortColumn::Cpu),
            KeyCode::Char('5') => self.set_sort(SortColumn::Memory),
            KeyCode::Char('6') => self.set_sort(SortColumn::GpuMemory),
            KeyCode::Char('r') => self.sort_ascending = !self.sort_ascending,
            KeyCode::Char('/') => {
                self.process_filter.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let new_rate = self.refresh_rate.as_millis().saturating_sub(100).max(100);
                self.refresh_rate = Duration::from_millis(new_rate as u64);
            }
            KeyCode::Char('-') => {
                let new_rate = self.refresh_rate.as_millis().saturating_add(100).min(5000);
                self.refresh_rate = Duration::from_millis(new_rate as u64);
            }
            KeyCode::Delete => {
                self.request_kill(Signal::Term);
            }
            _ => {}
        }
    }

    /// Request to kill a process (shows confirmation dialog).
    fn request_kill(&mut self, signal: Signal) {
        let (pid, name) = match self.active_panel {
            ActivePanel::CpuProcesses => {
                let procs = self.get_sorted_cpu_processes();
                let idx = self.cpu_process_state.selected().unwrap_or(0);
                if let Some(proc) = procs.get(idx) {
                    (proc.pid, proc.name.clone())
                } else {
                    return;
                }
            }
            ActivePanel::GpuProcesses => {
                let procs = self.get_sorted_gpu_processes();
                let idx = self.gpu_process_state.selected().unwrap_or(0);
                if let Some(proc) = procs.get(idx) {
                    (proc.pid, proc.name.clone())
                } else {
                    return;
                }
            }
        };

        self.kill_confirm = Some(KillConfirmation { pid, name, signal });
    }

    /// Execute a kill signal on a process.
    fn execute_kill(&mut self, pid: u32, signal: Signal) {
        let sys_pid = Pid::from_u32(pid);
        if let Some(process) = self.system.process(sys_pid) {
            let signal_name = match signal {
                Signal::Kill => "SIGKILL",
                Signal::Term => "SIGTERM",
                Signal::Interrupt => "SIGINT",
                _ => "signal",
            };
            if process.kill_with(signal).unwrap_or(false) {
                self.set_status(format!("Sent {} to PID {}", signal_name, pid));
            } else {
                self.set_status(format!("Failed to send {} to PID {}", signal_name, pid));
            }
        } else {
            self.set_status(format!("Process {} not found", pid));
        }
    }

    /// Set a status message to display briefly.
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Clear expired status message.
    pub fn clear_old_status(&mut self) {
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }

    /// Handle mouse input.
    pub fn handle_mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is in CPU process area
                if let Some(area) = self.cpu_process_area {
                    if column >= area.x
                        && column < area.x + area.width
                        && row >= area.y
                        && row < area.y + area.height
                    {
                        self.active_panel = ActivePanel::CpuProcesses;
                        let relative_row = row.saturating_sub(area.y + 2);
                        let procs = self.get_sorted_cpu_processes();
                        if (relative_row as usize) < procs.len() {
                            self.cpu_process_state.select(Some(relative_row as usize));
                        }
                        return;
                    }
                }
                // Check if click is in GPU process area
                if let Some(area) = self.gpu_process_area {
                    if column >= area.x
                        && column < area.x + area.width
                        && row >= area.y
                        && row < area.y + area.height
                    {
                        self.active_panel = ActivePanel::GpuProcesses;
                        let relative_row = row.saturating_sub(area.y + 2);
                        let procs = self.get_sorted_gpu_processes();
                        if (relative_row as usize) < procs.len() {
                            self.gpu_process_state.select(Some(relative_row as usize));
                        }
                        return;
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                self.move_selection(3);
            }
            MouseEventKind::ScrollUp => {
                self.move_selection(-3);
            }
            _ => {}
        }
    }

    /// Set the sort column for the active panel.
    fn set_sort(&mut self, column: SortColumn) {
        match self.active_panel {
            ActivePanel::CpuProcesses => {
                if self.cpu_sort == column {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.cpu_sort = column;
                    self.sort_ascending = false;
                }
            }
            ActivePanel::GpuProcesses => {
                if self.gpu_sort == column {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.gpu_sort = column;
                    self.sort_ascending = false;
                }
            }
        }
    }

    /// Move the selection by a delta.
    fn move_selection(&mut self, delta: i32) {
        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 {
            return;
        }

        let state = match self.active_panel {
            ActivePanel::CpuProcesses => &mut self.cpu_process_state,
            ActivePanel::GpuProcesses => &mut self.gpu_process_state,
        };

        let current = state.selected().unwrap_or(0);
        let new = if delta > 0 {
            (current + delta as usize).min(len - 1)
        } else {
            current.saturating_sub((-delta) as usize)
        };
        state.select(Some(new));
    }

    /// Move the selection to a specific position.
    fn move_selection_to(&mut self, pos: usize) {
        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 {
            return;
        }

        let state = match self.active_panel {
            ActivePanel::CpuProcesses => &mut self.cpu_process_state,
            ActivePanel::GpuProcesses => &mut self.gpu_process_state,
        };

        state.select(Some(pos.min(len - 1)));
    }
}
