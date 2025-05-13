mod cli;
mod config_loader;
mod app_config;
mod camera_config;
mod errors;
mod camera;
mod core;
mod operations;
mod common;

use common::logging_setup;
use core::camera_manager::CameraManager;
use errors::AppError;
use log::{info, error};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Basic logging first, in case config loading or full logger setup fails.
    logging_setup::basic_env_logging_init(); 

    // Parse CLI arguments early for potential use in logging or config path
    let matches = cli::build_cli().get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").map(|s| s.as_str()).unwrap_or("config/default_config.yaml");
    
    // Attempt to load the full configuration
    let master_config = match config_loader::load_config(config_path) {
        Ok(cfg) => {
            // Now initialize logging fully with config and CLI args
            // This might re-init if basic_env_logging_init was too simple, or refine it.
            logging_setup::initialize_logging(Some(&cfg), &matches);
            info!("Full configuration loaded successfully from: {}", config_path);
            cfg
        }
        Err(e) => {
            // Full config failed, use CLI for logging if possible, otherwise basic has already run.
            // We don't have `cfg` here, so pass None for config to initialize_logging.
            logging_setup::initialize_logging(None, &matches);
            error!("Failed to load master configuration from '{}': {}. Exiting.", config_path, e);
            return Err(e); // Propagate the config error
        }
    };

    info!("RCam starting with {} cameras configured.", master_config.cameras.len());

    // Initialize CameraManager
    let camera_manager = match CameraManager::new(&master_config) {
        Ok(manager) => manager,
        Err(e) => {
            error!("Failed to initialize CameraManager: {}. Exiting.", e);
            return Err(e);
        }
    };

    // Dispatch based on subcommand
    if let Some(subcommand_matches) = matches.subcommand() {
        let op_result = match subcommand_matches.0 {
            "capture-image" => {
                operations::image_capture_op::handle_capture_image_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "capture-video" => {
                operations::video_record_op::handle_record_video_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "verify-times" => {
                operations::time_sync_op::handle_verify_times_cli(&master_config, &camera_manager /*, subcommand_matches.1 */).await
            }
            "control" => {
                operations::camera_control_op::handle_control_camera_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            "test" => {
                operations::diagnostic_op::handle_diagnostic_cli(&master_config, &camera_manager, subcommand_matches.1).await
            }
            _ => {
                error!("Subcommand '{}' not implemented.", subcommand_matches.0);
                Err(AppError::Operation(format!("Subcommand '{}' not implemented.", subcommand_matches.0)))
            }
        };

        if let Err(e) = op_result {
            error!("Operation '{}' failed: {}", subcommand_matches.0, e);
            // Decide if this should cause an exit or just log
            // return Err(e); // Or handle more gracefully
        }

    } else {
        info!("No subcommand provided. RCam will now exit. In the future, this might start a default mode.");
        // Potentially print help: cli::build_cli().print_help().unwrap_or_default();
    }

    info!("RCam operations finished.");
    Ok(())
} 