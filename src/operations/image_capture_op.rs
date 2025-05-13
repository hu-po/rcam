use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::Result;
use crate::common::file_utils;
use crate::operations::op_helper::run_generic_camera_op;
use clap::ArgMatches;
use log::{info, error};
use std::time::Duration;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let delay_option = args.get_one::<u64>("delay").map(|&s| Duration::from_secs(s));
    
    let media_manager = CameraMediaManager::new();

    run_generic_camera_op(
        master_config,
        camera_manager,
        args,
        "Image Capture",
        "output",
        Some("images"),
        move |cam_entity_arc, app_settings_arc, operation_output_dir| {
            let media_manager_clone = media_manager.clone();
            let delay_clone = delay_option.clone();

            async move {
                let mut cam_entity = cam_entity_arc.lock().await;
                
                let filename = file_utils::generate_timestamped_filename(
                    &cam_entity.config.name,
                    &app_settings_arc.filename_timestamp_format,
                    &app_settings_arc.image_format
                );
                let output_path = operation_output_dir.join(filename);
                
                info!("Attempting to capture image for '{}' to {}", cam_entity.config.name, output_path.display());

                match media_manager_clone.capture_image(
                    &mut *cam_entity, 
                    &app_settings_arc,
                    output_path.clone(), 
                    delay_clone
                ).await {
                    Ok(path) => {
                        info!("Successfully captured image for '{}' to {}", cam_entity.config.name, path.display());
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to capture image for '{}': {:#}", cam_entity.config.name, e);
                        Err(e)
                    }
                }
            }
        },
    )
    .await
} 