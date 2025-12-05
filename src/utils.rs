//! Utility functions for formatting and display.

use ratatui::style::Color;

/// Get a color based on usage percentage.
pub fn usage_color(pct: f64) -> Color {
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

/// Get a color based on temperature.
pub fn temp_color(temp: u32) -> Color {
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

/// Create a text-based progress bar.
pub fn create_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Format a duration in seconds to a human-readable string.
pub fn format_duration(secs: u64) -> String {
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

/// Truncate a string to a maximum length with ellipsis.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
