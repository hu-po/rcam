mod cli;
mod config_loader;
mod app_config;
mod camera_config;
mod camera;
mod core;
mod operations;
mod common;

use common::logging_setup;
use core::camera_manager::CameraManager;
use log::{info, error};
use anyhow::{Context, Result, bail};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments early for potential use in logging or config path
    let matches = cli::build_cli().get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").map(|s| s.as_str()).unwrap_or("config/default_config.yaml");
    
    // Attempt to load the full configuration
    let master_config = match config_loader::load_config(config_path) {
        Ok(cfg) => {
            logging_setup::initialize_logging(Some(&cfg), &matches)
                .context("Failed to initialize logging with full config")?;
            info!("Full configuration loaded successfully from: {}", config_path);
            cfg
        }
        Err(e) => {
            // Try to initialize logging with CLI args only, or defaults
            logging_setup::initialize_logging(None, &matches)
                .context("Failed to initialize logging with basic settings after config load failure")?;
            error!("Failed to load master configuration from '{}': {:#}. Exiting.", config_path, e);
            // Attach context to the existing anyhow::Error
            return Err(e.context(format!("Failed to load master configuration from '{}'", config_path)));
        }
    };

    info!("RCam starting with {} cameras configured.", master_config.cameras.len());

    // Initialize CameraManager
    let camera_manager = CameraManager::new(&master_config)
        .context("Failed to initialize CameraManager")?;

    // Dispatch based on subcommand
    if let Some(subcommand_matches) = matches.subcommand() {
        let op_result: Result<()> = match subcommand_matches.0 {
            "capture-image" => {
                operations::image_capture_op::handle_capture_image_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "capture-video" => {
                operations::video_record_op::handle_record_video_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "verify-times" => {
                operations::time_sync_op::handle_verify_times_cli(&master_config, &camera_manager).await
            }
            "control" => {
                operations::camera_control_op::handle_control_camera_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "test" => {
                operations::diagnostic_op::handle_diagnostic_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            _ => {
                let sub_cmd_name = subcommand_matches.0;
                bail!("Subcommand '{}' not implemented.", sub_cmd_name)
            }
        };

        if let Err(e) = op_result {
            error!("Operation '{}' failed: {:#}", subcommand_matches.0, e);
            return Err(e);
        }

    } else {
        info!("No subcommand provided. RCam will now exit. In the future, this might start a default mode.");
    }

    info!("RCam operations finished.");
    Ok(())
} 