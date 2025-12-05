//! Data types and structures used throughout nvglances.

/// CPU core information.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct CpuInfo {
    pub name: String,
    pub usage: f32,
    pub frequency: u64,
}

/// Memory and swap information.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

/// Disk/filesystem information.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub fs_type: String,
}

/// Network interface information.
#[derive(Clone, Default)]
pub struct NetworkInfo {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_rate: f64,
    pub tx_rate: f64,
}

/// Process information.
#[derive(Clone, Default)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub memory_bytes: u64,
    pub status: String,
    pub command: String,
}

/// GPU information from NVML.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub temperature: u32,
    pub fan_speed: u32,
    pub power_usage: u32,
    pub power_limit: u32,
    pub gpu_utilization: u32,
    pub memory_utilization: u32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub encoder_utilization: u32,
    pub decoder_utilization: u32,
    pub pcie_rx: u64,
    pub pcie_tx: u64,
    pub sm_clock: u32,
    pub mem_clock: u32,
    pub pstate: String,
}

/// GPU process information.
#[derive(Clone, Default)]
pub struct GpuProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub gpu_index: u32,
    pub gpu_memory: u64,
    pub sm_utilization: Option<u32>,
    pub command: String,
    pub process_type: String,
}

/// Aggregated system metrics.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct SystemMetrics {
    pub hostname: String,
    pub os_name: String,
    pub kernel_version: String,
    pub uptime: u64,
    pub load_avg: (f64, f64, f64),
    pub cpus: Vec<CpuInfo>,
    pub cpu_global: f32,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
    pub processes: Vec<ProcessInfo>,
    pub process_count: usize,
    pub thread_count: usize,
    pub temperatures: Vec<(String, f32)>,
}

/// Aggregated GPU metrics.
#[derive(Clone, Default)]
pub struct GpuMetrics {
    pub gpus: Vec<GpuInfo>,
    pub processes: Vec<GpuProcessInfo>,
    pub driver_version: String,
    pub cuda_version: String,
}

/// Historical data for graphs.
pub struct HistoryData {
    pub cpu_history: Vec<f64>,
    pub memory_history: Vec<f64>,
    pub gpu_util_history: Vec<Vec<f64>>,
    pub gpu_mem_history: Vec<Vec<f64>>,
    pub network_rx_history: Vec<f64>,
    pub network_tx_history: Vec<f64>,
}

impl Default for HistoryData {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryData {
    /// Create a new HistoryData with 60-second buffers.
    pub fn new() -> Self {
        Self {
            cpu_history: vec![0.0; 60],
            memory_history: vec![0.0; 60],
            gpu_util_history: Vec::new(),
            gpu_mem_history: Vec::new(),
            network_rx_history: vec![0.0; 60],
            network_tx_history: vec![0.0; 60],
        }
    }

    /// Push a CPU usage value.
    pub fn push_cpu(&mut self, value: f64) {
        self.cpu_history.remove(0);
        self.cpu_history.push(value);
    }

    /// Push a memory usage value.
    pub fn push_memory(&mut self, value: f64) {
        self.memory_history.remove(0);
        self.memory_history.push(value);
    }

    /// Push a GPU utilization value for a specific GPU.
    pub fn push_gpu_util(&mut self, gpu_idx: usize, value: f64) {
        while self.gpu_util_history.len() <= gpu_idx {
            self.gpu_util_history.push(vec![0.0; 60]);
        }
        self.gpu_util_history[gpu_idx].remove(0);
        self.gpu_util_history[gpu_idx].push(value);
    }

    /// Push a GPU memory usage value for a specific GPU.
    pub fn push_gpu_mem(&mut self, gpu_idx: usize, value: f64) {
        while self.gpu_mem_history.len() <= gpu_idx {
            self.gpu_mem_history.push(vec![0.0; 60]);
        }
        self.gpu_mem_history[gpu_idx].remove(0);
        self.gpu_mem_history[gpu_idx].push(value);
    }

    /// Push network throughput values.
    pub fn push_network(&mut self, rx: f64, tx: f64) {
        self.network_rx_history.remove(0);
        self.network_rx_history.push(rx);
        self.network_tx_history.remove(0);
        self.network_tx_history.push(tx);
    }
}

/// Sort column for process tables.
#[derive(PartialEq, Clone, Copy)]
pub enum SortColumn {
    Pid,
    Name,
    User,
    Cpu,
    Memory,
    GpuMemory,
}

/// Which process panel is active.
#[derive(PartialEq, Clone, Copy)]
pub enum ActivePanel {
    CpuProcesses,
    GpuProcesses,
}

/// Kill confirmation dialog state.
#[derive(Clone)]
pub struct KillConfirmation {
    pub pid: u32,
    pub name: String,
    pub signal: sysinfo::Signal,
}
