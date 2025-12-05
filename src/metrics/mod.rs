//! Metrics collection modules.

mod system;
mod gpu;

pub use system::collect_system_metrics;
pub use gpu::collect_gpu_metrics;
