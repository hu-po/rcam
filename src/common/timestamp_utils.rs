use chrono::{DateTime, Local};
use log::debug;
use std::time::Instant;

// Get current local timestamp as a formatted string
pub fn current_local_timestamp_str(format_str: &str) -> String {
    debug!("🕒 Generating timestamp with format: {}", format_str);
    let start_time = Instant::now();
    let now: DateTime<Local> = Local::now();
    let formatted = now.format(format_str).to_string();
    debug!("Generated timestamp \'{}\' in {:?}", formatted, start_time.elapsed());
    formatted
}