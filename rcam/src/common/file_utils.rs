use crate::common::timestamp_utils;
use std::path::{PathBuf};
use crate::errors::AppError;
use log::debug;
use std::fs;
use chrono::Local;

pub fn generate_timestamped_filename(
    base_name: &str,      // e.g., camera name
    timestamp_format: &str, // from config, e.g., "%Y%m%d_%H%M%S"
    extension: &str,      // e.g., "jpg", "mp4"
) -> String {
    let timestamp = timestamp_utils::current_local_timestamp_str(timestamp_format);
    format!("{}_{}.{}", base_name, timestamp, extension)
}

pub fn ensure_output_directory(dir_path_str: &str) -> Result<PathBuf, AppError> {
    let dir_path = PathBuf::from(dir_path_str);
    if !dir_path.exists() {
        debug!("Output directory '{}' does not exist, attempting to create it.", dir_path.display());
        std::fs::create_dir_all(&dir_path).map_err(|e| {
            AppError::Io(format!(
                "Failed to create output directory '{}': {}",
                dir_path.display(),
                e
            ))
        })?;
    } else if !dir_path.is_dir() {
        return Err(AppError::Io(format!(
            "Output path '{}' exists but is not a directory.",
            dir_path.display()
        )));
    }
    Ok(dir_path)
}

// Future placeholder for file rotation logic (e.g., deleting oldest files to maintain storage limit)
// pub fn manage_storage_retention(
//     output_directory: &Path,
//     max_storage_gb: u64, 
//     camera_name: Option<&str> // If retention is per-camera
// ) -> Result<(), AppError> {
//     // 1. Calculate current storage used (for this camera or globally).
//     // 2. List files sorted by creation/modification time.
//     // 3. If over limit, delete oldest files until under limit.
//     warn!("Storage retention management not yet implemented for directory: {}", output_directory.display());
//     Ok(())
// } 