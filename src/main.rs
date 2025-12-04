use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use humansize::{format_size, BINARY};
use nvml_wrapper::Nvml;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Cell, Chart, Dataset, Gauge, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
    Frame, Terminal,
};
use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};
use sysinfo::{Components, Disks, Networks, Pid, ProcessStatus, System, Users};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Clone)]
#[allow(dead_code)]
struct CpuInfo {
    name: String,
    usage: f32,
    frequency: u64,
}

#[derive(Clone)]
#[allow(dead_code)]
struct MemoryInfo {
    total: u64,
    used: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
}

#[derive(Clone)]
#[allow(dead_code)]
struct DiskInfo {
    name: String,
    mount_point: String,
    total: u64,
    used: u64,
    fs_type: String,
}

#[derive(Clone)]
struct NetworkInfo {
    interface: String,
    rx_bytes: u64,
    tx_bytes: u64,
    rx_rate: f64,
    tx_rate: f64,
}

#[derive(Clone)]
struct ProcessInfo {
    pid: u32,
    name: String,
    user: String,
    cpu_usage: f32,
    memory_usage: f32,
    memory_bytes: u64,
    status: String,
    command: String,
}

#[derive(Clone)]
#[allow(dead_code)]
struct GpuInfo {
    index: u32,
    name: String,
    temperature: u32,
    fan_speed: u32,
    power_usage: u32,
    power_limit: u32,
    gpu_utilization: u32,
    memory_utilization: u32,
    memory_used: u64,
    memory_total: u64,
    encoder_utilization: u32,
    decoder_utilization: u32,
    pcie_rx: u64,
    pcie_tx: u64,
    sm_clock: u32,
    mem_clock: u32,
    pstate: String,
}

#[derive(Clone)]
struct GpuProcessInfo {
    pid: u32,
    name: String,
    user: String,
    gpu_index: u32,
    gpu_memory: u64,
    sm_utilization: Option<u32>,
    command: String,
    process_type: String,
}

#[derive(Clone)]
struct SystemMetrics {
    hostname: String,
    uptime: u64,
    load_avg: (f64, f64, f64),
    cpus: Vec<CpuInfo>,
    cpu_global: f32,
    memory: MemoryInfo,
    disks: Vec<DiskInfo>,
    networks: Vec<NetworkInfo>,
    processes: Vec<ProcessInfo>,
    process_count: usize,
    thread_count: usize,
    temperatures: Vec<(String, f32)>,
}

#[derive(Clone)]
struct GpuMetrics {
    gpus: Vec<GpuInfo>,
    processes: Vec<GpuProcessInfo>,
    driver_version: String,
    cuda_version: String,
}

struct HistoryData {
    cpu_history: Vec<f64>,
    memory_history: Vec<f64>,
    gpu_util_history: Vec<Vec<f64>>,
    gpu_mem_history: Vec<Vec<f64>>,
    network_rx_history: Vec<f64>,
    network_tx_history: Vec<f64>,
}

impl HistoryData {
    fn new() -> Self {
        Self {
            cpu_history: vec![0.0; 60],
            memory_history: vec![0.0; 60],
            gpu_util_history: Vec::new(),
            gpu_mem_history: Vec::new(),
            network_rx_history: vec![0.0; 60],
            network_tx_history: vec![0.0; 60],
        }
    }

    fn push_cpu(&mut self, value: f64) {
        self.cpu_history.remove(0);
        self.cpu_history.push(value);
    }

    fn push_memory(&mut self, value: f64) {
        self.memory_history.remove(0);
        self.memory_history.push(value);
    }

    fn push_gpu_util(&mut self, gpu_idx: usize, value: f64) {
        while self.gpu_util_history.len() <= gpu_idx {
            self.gpu_util_history.push(vec![0.0; 60]);
        }
        self.gpu_util_history[gpu_idx].remove(0);
        self.gpu_util_history[gpu_idx].push(value);
    }

    fn push_gpu_mem(&mut self, gpu_idx: usize, value: f64) {
        while self.gpu_mem_history.len() <= gpu_idx {
            self.gpu_mem_history.push(vec![0.0; 60]);
        }
        self.gpu_mem_history[gpu_idx].remove(0);
        self.gpu_mem_history[gpu_idx].push(value);
    }

    fn push_network(&mut self, rx: f64, tx: f64) {
        self.network_rx_history.remove(0);
        self.network_rx_history.push(rx);
        self.network_tx_history.remove(0);
        self.network_tx_history.push(tx);
    }
}

// ============================================================================
// Application State
// ============================================================================

