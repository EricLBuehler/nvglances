//! System metrics collection (CPU, memory, disk, network, processes).

use std::collections::HashMap;
use std::time::Duration;
use sysinfo::{Components, Disks, Networks, ProcessStatus, System, Users};

use crate::types::{CpuInfo, DiskInfo, MemoryInfo, NetworkInfo, ProcessInfo, SystemMetrics};

/// Collect all system metrics.
pub fn collect_system_metrics(
    system: &System,
    networks: &Networks,
    disks: &Disks,
    components: &Components,
    users: &Users,
    last_network_stats: &mut HashMap<String, (u64, u64)>,
    elapsed: Duration,
) -> SystemMetrics {
    let elapsed_secs = elapsed.as_secs_f64().max(0.001);

    // Hostname and OS info
    let hostname = System::host_name().unwrap_or_else(|| "unknown".into());
    let os_name = System::long_os_version().unwrap_or_else(|| "Unknown OS".into());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "?".into());

    // Uptime and load
    let uptime = System::uptime();
    let load = System::load_average();
    let load_avg = (load.one, load.five, load.fifteen);

    // CPUs
    let cpus: Vec<CpuInfo> = system
        .cpus()
        .iter()
        .map(|cpu| CpuInfo {
            name: cpu.name().to_string(),
            usage: cpu.cpu_usage(),
            frequency: cpu.frequency(),
        })
        .collect();

    let cpu_global = system.global_cpu_usage();

    // Memory
    let memory = MemoryInfo {
        total: system.total_memory(),
        used: system.used_memory(),
        available: system.available_memory(),
        swap_total: system.total_swap(),
        swap_used: system.used_swap(),
    };

    // Disks
    let disks_info: Vec<DiskInfo> = disks
        .iter()
        .map(|disk| DiskInfo {
            name: disk.name().to_string_lossy().to_string(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            total: disk.total_space(),
            used: disk.total_space() - disk.available_space(),
            fs_type: disk.file_system().to_string_lossy().to_string(),
        })
        .collect();

    // Networks
    let networks_info: Vec<NetworkInfo> = networks
        .iter()
        .filter(|(name, _)| !name.starts_with("lo"))
        .map(|(name, data)| {
            let (prev_rx, prev_tx) = last_network_stats
                .get(name)
                .copied()
                .unwrap_or((data.total_received(), data.total_transmitted()));

            let rx_bytes = data.total_received();
            let tx_bytes = data.total_transmitted();
            let rx_rate = (rx_bytes.saturating_sub(prev_rx)) as f64 / elapsed_secs;
            let tx_rate = (tx_bytes.saturating_sub(prev_tx)) as f64 / elapsed_secs;

            last_network_stats.insert(name.clone(), (rx_bytes, tx_bytes));

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
    let temperatures: Vec<(String, f32)> = components
        .iter()
        .filter_map(|c| {
            let temp = c.temperature();
            if temp > 0.0 {
                Some((c.label().to_string(), temp))
            } else {
                None
            }
        })
        .collect();

    // User map for process info
    let user_map: HashMap<_, _> = users
        .iter()
        .map(|u| (u.id().clone(), u.name().to_string()))
        .collect();

    // Processes
    let total_memory = system.total_memory();
    let processes: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .map(|(pid, proc)| {
            let user = proc
                .user_id()
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
            }
            .to_string();

            let cmd: Vec<_> = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();
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
                memory_usage: (proc.memory() as f32 / total_memory as f32) * 100.0,
                memory_bytes: proc.memory(),
                status,
                command,
            }
        })
        .collect();

    let process_count = system.processes().len();
    let thread_count = system.processes().len();

    SystemMetrics {
        hostname,
        os_name,
        kernel_version,
        uptime,
        load_avg,
        cpus,
        cpu_global,
        memory,
        disks: disks_info,
        networks: networks_info,
        processes,
        process_count,
        thread_count,
        temperatures,
    }
}
