use crate::common::timestamp_utils;

pub fn generate_timestamped_filename(
    base_name: &str,      // e.g., camera name
    timestamp_format: &str, // from config, e.g., "%Y%m%d_%H%M%S"
    extension: &str,      // e.g., "jpg", "mp4"
) -> String {
    let timestamp = timestamp_utils::current_local_timestamp_str(timestamp_format);
    format!("{}_{}.{}", base_name, timestamp, extension)
}