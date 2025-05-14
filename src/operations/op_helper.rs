use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::core::capture_source::CaptureSource;
use anyhow::{Context, Result};
use clap::ArgMatches;
use log::{info, debug, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

/// Determines the target cameras based on CLI arguments or all available cameras.
pub async fn determine_target_devices(
    camera_manager: &CameraManager,
    specific_devices_arg: Option<&String>,
    operation_display_name: &str,
) -> Result<Vec<Arc<Mutex<dyn CaptureSource + Send>>>> {
    debug!(
        "Determining target devices for '{}'. Specific devices arg: {:?}",
        operation_display_name,
        specific_devices_arg
    );

    let devices_to_target: Vec<Arc<Mutex<dyn CaptureSource + Send>>>;

    if let Some(specific_names_str) = specific_devices_arg {
        if specific_names_str.to_lowercase() == "all" {
            info!(
                "Targeting all available/configured devices for '{}'.",
                operation_display_name
            );
            devices_to_target = camera_manager.get_all_devices().await;
        } else {
            let device_names: Vec<String> = specific_names_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            info!(
                "Targeting specific devices for '{}': {:?}",
                operation_display_name,
                device_names
            );
            devices_to_target = camera_manager.get_devices_by_names(&device_names).await;
        }
    } else {
        warn!(
            "No specific devices argument provided for '{}'. Defaulting to all available devices.",
            operation_display_name
        );
        devices_to_target = camera_manager.get_all_devices().await;
    }

    if devices_to_target.is_empty() {
        warn!(
            "No devices were ultimately targeted for '{}' (either none configured, none matched, or none specified and none available).",
            operation_display_name
        );
    } else {
        info!(
            "Targeting {} device(s) for '{}'.",
            devices_to_target.len(),
            operation_display_name
        );
    }
    Ok(devices_to_target)
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
            let mut dir = PathBuf::from(&master_config.application.output_directory_base);
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