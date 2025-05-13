use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_entity::CameraEntity;
use anyhow::{Context, Result};
use clap::ArgMatches;
use log::{info, debug};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

/// Parses the camera names argument from CLI.
/// Returns Some(Vec<String>) if specific names are provided, None otherwise.
fn parse_camera_names_arg_local(specific_cameras_arg: Option<&String>) -> Option<Vec<String>> {
    specific_cameras_arg.map(|s| s.split(',').map(|name| name.trim().to_string()).collect())
}

/// Determines the target cameras based on CLI arguments or all available cameras.
pub async fn determine_target_cameras(
    camera_manager: &CameraManager,
    specific_cameras_arg: Option<&String>,
    operation_display_name: &str, // For logging context
) -> Result<Vec<Arc<Mutex<CameraEntity>>>> {
    let camera_parse_start = Instant::now();
    let camera_names_to_process = parse_camera_names_arg_local(specific_cameras_arg);
    debug!(
        "  Parsed camera selection for '{}' in {:?}. CLI arg: {:?}, Parsed: {:?}",
        operation_display_name, camera_parse_start.elapsed(), specific_cameras_arg, camera_names_to_process
    );

    let cameras_fetch_start = Instant::now();
    let cameras_to_target = match camera_names_to_process {
        Some(ref names) => {
            debug!("  Fetching specific cameras by names: {:?} for '{}'", names, operation_display_name);
            camera_manager.get_cameras_by_names(names).await
        }
        None => {
            debug!("  Fetching all available cameras for '{}'", operation_display_name);
            camera_manager.get_all_cameras().await
        }
    };
    debug!("  Fetched {} target cameras for '{}' in {:?}.", cameras_to_target.len(), operation_display_name, cameras_fetch_start.elapsed());
    Ok(cameras_to_target)
}

/// Determines and creates the operation's base output directory.
pub fn determine_operation_output_dir(
    master_config: &MasterConfig,
    args: &ArgMatches,
    output_cli_arg_key: &str,
    default_output_subdir: Option<&str>,
    operation_display_name: &str, // For logging context
) -> Result<PathBuf> {
    let output_dir_determine_start = Instant::now();
    let operation_base_output_dir: PathBuf = match args.get_one::<String>(output_cli_arg_key) {
        Some(path_str) => {
            debug!("  Output directory specified via CLI for '{}': {}", operation_display_name, path_str);
            PathBuf::from(path_str)
        }
        None => {
            let mut dir = PathBuf::from(&master_config.app_settings.output_directory);
            if let Some(subdir) = default_output_subdir {
                dir.push(subdir);
                debug!("  Using default output directory with subdir for '{}': {}", operation_display_name, dir.display());
            } else {
                debug!("  Using default output directory for '{}': {}", operation_display_name, dir.display());
            }
            dir
        }
    };
    debug!("  Determined operation base output directory for '{}' as '{}' in {:?}.", operation_display_name, operation_base_output_dir.display(), output_dir_determine_start.elapsed());

    if !operation_base_output_dir.exists() {
        info!("📁 Output directory {} does not exist. Creating it for '{}'.", operation_base_output_dir.display(), operation_display_name);
        let create_dir_start = Instant::now();
        std::fs::create_dir_all(&operation_base_output_dir)
            .with_context(|| format!(
                    "❌ Failed to create output directory '{}' for '{}'",
                    operation_base_output_dir.display(), operation_display_name
            ))?;
        info!("  ✅ Created output directory '{}' for '{}' in {:?}", operation_base_output_dir.display(), operation_display_name, create_dir_start.elapsed());
    } else {
        info!("ℹ️ Using existing output directory: {} for '{}'", operation_base_output_dir.display(), operation_display_name);
    }
    Ok(operation_base_output_dir)
}