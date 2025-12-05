//! GPU metrics collection - supports NVML (Linux/Windows) and Metal (macOS).

use std::collections::HashMap;
use sysinfo::{Pid, System, Users};

use crate::types::{GpuBackend, GpuInfo, GpuMetrics};

#[cfg(not(target_os = "macos"))]
use crate::types::GpuProcessInfo;

// ============================================================================
// NVML Backend (Linux/Windows)
// ============================================================================

#[cfg(not(target_os = "macos"))]
mod nvml_backend {
    use super::*;
    use nvml_wrapper::Nvml;

    /// GPU backend handle for NVML.
    pub struct GpuHandle {
        pub nvml: Option<Nvml>,
    }

    impl GpuHandle {
        pub fn new() -> Self {
            Self {
                nvml: Nvml::init().ok(),
            }
        }
    }

    /// Collect GPU metrics from NVML.
    pub fn collect_gpu_metrics(
        handle: &GpuHandle,
        system: &System,
        users: &Users,
    ) -> Option<GpuMetrics> {
        let nvml = handle.nvml.as_ref()?;

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

            let utilization = device.utilization_rates().unwrap_or(
                nvml_wrapper::struct_wrappers::device::Utilization { gpu: 0, memory: 0 },
            );
            let memory_info =
                device
                    .memory_info()
                    .unwrap_or(nvml_wrapper::struct_wrappers::device::MemoryInfo {
                        free: 0,
                        total: 1,
                        used: 0,
                    });

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
            api_version: cuda_version,
            backend: GpuBackend::Nvml,
        })
    }
}

// ============================================================================
// Metal Backend (macOS)
// ============================================================================

#[cfg(target_os = "macos")]
mod metal_backend {
    use super::*;
    use metal::Device;
    use std::process::Command;

    /// GPU backend handle for Metal.
    pub struct GpuHandle {
        pub devices: Vec<Device>,
    }

    impl GpuHandle {
        pub fn new() -> Self {
            Self {
                devices: Device::all(),
            }
        }
    }

    /// Get macOS GPU driver info via system_profiler.
    fn get_macos_gpu_info() -> (String, u64, u64) {
        // Try to get GPU info from system_profiler
        let output = Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output();

        if let Ok(output) = output {
            if let Ok(json_str) = String::from_utf8(output.stdout) {
                // Parse basic info from JSON - look for Metal family version
                // This is a simplified parser
                if let Some(metal_idx) = json_str.find("spdisplays_metal") {
                    if let Some(end) = json_str[metal_idx..].find(',') {
                        let metal_info = &json_str[metal_idx..metal_idx + end];
                        if let Some(family) = metal_info.split(':').nth(1) {
                            let family = family.trim().trim_matches('"');
                            return (family.to_string(), 0, 0);
                        }
                    }
                }
            }
        }

        ("N/A".to_string(), 0, 0)
    }

    /// Get GPU utilization from powermetrics (requires sudo, so we estimate instead).
    fn estimate_gpu_utilization() -> u32 {
        // On macOS, getting real GPU utilization requires elevated privileges.
        // We return 0 as a placeholder - the memory usage is more reliable.
        0
    }

    /// Collect GPU metrics from Metal.
    pub fn collect_gpu_metrics(
        handle: &GpuHandle,
        _system: &System,
        _users: &Users,
    ) -> Option<GpuMetrics> {
        if handle.devices.is_empty() {
            return None;
        }

        let mut gpus = Vec::new();
        let (driver_version, _, _) = get_macos_gpu_info();

        for (i, device) in handle.devices.iter().enumerate() {
            let name = device.name().to_string();

            // Metal provides recommended and current working set sizes
            let memory_total = device.recommended_max_working_set_size();
            let memory_used = device.current_allocated_size();

            // Calculate memory utilization percentage
            let memory_utilization = if memory_total > 0 {
                ((memory_used as f64 / memory_total as f64) * 100.0) as u32
            } else {
                0
            };

            // Metal doesn't provide these metrics directly
            let gpu_utilization = estimate_gpu_utilization();

            gpus.push(GpuInfo {
                index: i as u32,
                name,
                temperature: 0, // Not available via Metal API
                fan_speed: 0,   // Not available via Metal API
                power_usage: 0, // Not available via Metal API
                power_limit: 0, // Not available via Metal API
                gpu_utilization,
                memory_utilization,
                memory_used,
                memory_total,
                encoder_utilization: 0, // Not available via Metal API
                decoder_utilization: 0, // Not available via Metal API
                pcie_rx: 0,             // Not applicable for integrated GPUs
                pcie_tx: 0,             // Not applicable for integrated GPUs
                sm_clock: 0,            // Not available via Metal API
                mem_clock: 0,           // Not available via Metal API
                pstate: "N/A".to_string(),
            });
        }

        // Metal API version based on device capabilities
        let api_version = if let Some(device) = handle.devices.first() {
            // Check for Metal 3 support (Apple Silicon)
            if device.supports_family(metal::MTLGPUFamily::Metal3) {
                "Metal 3".to_string()
            } else if device.supports_family(metal::MTLGPUFamily::Apple7) {
                "Metal 2 (Apple7)".to_string()
            } else if device.supports_family(metal::MTLGPUFamily::Apple6) {
                "Metal 2 (Apple6)".to_string()
            } else if device.supports_family(metal::MTLGPUFamily::Apple5) {
                "Metal 2 (Apple5)".to_string()
            } else {
                "Metal".to_string()
            }
        } else {
            "Metal".to_string()
        };

        // Note: Metal doesn't provide per-process GPU memory tracking
        // Process tracking would require IOKit or elevated privileges
        let processes = Vec::new();

        Some(GpuMetrics {
            gpus,
            processes,
            driver_version,
            api_version,
            backend: GpuBackend::Metal,
        })
    }
}

// ============================================================================
// Common utilities
// ============================================================================

/// Get process info from sysinfo by PID.
#[allow(dead_code)]
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

// ============================================================================
// Public API
// ============================================================================

#[cfg(not(target_os = "macos"))]
pub use nvml_backend::GpuHandle;

#[cfg(target_os = "macos")]
pub use metal_backend::GpuHandle;

/// Collect GPU metrics using the appropriate backend for the platform.
#[cfg(not(target_os = "macos"))]
pub fn collect_gpu_metrics(
    handle: &GpuHandle,
    system: &System,
    users: &Users,
) -> Option<GpuMetrics> {
    nvml_backend::collect_gpu_metrics(handle, system, users)
}

#[cfg(target_os = "macos")]
pub fn collect_gpu_metrics(
    handle: &GpuHandle,
    system: &System,
    users: &Users,
) -> Option<GpuMetrics> {
    metal_backend::collect_gpu_metrics(handle, system, users)
}
