use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_media::CameraMediaManager;
use anyhow::{Result, anyhow};
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::Instant;
use rerun::RecordingStreamBuilder;
use rerun::datatypes::{TensorData, TensorBuffer, ColorModel};
use rerun::archetypes::Image as RerunImage;
use image;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let operation_display_name = "Image Capture";

    let enable_rerun = args.get_one::<bool>("rerun").copied().unwrap_or(false);
    let mut rec_stream_opt: Option<rerun::RecordingStream> = None;

    if enable_rerun {
        let flush_timeout_secs = master_config.app_settings.rerun_flush_timeout_secs.unwrap_or(10.0);

        match RecordingStreamBuilder::new("rcam_image_capture")
            .spawn_opts(&rerun::SpawnOptions::default(), Some(std::time::Duration::from_secs_f32(flush_timeout_secs)))
        {
            Ok(stream) => {
                info!("Rerun recording stream initialized and viewer spawned (FlushTimeout: {}s).", flush_timeout_secs);
                rec_stream_opt = Some(stream);
            }
            Err(e) => {
                error!("Failed to initialize Rerun recording stream: {}. Continuing without Rerun.", e);
            }
        }
    }

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

    let _camera_name_to_index: std::collections::HashMap<String, usize> = cameras_info
        .iter()
        .enumerate()
        .map(|(idx, (name, _))| (name.clone(), idx))
        .collect();
    
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
                for path in &paths {
                    info!("  -> {}", path.display());
                }

                if let Some(rec_stream) = &rec_stream_opt {
                    if paths.is_empty() {
                        info!("Rerun: No images were captured, nothing to log to Rerun.");
                    } else {
                        info!("Rerun: Logging {} captured image(s)...", paths.len());
                    }

                    for (idx, path) in paths.iter().enumerate() {
                        let camera_name_opt = cameras_info.get(idx).map(|(name, _url)| name.as_str());
                        
                        let entity_path_str = if let Some(name) = camera_name_opt {
                            format!("camera/{}/image", name)
                        } else {
                            format!("capture/image_{}", idx)
                        };

                        debug!("Rerun: Attempting to log image {} to entity path: {}", path.display(), entity_path_str);

                        match image::load_from_memory(&std::fs::read(path)?) {
                            Ok(dynamic_image) => {
                                let img_rgb8 = dynamic_image.to_rgb8();
                                
                                let log_cam_name = camera_name_opt.unwrap_or("unknown_camera");
                                
                                rec_stream.set_duration_secs("capture_time", op_start_time.elapsed().as_secs_f64());

                                let (width, height) = img_rgb8.dimensions();
                                
                                let dimension_sizes = vec![height as u64, width as u64, 3_u64];
                                
                                let tensor_data = TensorData::new(
                                    dimension_sizes, 
                                    TensorBuffer::U8(img_rgb8.into_raw().into())
                                );

                                match RerunImage::from_color_model_and_tensor(ColorModel::RGB, tensor_data.clone()) {
                                    Ok(rerun_image_archetype) => {
                                        if let Err(e) = rec_stream.log(&*entity_path_str, &rerun_image_archetype) {
                                            error!("Failed to log image to Rerun for {}: {}", log_cam_name, e);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to create Rerun image for {} using from_color_model_and_tensor: {:?}", log_cam_name, e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Rerun: Failed to open or decode image at {}: {}. Skipping Rerun log for this image.", path.display(), e);
                            }
                        }
                    }
                    // After the loop, explicitly flush the Rerun stream.
                    info!("Rerun: Attempting to flush all logged data...");
                    rec_stream.flush_blocking();
                    info!("Rerun: Flush completed.");
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