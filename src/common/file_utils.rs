use crate::common::timestamp_utils;
use log::debug;
use std::time::Instant;

pub fn generate_timestamped_filename(
    base_name: &str,      // e.g., camera name
    timestamp_format: &str, // from config, e.g., "%Y%m%d_%H%M%S"
    extension: &str,      // e.g., "jpg", "mp4"
) -> String {
    debug!("🏷️ Generating timestamped filename for base: \'{}\', ext: \'{}\'", base_name, extension);
    let start_time = Instant::now();
    let timestamp = timestamp_utils::current_local_timestamp_str(timestamp_format);
    let filename = format!("{}_{}.{}", base_name, timestamp, extension);
    debug!("Generated filename \'{}\' in {:?}", filename, start_time.elapsed());
    filename
}