#[derive(PartialEq, Clone, Copy)]
enum SortColumn {
    Pid,
    Name,
    User,
    Cpu,
    Memory,
    GpuMemory,
}

#[derive(PartialEq, Clone, Copy)]
enum ActivePanel {
    CpuProcesses,
    GpuProcesses,
}

struct App {
    system: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    users: Users,
    nvml: Option<Nvml>,

    system_metrics: SystemMetrics,
    gpu_metrics: Option<GpuMetrics>,
    history: HistoryData,

    last_network_stats: HashMap<String, (u64, u64)>,
    last_update: Instant,

    running: bool,
    show_help: bool,
    active_panel: ActivePanel,
    cpu_process_state: TableState,
    gpu_process_state: TableState,
    cpu_sort: SortColumn,
    gpu_sort: SortColumn,
    sort_ascending: bool,
    process_filter: String,
    show_all_processes: bool,
    compact_mode: bool,
    show_graphs: bool,

    refresh_rate: Duration,
    #[allow(dead_code)]
    start_time: DateTime<Local>,
}

impl App {
    fn new() -> Result<Self> {
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
            system_metrics: SystemMetrics {
                hostname: String::new(),
                uptime: 0,
                load_avg: (0.0, 0.0, 0.0),
                cpus: Vec::new(),
                cpu_global: 0.0,
                memory: MemoryInfo {
                    total: 0,
                    used: 0,
                    available: 0,
                    swap_total: 0,
                    swap_used: 0,
                },
                disks: Vec::new(),
                networks: Vec::new(),
                processes: Vec::new(),
                process_count: 0,
                thread_count: 0,
                temperatures: Vec::new(),
            },
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
            start_time: Local::now(),
        };

        app.cpu_process_state.select(Some(0));
        app.gpu_process_state.select(Some(0));
        app.refresh_all()?;

