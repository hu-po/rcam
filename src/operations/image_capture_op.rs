use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::{Result, anyhow};
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::Instant;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let operation_display_name = "Image Capture";

    if args.contains_id("delay") {
        warn!("⚠️ The --delay argument is ignored for image capture as it is now always synchronized.");
    }
    debug!(
        "Capture image CLI: cameras_arg: {:?}, output_arg: {:?}",
        args.get_one::<String>("cameras"), args.get_one::<String>("output")
    );
    info!("🖼️ Preparing to capture images from specified cameras.");
    
    let media_manager_init_start = Instant::now();
    let media_manager = CameraMediaManager::new();
    debug!("CameraMediaManager initialized for image capture in {:?}.", media_manager_init_start.elapsed());

    let camera_entities = op_helper::determine_target_cameras(
        camera_manager,
        args.get_one::<String>("cameras"),
        operation_display_name
    ).await?;

    if camera_entities.is_empty() {
        info!("No cameras selected or available for image capture. Exiting.");
        return Ok(());
    }

    let mut cameras_info = Vec::new();
    for cam_entity_arc in &camera_entities {
        let cam_entity = cam_entity_arc.lock().await;
        let name = cam_entity.config.name.clone();
        match cam_entity.get_rtsp_url() {
            Ok(url) => cameras_info.push((name, url)),
            Err(e) => {
                error!("Failed to get RTSP URL for camera '{}' for {}: {}. This camera will be excluded.", name, operation_display_name, e);
            }
        }
    }
    
    if cameras_info.is_empty() {
        error!("Could not retrieve RTSP URLs for any of the {} selected/available cameras. Cannot proceed with {}.", camera_entities.len(), operation_display_name);
        return Err(anyhow!("Failed to retrieve any usable RTSP URLs for image capture"));
    }

    let output_dir = op_helper::determine_operation_output_dir(
        master_config,
        args,
        "output",
        Some("images"),
        operation_display_name
    )?;

    info!(
        "📸 Attempting image capture for {} camera(s) to {}.",
        cameras_info.len(),
        output_dir.display()
    );

    match media_manager
        .capture_image(
            &cameras_info,
            &master_config.app_settings,
            output_dir.clone(),
        )
        .await
    {
        Ok(paths) => {
            if paths.is_empty() && !cameras_info.is_empty() {
                warn!(
                    "🖼️ Image capture completed but no files were produced. This might indicate an issue during capture for all cameras."
                );
            } else if paths.is_empty() && cameras_info.is_empty() {
                 info!("🖼️ Image capture: No cameras were processed (likely due to RTSP URL issues).");
            } else {
                info!(
                    "✅ Successfully captured {} image file(s) in {:?}:",
                    paths.len(),
                    op_start_time.elapsed()
                );
                for path in paths {
                    info!("  -> {}", path.display());
                }
            }
            info!("🖼️ All image capture operations completed in {:?}.", op_start_time.elapsed());
            Ok(())
        }
        Err(e) => {
            error!(
                "❌ Failed image capture for {} camera(s) after {:?}: {:#}",
                cameras_info.len(),
                op_start_time.elapsed(),
                e
            );
            Err(e)
        }
    }
} 