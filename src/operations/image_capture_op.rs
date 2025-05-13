use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::Result;
use crate::common::file_utils;
use crate::operations::op_helper::run_generic_camera_op;
use clap::ArgMatches;
use log::{info, error, debug};
use std::time::Duration;
use std::time::Instant;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let delay_seconds_arg = args.get_one::<u64>("delay").copied();
    let delay_option = delay_seconds_arg.map(Duration::from_secs);
    debug!(
        "Capture image CLI: delay_arg: {:?}, cameras_arg: {:?}, output_arg: {:?}",
        delay_seconds_arg, args.get_one::<String>("cameras"), args.get_one::<String>("output")
    );
    info!("🖼️ Preparing to capture images from specified cameras{}.", delay_option.map_or_else(String::new, |d| format!(" with a delay of {:?}", d)));
    
    let media_manager_init_start = Instant::now();
    let media_manager = CameraMediaManager::new();
    debug!("CameraMediaManager initialized for image capture in {:?}.", media_manager_init_start.elapsed());

    let result = run_generic_camera_op(
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
                let cam_op_start_time = Instant::now();
                let mut cam_entity = cam_entity_arc.lock().await;
                let cam_name = cam_entity.config.name.clone();
                
                let filename = file_utils::generate_timestamped_filename(
                    &cam_entity.config.name,
                    &app_settings_arc.filename_timestamp_format,
                    &app_settings_arc.image_format
                );
                let output_path = operation_output_dir.join(filename);
                
                info!("📸 Attempting to capture image for '{}' to {}{}",
                    cam_name,
                    output_path.display(),
                    delay_clone.map_or_else(String::new, |d| format!(" after {:?} delay", d))
                );

                match media_manager_clone.capture_image(
                    &mut *cam_entity, 
                    &app_settings_arc,
                    output_path.clone(), 
                    delay_clone
                ).await {
                    Ok(path) => {
                        info!("✅ Successfully captured image for '{}' to {} in {:?}.",
                            cam_name, path.display(), cam_op_start_time.elapsed()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!("❌ Failed to capture image for '{}' after {:?}: {:#}",
                            cam_name, cam_op_start_time.elapsed(), e
                        );
                        Err(e)
                    }
                }
            }
        },
    )
    .await;

    if result.is_ok() {
        info!("🖼️ All image capture operations completed successfully in {:?}.", op_start_time.elapsed());
    } else {
        error!("🖼️ Image capture operation failed after {:?}. See errors above.", op_start_time.elapsed());
    }
    result
} 