        Ok(app)
    }

    fn refresh_all(&mut self) -> Result<()> {
        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();

        self.system.refresh_all();
        self.networks.refresh();
        self.disks.refresh();
        self.components.refresh();

        self.refresh_system_metrics(elapsed);
        self.refresh_gpu_metrics();
        self.update_history();

        Ok(())
    }

    fn refresh_system_metrics(&mut self, elapsed: Duration) {
        // Hostname
        self.system_metrics.hostname = System::host_name().unwrap_or_else(|| "unknown".into());

        // Uptime
        self.system_metrics.uptime = System::uptime();

        // Load average
        let load = System::load_average();
        self.system_metrics.load_avg = (load.one, load.five, load.fifteen);

        // CPUs
        self.system_metrics.cpus = self.system.cpus()
            .iter()
            .map(|cpu| CpuInfo {
                name: cpu.name().to_string(),
                usage: cpu.cpu_usage(),
                frequency: cpu.frequency(),
            })
            .collect();

        self.system_metrics.cpu_global = self.system.global_cpu_usage();

        // Memory
        self.system_metrics.memory = MemoryInfo {
            total: self.system.total_memory(),
            used: self.system.used_memory(),
            available: self.system.available_memory(),
            swap_total: self.system.total_swap(),
            swap_used: self.system.used_swap(),
        };

        // Disks
        self.system_metrics.disks = self.disks.iter()
            .map(|disk| DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total: disk.total_space(),
                used: disk.total_space() - disk.available_space(),
                fs_type: disk.file_system().to_string_lossy().to_string(),
            })
            .collect();

        // Networks
        let elapsed_secs = elapsed.as_secs_f64().max(0.001);
        self.system_metrics.networks = self.networks.iter()
            .filter(|(name, _)| !name.starts_with("lo"))
            .map(|(name, data)| {
                let (prev_rx, prev_tx) = self.last_network_stats
                    .get(name)
                    .copied()
                    .unwrap_or((data.total_received(), data.total_transmitted()));

                let rx_bytes = data.total_received();
                let tx_bytes = data.total_transmitted();
                let rx_rate = (rx_bytes.saturating_sub(prev_rx)) as f64 / elapsed_secs;
                let tx_rate = (tx_bytes.saturating_sub(prev_tx)) as f64 / elapsed_secs;

                self.last_network_stats.insert(name.clone(), (rx_bytes, tx_bytes));

                NetworkInfo {
                    interface: name.clone(),
                    rx_bytes,
                    tx_bytes,
                    rx_rate,
                    tx_rate,
                }
            })
            .collect();

        // Temperatures
        self.system_metrics.temperatures = self.components.iter()
            .filter_map(|c| {
                let temp = c.temperature();
                if temp > 0.0 {
                    Some((c.label().to_string(), temp))
                } else {
                    None
                }
            })
            .collect();

        // Processes
        let user_map: HashMap<_, _> = self.users.iter()
            .map(|u| (u.id().clone(), u.name().to_string()))
            .collect();

        self.system_metrics.processes = self.system.processes()
            .iter()
            .map(|(pid, proc)| {
                let user = proc.user_id()
                    .and_then(|uid| user_map.get(uid))
                    .cloned()
                    .unwrap_or_else(|| "?".into());

                let status = match proc.status() {
                    ProcessStatus::Run => "Running",
                    ProcessStatus::Sleep => "Sleep",
                    ProcessStatus::Idle => "Idle",
                    ProcessStatus::Zombie => "Zombie",
                    ProcessStatus::Stop => "Stopped",
                    _ => "Unknown",
                }.to_string();
                let cmd: Vec<_> = proc.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();
                let command = if cmd.is_empty() {
                    proc.name().to_string_lossy().to_string()
                } else {
                    cmd.join(" ")
                };

                ProcessInfo {
                    pid: pid.as_u32(),
                    name: proc.name().to_string_lossy().to_string(),
                    user,
                    cpu_usage: proc.cpu_usage(),
                    memory_usage: (proc.memory() as f32 / self.system_metrics.memory.total as f32) * 100.0,
                    memory_bytes: proc.memory(),
                    status,
                    command,
                }
            })
            .collect();

        self.system_metrics.process_count = self.system.processes().len();
        self.system_metrics.thread_count = self.system.processes().len();
    }

    fn refresh_gpu_metrics(&mut self) {
        let Some(ref nvml) = self.nvml else { return };

        let device_count = match nvml.device_count() {
            Ok(c) => c,
            Err(_) => return,
        };

        let driver_version = nvml.sys_driver_version().unwrap_or_else(|_| "N/A".into());
        let cuda_version = nvml.sys_cuda_driver_version()
            .map(|v| format!("{}.{}", v / 1000, (v % 1000) / 10))
            .unwrap_or_else(|_| "N/A".into());

        let mut gpus = Vec::new();
        let mut processes = Vec::new();

        for i in 0..device_count {
            let Ok(device) = nvml.device_by_index(i) else { continue };

            let name = device.name().unwrap_or_else(|_| "Unknown GPU".into());
            let temperature = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).unwrap_or(0);
            let fan_speed = device.fan_speed(0).unwrap_or(0);
            let power_usage = device.power_usage().unwrap_or(0) / 1000;
            let power_limit = device.power_management_limit().unwrap_or(0) / 1000;

            let utilization = device.utilization_rates().unwrap_or(nvml_wrapper::struct_wrappers::device::Utilization { gpu: 0, memory: 0 });
            let memory_info = device.memory_info().unwrap_or(nvml_wrapper::struct_wrappers::device::MemoryInfo { free: 0, total: 1, used: 0 });

            let encoder = device.encoder_utilization().map(|e| e.utilization).unwrap_or(0);
            let decoder = device.decoder_utilization().map(|d| d.utilization).unwrap_or(0);

            let pcie = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Send).unwrap_or(0);
            let pcie_rx = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Receive).unwrap_or(0);

            let sm_clock = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics).unwrap_or(0);
            let mem_clock = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory).unwrap_or(0);

            let pstate = device.performance_state()
                .map(|p| format!("{:?}", p))
                .unwrap_or_else(|_| "?".into());

            gpus.push(GpuInfo {
                index: i,
                name,
                temperature,
                fan_speed,
                power_usage,
                power_limit,
                gpu_utilization: utilization.gpu,
                memory_utilization: utilization.memory,
                memory_used: memory_info.used,
                memory_total: memory_info.total,
                encoder_utilization: encoder,
                decoder_utilization: decoder,
                pcie_rx: pcie_rx as u64 * 1024,
                pcie_tx: pcie as u64 * 1024,
                sm_clock,
                mem_clock,
                pstate,
            });

            // GPU Processes
            if let Ok(compute_procs) = device.running_compute_processes() {
                for proc in compute_procs {
                    let pid = proc.pid;
                    let (name, user, command) = self.get_process_info(pid);

                    processes.push(GpuProcessInfo {
                        pid,
                        name,
                        user,
                        gpu_index: i,
                        gpu_memory: match proc.used_gpu_memory {
                            nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) => bytes,
                            nvml_wrapper::enums::device::UsedGpuMemory::Unavailable => 0,
                        },
                        sm_utilization: None,
                        command,
                        process_type: "C".into(),
                    });
                }
            }

            if let Ok(graphics_procs) = device.running_graphics_processes() {
                for proc in graphics_procs {
                    let pid = proc.pid;
                    let (name, user, command) = self.get_process_info(pid);

                    // Check if we already have this process as compute
                    if !processes.iter().any(|p| p.pid == pid && p.gpu_index == i) {
                        processes.push(GpuProcessInfo {
                            pid,
                            name,
                            user,
                            gpu_index: i,
                            gpu_memory: match proc.used_gpu_memory {
                            nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) => bytes,
                            nvml_wrapper::enums::device::UsedGpuMemory::Unavailable => 0,
                        },
                            sm_utilization: None,
                            command,
                            process_type: "G".into(),
                        });
                    }
                }
            }
        }

        self.gpu_metrics = Some(GpuMetrics {
            gpus,
            processes,
            driver_version,
            cuda_version,
        });
    }

    fn get_process_info(&self, pid: u32) -> (String, String, String) {
        let sys_pid = Pid::from_u32(pid);
        if let Some(proc) = self.system.process(sys_pid) {
            let user_map: HashMap<_, _> = self.users.iter()
                .map(|u| (u.id().clone(), u.name().to_string()))
                .collect();

            let user = proc.user_id()
                .and_then(|uid| user_map.get(uid))
                .cloned()
                .unwrap_or_else(|| "?".into());

            let cmd: Vec<_> = proc.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();
            let command = if cmd.is_empty() {
                proc.name().to_string_lossy().to_string()
            } else {
                cmd.join(" ")
            };

            (proc.name().to_string_lossy().to_string(), user, command)
        } else {
            ("?".into(), "?".into(), "?".into())
        }
    }

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
        self.history.push_network(total_rx / 1024.0 / 1024.0, total_tx / 1024.0 / 1024.0);
    }

    fn get_sorted_cpu_processes(&self) -> Vec<ProcessInfo> {
        let mut procs = if self.show_all_processes {
            self.system_metrics.processes.clone()
        } else {
            self.system_metrics.processes.iter()
                .filter(|p| p.cpu_usage > 0.0 || p.memory_usage > 0.1)
                .cloned()
                .collect()
        };

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter) ||
                p.user.to_lowercase().contains(&filter) ||
                p.command.to_lowercase().contains(&filter)
            });
        }

        procs.sort_by(|a, b| {
            let cmp = match self.cpu_sort {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                SortColumn::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Memory | SortColumn::GpuMemory => a.memory_usage.partial_cmp(&b.memory_usage).unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.sort_ascending { cmp } else { cmp.reverse() }
        });

        procs
    }

    fn get_sorted_gpu_processes(&self) -> Vec<GpuProcessInfo> {
        let Some(ref gpu_metrics) = self.gpu_metrics else {
            return Vec::new();
        };

        let mut procs = gpu_metrics.processes.clone();

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter) ||
                p.user.to_lowercase().contains(&filter) ||
                p.command.to_lowercase().contains(&filter)
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
            if self.sort_ascending { cmp } else { cmp.reverse() }
        });

        procs
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if self.show_help {
            self.show_help = false;
            return;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.running = false,
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
                // Filter mode would require more complex input handling
                // For now, toggle filter off
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
            _ => {}
        }
    }

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

    fn move_selection(&mut self, delta: i32) {
        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 { return; }

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

    fn move_selection_to(&mut self, pos: usize) {
        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 { return; }

        let state = match self.active_panel {
            ActivePanel::CpuProcesses => &mut self.cpu_process_state,
            ActivePanel::GpuProcesses => &mut self.gpu_process_state,
        };

        state.select(Some(pos.min(len - 1)));
    }
}

