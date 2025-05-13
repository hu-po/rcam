use chrono::{DateTime, Local, Utc};

// Get current local timestamp as a formatted string
pub fn current_local_timestamp_str(format_str: &str) -> String {
    let now: DateTime<Local> = Local::now();
    now.format(format_str).to_string()
}

// Get current UTC timestamp as a formatted string
pub fn current_utc_timestamp_str(format_str: &str) -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format(format_str).to_string()
}