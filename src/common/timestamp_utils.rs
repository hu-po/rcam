use chrono::{DateTime, Local};

// Get current local timestamp as a formatted string
pub fn current_local_timestamp_str(format_str: &str) -> String {
    let now: DateTime<Local> = Local::now();
    now.format(format_str).to_string()
}