// ============================================================================
// UI Rendering
// ============================================================================

fn ui(frame: &mut Frame, app: &mut App) {
    if app.show_help {
        render_help(frame, frame.area());
        return;
    }

    let has_gpu = app.gpu_metrics.is_some();

    // Main layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Header
            Constraint::Min(0),     // Content
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    render_header(frame, main_chunks[0], app);
    render_footer(frame, main_chunks[2], app);

    // Content area layout depends on whether we have GPU and graphs enabled
    let content_area = main_chunks[1];

    if has_gpu {
        // Split horizontally: left for system, right for GPU
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);

        render_system_panel(frame, h_chunks[0], app);
        render_gpu_panel(frame, h_chunks[1], app);
    } else {
        render_system_panel(frame, content_area, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let uptime = format_duration(app.system_metrics.uptime);
    let now = Local::now();

    let gpu_info = if let Some(ref gm) = app.gpu_metrics {
        format!(" | Driver: {} | CUDA: {}", gm.driver_version, gm.cuda_version)
    } else {
        String::new()
    };

    let header = Line::from(vec![
        Span::styled("nvglances", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled(&app.system_metrics.hostname, Style::default().fg(Color::Green)),
        Span::raw(" | "),
        Span::styled(format!("up {}", uptime), Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled(format!("Load: {:.2} {:.2} {:.2}",
            app.system_metrics.load_avg.0,
            app.system_metrics.load_avg.1,
            app.system_metrics.load_avg.2),
            Style::default().fg(Color::Magenta)),
        Span::styled(gpu_info, Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(now.format("%H:%M:%S").to_string(), Style::default().fg(Color::White)),
    ]);

    frame.render_widget(Paragraph::new(header), area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let refresh_ms = app.refresh_rate.as_millis();

    let footer = Line::from(vec![
        Span::styled(" ?", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Help "),
        Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Switch "),
        Span::styled("1-6", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Sort "),
        Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Reverse "),
        Span::styled("a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":All "),
        Span::styled("g", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Graphs "),
        Span::styled("c", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":Compact "),
        Span::styled("+/-", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(format!(":Rate({}ms) ", refresh_ms)),
        Span::styled("q", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(":Quit"),
    ]);

    frame.render_widget(Paragraph::new(footer), area);
}

fn render_system_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.show_graphs && !app.compact_mode {
            vec![
                Constraint::Length(3),  // CPU gauge
                Constraint::Length(3),  // Memory gauge
                Constraint::Length(6),  // CPU/Memory graphs
                Constraint::Length(4),  // Network
                Constraint::Length(4),  // Disk
                Constraint::Min(8),     // CPU Processes
            ]
        } else if app.compact_mode {
            vec![
                Constraint::Length(2),  // CPU + Memory compact
                Constraint::Length(2),  // Network compact
                Constraint::Min(8),     // CPU Processes
            ]
        } else {
            vec![
                Constraint::Length(3),  // CPU gauge
                Constraint::Length(3),  // Memory gauge
                Constraint::Length(4),  // Network
                Constraint::Length(4),  // Disk
                Constraint::Min(8),     // CPU Processes
            ]
        })
        .split(area);

    let mut chunk_idx = 0;

    if app.compact_mode {
        render_compact_cpu_mem(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
        render_compact_network(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
    } else {
        render_cpu_gauge(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
        render_memory_gauge(frame, chunks[chunk_idx], app);
        chunk_idx += 1;

        if app.show_graphs {
            render_cpu_mem_graph(frame, chunks[chunk_idx], app);
            chunk_idx += 1;
        }

        render_network(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
        render_disk(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
    }

    render_cpu_processes(frame, chunks[chunk_idx], app);
}

fn render_gpu_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else { return };

    let gpu_count = gpu_metrics.gpus.len();
    let gpu_height = if app.compact_mode { 2 } else { 5 };
    let total_gpu_height = (gpu_height * gpu_count).min(area.height.saturating_sub(10) as usize);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.show_graphs && !app.compact_mode {
            vec![
                Constraint::Length(total_gpu_height as u16),  // GPU cards
                Constraint::Length(6),                         // GPU graphs
                Constraint::Min(8),                            // GPU Processes
            ]
        } else {
            vec![
                Constraint::Length(total_gpu_height as u16),  // GPU cards
                Constraint::Min(8),                            // GPU Processes
            ]
        })
        .split(area);

    render_gpu_cards(frame, chunks[0], app);

    let mut chunk_idx = 1;
    if app.show_graphs && !app.compact_mode {
        render_gpu_graphs(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
    }

    render_gpu_processes(frame, chunks[chunk_idx], app);
}

fn render_cpu_gauge(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_pct = app.system_metrics.cpu_global;
    let color = usage_color(cpu_pct as f64);

    let label = format!(
        "CPU: {:.1}% | {} cores @ {} MHz | Procs: {} | Threads: {}",
        cpu_pct,
        app.system_metrics.cpus.len(),
        app.system_metrics.cpus.first().map(|c| c.frequency).unwrap_or(0),
        app.system_metrics.process_count,
        app.system_metrics.thread_count,
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .gauge_style(Style::default().fg(color))
        .percent(cpu_pct as u16)
        .label(label);

    frame.render_widget(gauge, area);
}

fn render_memory_gauge(frame: &mut Frame, area: Rect, app: &App) {
    let mem = &app.system_metrics.memory;
    let mem_pct = if mem.total > 0 { (mem.used as f64 / mem.total as f64) * 100.0 } else { 0.0 };
    let swap_pct = if mem.swap_total > 0 { (mem.swap_used as f64 / mem.swap_total as f64) * 100.0 } else { 0.0 };
    let color = usage_color(mem_pct);

    let label = format!(
        "MEM: {} / {} ({:.1}%) | SWAP: {} / {} ({:.1}%)",
        format_size(mem.used, BINARY),
        format_size(mem.total, BINARY),
        mem_pct,
        format_size(mem.swap_used, BINARY),
        format_size(mem.swap_total, BINARY),
        swap_pct,
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory"))
        .gauge_style(Style::default().fg(color))
        .percent(mem_pct as u16)
        .label(label);

    frame.render_widget(gauge, area);
}

fn render_compact_cpu_mem(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_pct = app.system_metrics.cpu_global;
    let mem = &app.system_metrics.memory;
    let mem_pct = if mem.total > 0 { (mem.used as f64 / mem.total as f64) * 100.0 } else { 0.0 };

    let cpu_bar = create_bar(cpu_pct as f64, 20);
    let mem_bar = create_bar(mem_pct, 20);

    let text = vec![
        Line::from(vec![
            Span::styled("CPU ", Style::default().fg(Color::Cyan)),
            Span::styled(cpu_bar, Style::default().fg(usage_color(cpu_pct as f64))),
            Span::raw(format!(" {:5.1}%", cpu_pct)),
            Span::raw("  "),
            Span::styled("MEM ", Style::default().fg(Color::Cyan)),
            Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
            Span::raw(format!(" {:5.1}%", mem_pct)),
        ]),
    ];

    frame.render_widget(Paragraph::new(text), area);
}

fn render_compact_network(frame: &mut Frame, area: Rect, app: &App) {
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

fn render_cpu_mem_graph(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_data: Vec<(f64, f64)> = app.history.cpu_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let mem_data: Vec<(f64, f64)> = app.history.memory_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let datasets = vec![
        Dataset::default()
            .name("CPU")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&cpu_data),
        Dataset::default()
            .name("MEM")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Magenta))
            .data(&mem_data),
    ];

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::ALL).title("History"))
        .x_axis(Axis::default()
            .bounds([0.0, 60.0])
            .labels::<Vec<Line>>(vec![]))
        .y_axis(Axis::default()
            .bounds([0.0, 100.0])
            .labels(vec![Line::from("0%"), Line::from("50%"), Line::from("100%")]));

    frame.render_widget(chart, area);
}

fn render_network(frame: &mut Frame, area: Rect, app: &App) {
    let mut rows: Vec<Row> = Vec::new();

    for net in &app.system_metrics.networks {
        let row = Row::new(vec![
            Cell::from(net.interface.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(format!("▼ {}/s", format_size(net.rx_rate as u64, BINARY))).style(Style::default().fg(Color::Green)),
            Cell::from(format!("▲ {}/s", format_size(net.tx_rate as u64, BINARY))).style(Style::default().fg(Color::Red)),
            Cell::from(format!("Total: {} / {}", format_size(net.rx_bytes, BINARY), format_size(net.tx_bytes, BINARY))),
        ]);
        rows.push(row);
    }

    let table = Table::new(rows, [
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Min(20),
    ])
    .block(Block::default().borders(Borders::ALL).title("Network"))
    .header(Row::new(vec!["Interface", "Download", "Upload", "Total"])
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));

    frame.render_widget(table, area);
}

fn render_disk(frame: &mut Frame, area: Rect, app: &App) {
    let mut rows: Vec<Row> = Vec::new();

    for disk in &app.system_metrics.disks {
        if disk.total == 0 { continue; }
        let pct = (disk.used as f64 / disk.total as f64) * 100.0;
        let bar = create_bar(pct, 10);

        let row = Row::new(vec![
            Cell::from(disk.mount_point.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(disk.fs_type.clone()),
            Cell::from(format!("{} / {}", format_size(disk.used, BINARY), format_size(disk.total, BINARY))),
            Cell::from(bar).style(Style::default().fg(usage_color(pct))),
            Cell::from(format!("{:.1}%", pct)),
        ]);
        rows.push(row);
    }

    let table = Table::new(rows, [
        Constraint::Length(15),
        Constraint::Length(8),
        Constraint::Length(18),
        Constraint::Length(12),
        Constraint::Length(6),
    ])
    .block(Block::default().borders(Borders::ALL).title("Disk"))
    .header(Row::new(vec!["Mount", "FS", "Used/Total", "Usage", "%"])
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));

    frame.render_widget(table, area);
}

fn render_cpu_processes(frame: &mut Frame, area: Rect, app: &mut App) {
    let procs = app.get_sorted_cpu_processes();
    let is_active = app.active_panel == ActivePanel::CpuProcesses;

    let sort_indicator = |col: SortColumn| -> &str {
        if app.cpu_sort == col {
            if app.sort_ascending { "▲" } else { "▼" }
        } else { "" }

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
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = procs.iter().map(|p| {
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
    }).collect();

    let title = format!("CPU Processes ({}) [{}]", procs.len(), if is_active { "ACTIVE" } else { "inactive" });
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let table = Table::new(rows, [
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(9),
        Constraint::Length(8),
        Constraint::Length(15),
        Constraint::Min(20),
    ])
    .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

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
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn render_gpu_cards(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else { return };

    let gpu_count = gpu_metrics.gpus.len();
    if gpu_count == 0 { return; }

    let height_per_gpu = if app.compact_mode { 2 } else { 5 };
    let constraints: Vec<Constraint> = (0..gpu_count)
        .map(|_| Constraint::Length(height_per_gpu))
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, gpu) in gpu_metrics.gpus.iter().enumerate() {
        if i >= chunks.len() { break; }
        render_gpu_card(frame, chunks[i], gpu, app.compact_mode);
    }
}

fn render_gpu_card(frame: &mut Frame, area: Rect, gpu: &GpuInfo, compact: bool) {
    let gpu_pct = gpu.gpu_utilization as f64;
    let mem_pct = if gpu.memory_total > 0 {
        (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0
    } else {
        0.0
    };

    if compact {
        let gpu_bar = create_bar(gpu_pct, 15);
        let mem_bar = create_bar(mem_pct, 15);

        let text = Line::from(vec![
            Span::styled(format!("GPU{} ", gpu.index), Style::default().fg(Color::Cyan)),
            Span::styled(gpu_bar, Style::default().fg(usage_color(gpu_pct))),
            Span::raw(format!(" {:3}%", gpu.gpu_utilization)),
            Span::raw("  "),
            Span::styled("MEM ", Style::default().fg(Color::Magenta)),
            Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
            Span::raw(format!(" {:3}%", mem_pct as u32)),
            Span::raw(format!(" {}°C {}W", gpu.temperature, gpu.power_usage)),
        ]);

        frame.render_widget(Paragraph::new(text), area);
    } else {
        let title = format!("GPU {} - {} [{}]", gpu.index, gpu.name, gpu.pstate);

        let gpu_bar = create_bar(gpu_pct, 20);
        let mem_bar = create_bar(mem_pct, 20);

        let lines = vec![
            Line::from(vec![
                Span::styled("GPU  ", Style::default().fg(Color::Cyan)),
                Span::styled(gpu_bar, Style::default().fg(usage_color(gpu_pct))),
                Span::raw(format!(" {:3}%  ", gpu.gpu_utilization)),
                Span::styled("Temp: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{}°C", gpu.temperature), Style::default().fg(temp_color(gpu.temperature))),
                Span::raw("  "),
                Span::styled("Fan: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}%", gpu.fan_speed)),
            ]),
            Line::from(vec![
                Span::styled("MEM  ", Style::default().fg(Color::Magenta)),
                Span::styled(mem_bar, Style::default().fg(usage_color(mem_pct))),
                Span::raw(format!(" {:3}%  ", mem_pct as u32)),
                Span::raw(format!("{} / {}", format_size(gpu.memory_used, BINARY), format_size(gpu.memory_total, BINARY))),
            ]),
            Line::from(vec![
                Span::styled("Power: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}W / {}W  ", gpu.power_usage, gpu.power_limit)),
                Span::styled("Clocks: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{} MHz / {} MHz  ", gpu.sm_clock, gpu.mem_clock)),
                Span::styled("Enc/Dec: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}% / {}%", gpu.encoder_utilization, gpu.decoder_utilization)),
            ]),
        ];

        let block = Block::default().borders(Borders::ALL).title(title);
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn render_gpu_graphs(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else { return };
    if gpu_metrics.gpus.is_empty() { return; }

    let mut datasets = Vec::new();
    let colors = [Color::Cyan, Color::Magenta, Color::Green, Color::Yellow];

    // Collect data first to extend lifetimes
    let util_data: Vec<Vec<(f64, f64)>> = app.history.gpu_util_history
        .iter()
        .map(|h| h.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect())
        .collect();

    for (i, data) in util_data.iter().enumerate() {
        if i >= 4 { break; }
        datasets.push(
            Dataset::default()
                .name(format!("GPU{}", i))
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(colors[i % colors.len()]))
                .data(data)
        );
    }

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::ALL).title("GPU History"))
        .x_axis(Axis::default().bounds([0.0, 60.0]).labels::<Vec<Line>>(vec![]))
        .y_axis(Axis::default().bounds([0.0, 100.0]).labels(vec![Line::from("0%"), Line::from("50%"), Line::from("100%")]));

    frame.render_widget(chart, area);
}

fn render_gpu_processes(frame: &mut Frame, area: Rect, app: &mut App) {
    let procs = app.get_sorted_gpu_processes();
    let is_active = app.active_panel == ActivePanel::GpuProcesses;

    let sort_indicator = |col: SortColumn| -> &str {
        if app.gpu_sort == col {
            if app.sort_ascending { "▲" } else { "▼" }
        } else { "" }
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
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = procs.iter().map(|p| {
        let type_color = if p.process_type == "C" { Color::Green } else { Color::Blue };

        Row::new(vec![
            Cell::from(format!("{}", p.pid)),
            Cell::from(format!("{}", p.gpu_index)),
            Cell::from(p.process_type.clone()).style(Style::default().fg(type_color)),
            Cell::from(p.user.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(format_size(p.gpu_memory, BINARY)),
            Cell::from(p.name.clone()).style(Style::default().fg(Color::Green)),
            Cell::from(truncate_string(&p.command, 40)),
        ])
    }).collect();

    let title = format!("GPU Processes ({}) [{}]", procs.len(), if is_active { "ACTIVE" } else { "inactive" });
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let table = Table::new(rows, [
        Constraint::Length(7),
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(15),
        Constraint::Min(20),
    ])
    .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

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
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![
            Span::styled("nvglances", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - System and GPU Monitor"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("Navigation:", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  Tab          Switch between CPU and GPU process panels"),
        Line::from("  j/↓          Move selection down"),
        Line::from("  k/↑          Move selection up"),
        Line::from("  PgDn/PgUp    Move selection by page"),
        Line::from("  Home/End     Jump to first/last item"),
        Line::from(""),
        Line::from(vec![Span::styled("Sorting:", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  1            Sort by PID"),
        Line::from("  2            Sort by Name"),
        Line::from("  3            Sort by User"),
        Line::from("  4            Sort by CPU%"),
        Line::from("  5            Sort by Memory%"),
        Line::from("  6            Sort by GPU Memory"),
        Line::from("  r            Reverse sort order"),
        Line::from(""),
        Line::from(vec![Span::styled("Display:", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  a            Toggle show all processes"),
        Line::from("  g            Toggle graphs"),
        Line::from("  c            Toggle compact mode"),
        Line::from("  +/-          Adjust refresh rate"),
        Line::from(""),
        Line::from(vec![Span::styled("Other:", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  ?/F1         Show this help"),
        Line::from("  q/Esc        Quit"),
        Line::from(""),
        Line::from(vec![Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))]),
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
    frame.render_widget(ratatui::widgets::Clear, help_area);
    frame.render_widget(paragraph, help_area);
}

// ============================================================================
// Helpers
// ============================================================================

fn usage_color(pct: f64) -> Color {
    if pct >= 90.0 {
        Color::Red
    } else if pct >= 70.0 {
        Color::Yellow
    } else if pct >= 50.0 {
        Color::Cyan
    } else {
        Color::Green
    }
}

fn temp_color(temp: u32) -> Color {
    if temp >= 85 {
        Color::Red
    } else if temp >= 70 {
        Color::Yellow
    } else if temp >= 50 {
        Color::Cyan
    } else {
        Color::Green
    }
}

fn create_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;

    if days > 0 {
        format!("{}d {:02}h {:02}m", days, hours, mins)
    } else if hours > 0 {
        format!("{:02}h {:02}m", hours, mins)
    } else {
        format!("{:02}m", mins)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Create app
    let mut app = App::new().context("Failed to initialize application")?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();

    while app.running {
        terminal.draw(|f| ui(f, app))?;

        let timeout = app.refresh_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));

        if event::poll(timeout).context("Failed to poll events")? {
            if let Event::Key(key) = event::read().context("Failed to read event")? {
                app.handle_key(key.code, key.modifiers);
            }
        }

        if last_tick.elapsed() >= app.refresh_rate {
            app.refresh_all()?;
            last_tick = Instant::now();
        }
    }

    Ok(())
}
