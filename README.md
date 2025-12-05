# nvglances

A feature-complete terminal UI that combines the best of [glances](https://github.com/nicolargo/glances) and [nvitop](https://github.com/XuehaiPan/nvitop) - system and GPU monitoring in one tool.

**Supports NVIDIA GPUs (via CUDA/NVML) and Apple Silicon GPUs (via Metal).**

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

### System Monitoring
- **CPU**: Global usage, per-core stats, frequency, load averages
- **Memory**: RAM and swap usage with visual gauges
- **Disk**: Mount points, filesystem types, usage statistics
- **Network**: Interface traffic rates and totals
- **Processes**: Sortable process table with CPU/memory usage

### GPU Monitoring

#### NVIDIA GPUs (Linux/Windows via NVML)
- **Multi-GPU support**: Monitor all NVIDIA GPUs simultaneously
- **GPU metrics**: Utilization, temperature, fan speed, power draw
- **Memory**: VRAM usage per GPU
- **Clocks**: SM and memory clock frequencies
- **P-States**: Performance state display (P0-P15)
- **Encoder/Decoder**: Video engine utilization
- **PCIe throughput**: Data transfer rates
- **GPU processes**: Track processes using GPU resources

#### Apple Silicon GPUs (macOS via Metal)
- **Multi-GPU support**: Monitor all Metal-compatible GPUs
- **Memory**: GPU memory usage and allocation
- **Metal API version**: Displays Metal 3, Metal 2, etc.

### User Interface
- **Adaptive layout**: Automatically adjusts to terminal size
- **Compact mode**: Condensed view for smaller terminals
- **History graphs**: CPU and GPU utilization over time
- **Color-coded**: Visual indicators for resource usage levels
- **Mouse support**: Click to select, scroll to navigate
- **Process management**: Kill processes with confirmation dialog

## Installation

### From crates.io
```bash
cargo install nvglances
```

### From source

```bash
# Clone the repository
git clone https://github.com/EricLBuehler/nvglances.git
cd nvglances

# Build release binary
cargo build --release

# Install (optional)
cargo install --path .
```

### Requirements

- Rust 1.70 or later
- **Linux/Windows**: NVIDIA drivers for GPU monitoring
- **macOS**: Metal-compatible GPU (Apple Silicon or AMD)

## Usage

```bash
# Run nvglances
nvglances

# Or run directly from target
./target/release/nvglances
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `?` / `F1` | Show help |
| `q` / `Esc` | Quit |
| `Tab` | Switch between CPU and GPU process panels |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `PgDn` / `PgUp` | Move selection by page |
| `Home` / `End` | Jump to first/last item |
| `1-6` | Sort by column (PID, Name, User, CPU%, MEM%, GPU MEM) |
| `r` | Reverse sort order |
| `a` | Toggle show all processes |
| `g` | Toggle history graphs |
| `c` | Toggle compact mode |
| `+` / `-` | Adjust refresh rate |

### Process Control

| Key | Signal | Description |
|-----|--------|-------------|
| `Del` / `Ctrl+T` | SIGTERM | Graceful termination |
| `Ctrl+K` | SIGKILL | Force kill |
| `Ctrl+I` | SIGINT | Interrupt |

A confirmation dialog appears before killing any process.

### Mouse Support

- **Click** on process tables to select rows
- **Scroll** to navigate up/down in process lists

## Configuration

nvglances currently uses sensible defaults. Configuration file support may be added in future versions.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

### Development Setup

```bash
# Clone and enter directory
git clone https://github.com/yourusername/nvglances.git
cd nvglances

# Build in debug mode (faster compilation)
cargo build

# Run with debug output
RUST_BACKTRACE=1 cargo run

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

### Code Style

- Follow standard Rust conventions
- Use `cargo fmt` before committing
- Add comments for complex logic
- Keep functions focused and small

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by [glances](https://github.com/nicolargo/glances) and [nvitop](https://github.com/XuehaiPan/nvitop)
- Built with [ratatui](https://github.com/ratatui-org/ratatui)
