use crate::config_loader::MasterConfig;
use crate::core::camera_manager::{CameraManager, parse_camera_names_arg};
use crate::camera::camera_media::CameraMediaManager; // MediaBackend import removed
use crate::errors::AppError;
use crate::common::file_utils; // For generate_timestamped_filename
use clap::ArgMatches;
use log::{info, error, warn};
use std::path::PathBuf;
use std::time::Duration;
use futures::future::join_all;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<(), AppError> {
    info!("Handling capture-image command...");

    let media_manager = CameraMediaManager::new(); // Simplified instantiation

    let specific_cameras_arg = args.get_one::<String>("cameras");
    let camera_names_to_process = parse_camera_names_arg(specific_cameras_arg);

    let cameras_to_target = match camera_names_to_process {
        Some(ref names) => camera_manager.get_cameras_by_names(names).await,
        None => camera_manager.get_all_cameras().await,
    };

    if cameras_to_target.is_empty() {
        if let Some(names) = camera_names_to_process {
            warn!("No cameras found matching names: {:?}. Please check your camera names and configuration.", names);
        } else {
            warn!("No cameras configured or matched for image capture.");
        }
        return Ok(());
    }

    let base_output_dir: PathBuf = match args.get_one::<String>("output") {
        Some(path_str) => PathBuf::from(path_str),
        None => PathBuf::from(&master_config.app_settings.output_directory),
    };
    
    // Ensure base output directory exists
    if !base_output_dir.exists() {
        std::fs::create_dir_all(&base_output_dir).map_err(|e| AppError::Io(format!("Failed to create output directory '{}': {}", base_output_dir.display(), e)))?;
    }

    let mut capture_tasks = Vec::new();

    let delay_option = args.get_one::<u64>("delay").map(|&s| Duration::from_secs(s));

    for cam_entity_arc in cameras_to_target {
        let app_config_clone = master_config.app_settings.clone(); 
        let media_manager_arc = media_manager.clone(); 
        let base_output_dir_clone = base_output_dir.clone();
        let delay_option_clone = delay_option.clone(); // Clone delay for each task

        let task = tokio::spawn(async move {
            let mut cam_entity = cam_entity_arc.lock().await;
            let filename = file_utils::generate_timestamped_filename(
                &cam_entity.config.name, 
                &app_config_clone.filename_timestamp_format,
                &app_config_clone.image_format
            );
            let output_path = base_output_dir_clone.join(filename);
            
            // Note: The delay is now handled inside capture_image method of CameraMediaManager
            // However, if the user wants a global delay *before* any camera starts, the old logic was fine.
            // For per-camera delay or more fine-grained control, it should be in CameraMediaManager.
            // The current `delay` CLI arg is a global pre-delay. Let's adjust to pass it to capture_image.

            match media_manager_arc.capture_image(&mut *cam_entity, &app_config_clone, output_path.clone(), delay_option_clone).await {
                Ok(path) => info!("Successfully captured image for '{}' to {}", cam_entity.config.name, path.display()),
                Err(e) => error!("Failed to capture image for '{}': {}", cam_entity.config.name, e),
            }
        });
        capture_tasks.push(task);
    }

    join_all(capture_tasks).await;
    info!("Image capture tasks completed.");

    Ok(())
} 