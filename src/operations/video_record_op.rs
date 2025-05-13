use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::Result;
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::{Duration, Instant};
use rerun::RecordingStreamBuilder;
use rerun::datatypes::{TensorData, TensorBuffer, ColorModel};
use rerun::archetypes::Image as RerunImage;
use opencv::prelude::*;
use opencv::{videoio, imgproc, core as opencv_core};

pub async fn handle_record_video_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let operation_display_name = "Video Recording";

    let enable_rerun = args.get_one::<bool>("rerun").copied().unwrap_or(false);
    let mut rec_stream_opt: Option<rerun::RecordingStream> = None;

    if enable_rerun {
        let flush_timeout_secs = master_config.app_settings.rerun_flush_timeout_secs.unwrap_or(10.0);

        let mut opts = rerun::SpawnOptions::default();

        let mut applied_memory_limit = "default".to_string();
        if let Some(limit) = &master_config.app_settings.rerun_memory_limit {
            opts.memory_limit = limit.clone().into();
            applied_memory_limit = limit.clone();
            debug!("Rerun: Setting memory limit to: {}", limit);
        } else {
            debug!("Rerun: Using default memory limit.");
        }

        let mut applied_latency_config = "not set".to_string();
        if let Some(latency_str) = &master_config.app_settings.rerun_drop_at_latency {
            opts.extra_args.push("--drop-at-latency".into());
            opts.extra_args.push(latency_str.clone().into());
            applied_latency_config = latency_str.clone();
            debug!("Rerun: Setting drop-at-latency to: {}", latency_str);
        } else {
            debug!("Rerun: drop-at-latency not configured.");
        }

        match RecordingStreamBuilder::new("rcam_video_record")
            .spawn_opts(&opts, Some(std::time::Duration::from_secs_f32(flush_timeout_secs)))
        {
            Ok(stream) => {
                info!(
                    "Rerun recording stream initialized and viewer spawned (FlushTimeout: {}s, MemoryLimit: {}, DropAtLatency: {}).",
                    flush_timeout_secs,
                    applied_memory_limit,
                    applied_latency_config
                );
                rec_stream_opt = Some(stream);
            }
            Err(e) => {
                error!("Failed to initialize Rerun recording stream: {}. Continuing without Rerun.", e);
            }
        }
    }

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

    let _camera_name_to_index: std::collections::HashMap<String, usize> = cameras_info
        .iter()
        .enumerate()
        .map(|(idx, (name, _))| (name.clone(), idx))
        .collect();

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
                for path in &paths {
                    info!("  -> {}", path.display());
                }

                if let Some(rec_stream) = &rec_stream_opt {
                    if paths.is_empty() {
                        info!("Rerun: No videos were recorded, nothing to log to Rerun.");
                    } else {
                        info!("Rerun: Logging {} recorded video file(s) frame by frame...", paths.len());
                    }

                    for (idx, video_path) in paths.iter().enumerate() {
                        let camera_name_opt = cameras_info.get(idx).map(|(name, _url)| name.as_str());
                        
                        let entity_path_str = if let Some(name) = camera_name_opt {
                            format!("recorded_videos/{}/frame", name)
                        } else {
                            format!("capture/video_stream_{}", idx)
                        };

                        debug!("Rerun: Processing video {} for entity path: {}", video_path.display(), entity_path_str);

                        match videoio::VideoCapture::from_file(&video_path.to_string_lossy(), videoio::CAP_ANY) {
                            Ok(mut cap) => {
                                if !videoio::VideoCapture::is_opened(&cap).unwrap_or(false) {
                                    error!("Rerun: Failed to open video file {} for Rerun logging.", video_path.display());
                                    continue;
                                }

                                let mut frame_idx = 0i64;
                                let mut bgr_frame = opencv_core::Mat::default();
                                
                                while match cap.read(&mut bgr_frame) {
                                    Ok(true) => true,
                                    Ok(false) => {
                                        debug!("Rerun: End of video stream {} or cannot read frame.", video_path.display());
                                        false
                                    }
                                    Err(e) => {
                                        error!("Rerun: Error reading frame from {}: {}", video_path.display(), e);
                                        false
                                    }
                                } {
                                    if bgr_frame.empty() {
                                        warn!("Rerun: Read empty frame from {}. Skipping.", video_path.display());
                                        continue;
                                    }

                                    rec_stream.set_time_sequence("frame_number", frame_idx);
                                    rec_stream.set_duration_secs("video_time", op_start_time.elapsed().as_secs_f64());

                                    let mut rgb_frame = opencv_core::Mat::default();
                                    if let Err(e) = imgproc::cvt_color(&bgr_frame, &mut rgb_frame, imgproc::COLOR_BGR2RGB, 0) {
                                        error!("Rerun: Failed to convert frame to RGB for {}: {}. Skipping frame.", video_path.display(), e);
                                        frame_idx += 1;
                                        continue;
                                    }

                                    match rgb_frame.data_bytes() {
                                        Ok(data) => {
                                            let rows = rgb_frame.rows() as u64;
                                            let cols = rgb_frame.cols() as u64;
                                            let channels = rgb_frame.channels() as u64;

                                            let dimension_sizes = vec![rows, cols, channels];

                                            let tensor_data = TensorData::new(
                                                dimension_sizes,
                                                TensorBuffer::U8(data.to_vec().into())
                                            );
                                            
                                            match RerunImage::from_color_model_and_tensor(ColorModel::RGB, tensor_data.clone()) {
                                                Ok(rerun_image_archetype) => {
                                                    if let Err(e) = rec_stream.log(&*entity_path_str, &rerun_image_archetype) {
                                                        error!(
                                                            "Rerun: Failed to log frame {} from {} to Rerun: {}",
                                                            frame_idx, video_path.display(), e
                                                        );
                                                    } else {
                                                        if frame_idx % 100 == 0 {
                                                            debug!("Rerun: Logged frame {} for {} to {}", frame_idx, video_path.display(), entity_path_str);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error!(
                                                        "Rerun: Failed to create Rerun image for frame {} from {}: {:?}",
                                                        frame_idx, video_path.display(), e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Rerun: Failed to get data_bytes for frame {} from {}: {}. Skipping frame.",
                                                frame_idx, video_path.display(), e
                                            );
                                        }
                                    }
                                    frame_idx += 1;
                                }
                                info!("Rerun: Finished processing video {} ({} frames) for entity path: {}", video_path.display(), frame_idx, entity_path_str);
                            }
                            Err(e) => {
                                error!("Rerun: Failed to create VideoCapture for {}: {}", video_path.display(), e);
                            }
                        }
                    }
                    // After the loop, explicitly flush the Rerun stream.
                    info!("Rerun: Attempting to flush all logged data...");
                    rec_stream.flush_blocking();
                    info!("Rerun: Flush completed.");
                }
            }
            info!("📹 All video recording operations completed in {:?}.", op_start_time.elapsed());
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