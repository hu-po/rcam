use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::Result;
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::Duration;
use std::time::Instant;

pub async fn handle_record_video_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let operation_display_name = "Video Recording";

    let duration_seconds_arg = args.get_one::<u64>("duration").copied();
    let duration_seconds = duration_seconds_arg.unwrap_or(master_config.app_settings.video_duration_default_seconds as u64);
    let recording_duration = Duration::from_secs(duration_seconds);
    debug!(
        "Record video CLI: duration_arg: {:?}, effective_duration: {:?}, cameras_arg: {:?}, output_arg: {:?}",
        duration_seconds_arg, recording_duration, args.get_one::<String>("cameras"), args.get_one::<String>("output")
    );
    info!("📹 Preparing to record video for {:?} from specified cameras.", recording_duration);

    let media_manager_init_start = Instant::now();
    let media_manager = CameraMediaManager::new();
    debug!("CameraMediaManager initialized for video recording in {:?}.", media_manager_init_start.elapsed());

    let camera_entities = op_helper::determine_target_cameras(
        camera_manager, 
        args.get_one::<String>("cameras"),
        operation_display_name
    ).await?;

    if camera_entities.is_empty() {
        info!("No cameras selected or available for video recording. Exiting.");
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
        return Err(anyhow::anyhow!("Failed to retrieve any usable RTSP URLs for video recording"));
    }

    let default_subdir_name = master_config.app_settings.video_format.clone();
    let output_dir = op_helper::determine_operation_output_dir(
        master_config,
        args,
        "output",
        Some(&default_subdir_name), 
        operation_display_name
    )?;

    info!(
        "🎬 Attempting video recording for {} camera(s) to {} for {:?}.",
        cameras_info.len(),
        output_dir.display(),
        recording_duration
    );

    match media_manager
        .record_video(
            &cameras_info,
            &master_config.app_settings,
            output_dir.clone(), 
            recording_duration,
        )
        .await
    {
        Ok(paths) => {
            if paths.is_empty() && !cameras_info.is_empty() {
                warn!(
                    "📹 Video recording completed but no files were produced. This might indicate an issue during recording for all cameras."
                );
            } else if paths.is_empty() && cameras_info.is_empty() {
                 info!("📹 Video recording: No cameras were processed (likely due to RTSP URL issues).");
            } else {
                info!(
                    "✅ Successfully recorded {} video file(s) in {:?}:",
                    paths.len(),
                    op_start_time.elapsed()
                );
                for path in paths {
                    info!("  -> {}", path.display());
                }
            }
            Ok(())
        }
        Err(e) => {
            error!(
                "❌ Failed video recording for {} camera(s) after {:?}: {:#}",
                cameras_info.len(),
                op_start_time.elapsed(),
                e
            );
            Err(e)
        }
    }
} 