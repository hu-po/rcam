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

pub async fn handle_record_video_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
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

    let result = run_generic_camera_op(
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
                let cam_op_start_time = Instant::now();
                let mut cam_entity = cam_entity_arc.lock().await;
                let cam_name = cam_entity.config.name.clone();
                
                let filename = file_utils::generate_timestamped_filename(
                    &cam_entity.config.name,
                    &app_settings_arc.filename_timestamp_format,
                    &app_settings_arc.video_format,
                );
                let output_path = operation_output_dir.join(filename);
                
                info!(
                    "🎬 Preparing to record video for '{}' to {} for {:?}",
                    cam_name,
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
                            "✅ Successfully recorded video for '{}' to {} in {:?}",
                            cam_name,
                            path.display(),
                            cam_op_start_time.elapsed()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to record video for '{}' after {:?}: {:#}",
                            cam_name,
                            cam_op_start_time.elapsed(),
                            e
                        );
                        Err(e)
                    }
                }
            }
        },
    )
    .await;

    if result.is_ok() {
        info!("📹 All video recording operations completed successfully in {:?}", op_start_time.elapsed());
    } else {
        error!("📹 Video recording operation failed after {:?}. See errors above.", op_start_time.elapsed());
    }
    result
} 