use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::Result;
use crate::common::file_utils;
use crate::operations::op_helper::run_generic_camera_op;
use clap::ArgMatches;
use log::{info, error};
use std::time::Duration;

pub async fn handle_record_video_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let duration_seconds = args
        .get_one::<u64>("duration")
        .copied()
        .unwrap_or(master_config.app_settings.video_duration_default_seconds as u64);
    let recording_duration = Duration::from_secs(duration_seconds);

    let media_manager = CameraMediaManager::new();

    run_generic_camera_op(
        master_config,
        camera_manager,
        args,
        "Video Recording",
        "output",
        Some("videos"),
        move |cam_entity_arc, app_settings_arc, operation_output_dir| {
            let media_manager_clone = media_manager.clone();
            let recording_duration_clone = recording_duration;

            async move {
                let mut cam_entity = cam_entity_arc.lock().await;
                
                let filename = file_utils::generate_timestamped_filename(
                    &cam_entity.config.name,
                    &app_settings_arc.filename_timestamp_format,
                    &app_settings_arc.video_format,
                );
                let output_path = operation_output_dir.join(filename);
                
                info!(
                    "Preparing to record video for '{}' to {} for {:?}",
                    cam_entity.config.name,
                    output_path.display(),
                    recording_duration_clone
                );

                match media_manager_clone
                    .record_video(
                        &mut *cam_entity,
                        &app_settings_arc,
                        output_path.clone(),
                        recording_duration_clone,
                    )
                    .await
                {
                    Ok(path) => {
                        info!(
                            "Successfully recorded video for '{}' to {}",
                            cam_entity.config.name,
                            path.display()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!(
                            "Failed to record video for '{}': {:#}",
                            cam_entity.config.name, e
                        );
                        Err(e)
                    }
                }
            }
        },
    )
    .await
} 