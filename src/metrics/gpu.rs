//! GPU metrics collection via NVML.

use nvml_wrapper::Nvml;
use sysinfo::{Pid, System, Users};
use std::collections::HashMap;

use crate::types::{GpuInfo, GpuMetrics, GpuProcessInfo};

/// Collect GPU metrics from NVML.
pub fn collect_gpu_metrics(
    nvml: &Option<Nvml>,
    system: &System,
    users: &Users,
) -> Option<GpuMetrics> {
    let nvml = nvml.as_ref()?;

    let device_count = nvml.device_count().ok()?;

    let driver_version = nvml.sys_driver_version().unwrap_or_else(|_| "N/A".into());
    let cuda_version = nvml
        .sys_cuda_driver_version()
        .map(|v| format!("{}.{}", v / 1000, (v % 1000) / 10))
        .unwrap_or_else(|_| "N/A".into());

    let mut gpus = Vec::new();
    let mut processes = Vec::new();

    for i in 0..device_count {
        let Ok(device) = nvml.device_by_index(i) else {
            continue;
        };

        let name = device.name().unwrap_or_else(|_| "Unknown GPU".into());
        let temperature = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);
        let fan_speed = device.fan_speed(0).unwrap_or(0);
        let power_usage = device.power_usage().unwrap_or(0) / 1000;
        let power_limit = device.power_management_limit().unwrap_or(0) / 1000;

        let utilization = device
            .utilization_rates()
            .unwrap_or(nvml_wrapper::struct_wrappers::device::Utilization { gpu: 0, memory: 0 });
        let memory_info = device.memory_info().unwrap_or(
            nvml_wrapper::struct_wrappers::device::MemoryInfo {
                free: 0,
                total: 1,
                used: 0,
            },
        );

        let encoder = device
            .encoder_utilization()
            .map(|e| e.utilization)
            .unwrap_or(0);
        let decoder = device
            .decoder_utilization()
            .map(|d| d.utilization)
            .unwrap_or(0);

        let pcie_tx = device
            .pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Send)
            .unwrap_or(0);
        let pcie_rx = device
            .pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Receive)
            .unwrap_or(0);

        let sm_clock = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .unwrap_or(0);
        let mem_clock = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory)
            .unwrap_or(0);

        let pstate = device
            .performance_state()
            .map(|p| {
                use nvml_wrapper::enum_wrappers::device::PerformanceState;
                match p {
                    PerformanceState::Zero => "P0",
                    PerformanceState::One => "P1",
                    PerformanceState::Two => "P2",
                    PerformanceState::Three => "P3",
                    PerformanceState::Four => "P4",
                    PerformanceState::Five => "P5",
                    PerformanceState::Six => "P6",
                    PerformanceState::Seven => "P7",
                    PerformanceState::Eight => "P8",
                    PerformanceState::Nine => "P9",
                    PerformanceState::Ten => "P10",
                    PerformanceState::Eleven => "P11",
                    PerformanceState::Twelve => "P12",
                    PerformanceState::Thirteen => "P13",
                    PerformanceState::Fourteen => "P14",
                    PerformanceState::Fifteen => "P15",
                    PerformanceState::Unknown => "P?",
                }
                .to_string()
            })
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
            pcie_tx: pcie_tx as u64 * 1024,
            sm_clock,
            mem_clock,
            pstate,
        });

        // Collect GPU processes
        if let Ok(compute_procs) = device.running_compute_processes() {
            for proc in compute_procs {
                let pid = proc.pid;
                let (name, user, command) = get_process_info(system, users, pid);

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
                let (name, user, command) = get_process_info(system, users, pid);

                // Skip if already added as compute process
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

    Some(GpuMetrics {
        gpus,
        processes,
        driver_version,
        cuda_version,
    })
}

/// Get process info from sysinfo by PID.
fn get_process_info(system: &System, users: &Users, pid: u32) -> (String, String, String) {
    let sys_pid = Pid::from_u32(pid);
    if let Some(proc) = system.process(sys_pid) {
        let user_map: HashMap<_, _> = users
            .iter()
            .map(|u| (u.id().clone(), u.name().to_string()))
            .collect();

        let user = proc
            .user_id()
            .and_then(|uid| user_map.get(uid))
            .cloned()
            .unwrap_or_else(|| "?".into());

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

        (proc.name().to_string_lossy().to_string(), user, command)
    } else {
        ("?".into(), "?".into(), "?".into())
    }
}
