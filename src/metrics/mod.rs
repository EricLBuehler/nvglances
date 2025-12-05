//! Metrics collection modules.

mod gpu;
mod system;

pub use gpu::{collect_gpu_metrics, GpuHandle};
pub use system::collect_system_metrics;
