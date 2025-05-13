use crate::config_loader::MasterConfig;
use crate::core::camera_manager::{CameraManager, parse_camera_names_arg};
use crate::camera::camera_media::CameraMediaManager;
use crate::errors::AppError;
use crate::common::file_utils;
use clap::ArgMatches;
use log::{info, error, warn};
use std::path::PathBuf;
use std::time::Duration;
use futures::future::join_all;

pub async fn handle_record_video_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<(), AppError> {
    info!("Handling record-video command...");

    let media_manager = CameraMediaManager::new();

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
            warn!("No cameras configured or matched for video recording.");
        }
        return Ok(());
    }

    let duration_seconds = args.get_one::<u64>("duration")
        .copied()
        .unwrap_or(master_config.app_settings.video_duration_default_seconds as u64);
    let recording_duration = Duration::from_secs(duration_seconds);

    let output_dir_override = args.get_one::<String>("output").map(PathBuf::from);
    let base_output_dir = output_dir_override.unwrap_or_else(|| PathBuf::from(&master_config.app_settings.output_directory));
    
    if !base_output_dir.exists() {
        std::fs::create_dir_all(&base_output_dir).map_err(|e| AppError::Io(format!("Failed to create output directory '{}': {}", base_output_dir.display(), e)))?;
    }

    let mut record_tasks = Vec::new();

    for cam_entity_arc in cameras_to_target {
        let app_config_clone = master_config.app_settings.clone();
        let media_manager_arc = media_manager.clone(); 
        let base_output_dir_clone = base_output_dir.clone();
        let recording_duration_clone = recording_duration;

        let task = tokio::spawn(async move {
            let mut cam_entity = cam_entity_arc.lock().await;
            let filename = file_utils::generate_timestamped_filename(
                &cam_entity.config.name, 
                &app_config_clone.filename_timestamp_format,
                &app_config_clone.video_format // Use video_format here
            );
            let output_path = base_output_dir_clone.join(filename);

            info!("Preparing to record video for '{}' to {} for {:?}", cam_entity.config.name, output_path.display(), recording_duration_clone);

            match media_manager_arc.record_video(&mut *cam_entity, &app_config_clone, output_path.clone(), recording_duration_clone).await {
                Ok(path) => info!("Successfully recorded video for '{}' to {}", cam_entity.config.name, path.display()),
                Err(e) => error!("Failed to record video for '{}': {}", cam_entity.config.name, e),
            }
        });
        record_tasks.push(task);
    }

    join_all(record_tasks).await;
    info!("Video recording tasks completed.");

    Ok(())
} 