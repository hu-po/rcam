use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::core::capture_source::{FrameData, FrameDataBundle};
use anyhow::{Result, anyhow};
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::Instant;
use rerun::RecordingStreamBuilder;
use rerun::datatypes::{TensorData, TensorBuffer, ColorModel};
use rerun::archetypes::Image as RerunImage;
use rerun::archetypes::EncodedImage as RerunEncodedImage;
use rerun::archetypes::DepthImage as RerunDepthImage;
use image;
use image::ImageFormat as ImageCrateFormat;
use chrono::Utc;
use futures::future::join_all;
use rerun::RecordingStream;

pub async fn handle_capture_image_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let operation_display_name = "Image Capture (Unified)";

    info!("🖼️ '{}' operation started.", operation_display_name);

    let enable_rerun = args.get_one::<bool>("rerun").copied().unwrap_or(false);
    let mut rec_stream_opt: Option<RecordingStream> = None;

    if enable_rerun {
        let flush_timeout_secs = master_config.application.rerun_flush_timeout_secs.unwrap_or(10.0);
        let mut opts = rerun::SpawnOptions::default();
        if let Some(limit) = &master_config.application.rerun_memory_limit {
            opts.memory_limit = limit.clone().into();
            debug!("Rerun: Setting memory limit to: {}", limit);
        }
        if let Some(latency_str) = &master_config.application.rerun_drop_at_latency {
            opts.extra_args.push("--drop-at-latency".into());
            opts.extra_args.push(latency_str.clone().into());
            debug!("Rerun: Setting drop-at-latency to: {}", latency_str);
        }
        match RecordingStreamBuilder::new("rcam_image_capture_unified")
            .spawn_opts(&opts, Some(std::time::Duration::from_secs_f32(flush_timeout_secs)))
        {
            Ok(stream) => {
                info!("Rerun recording stream initialized for '{}' (FlushTimeout: {}s).", operation_display_name, flush_timeout_secs);
                rec_stream_opt = Some(stream);
            }
            Err(e) => {
                error!("Failed to initialize Rerun recording stream: {}. Continuing without Rerun.", e);
            }
        }
    }

    if args.contains_id("delay") {
        warn!("⚠️ The --delay argument is ignored for image capture as it is now operationally synchronized.");
    }
    debug!(
        "Capture image CLI: devices_arg: {:?}, output_arg: {:?}",
        args.get_one::<String>("cameras"), args.get_one::<String>("output")
    );
    
    let target_devices = op_helper::determine_target_devices(
        camera_manager,
        args.get_one::<String>("cameras"),
        operation_display_name
    ).await?;

    if target_devices.is_empty() {
        info!("No devices selected or available for image capture. Exiting.");
        return Ok(());
    }
    info!("🖼️ Preparing to capture images from {} specified device(s).", target_devices.len());

    let output_dir = op_helper::determine_operation_output_dir(
        master_config,
        args,
        "output",
        Some("images_unified"),
        operation_display_name
    )?;
    
    let ts_str = Utc::now().format(&master_config.application.filename_timestamp_format).to_string();
    let mut capture_handles = Vec::new();

    for device_arc in target_devices {
        let output_dir_clone = output_dir.clone();
        let ts_str_clone = ts_str.clone();
        let image_format_for_device = master_config.application.image_format.clone();
        let jpeg_quality_clone = master_config.application.jpeg_quality;
        let png_compression_clone = master_config.application.png_compression;

        capture_handles.push(tokio::spawn(async move {
            let mut device_locked = device_arc.lock().await;
            let device_name = device_locked.get_name();
            let device_type = device_locked.get_type();
            info!("Initiating capture for device: '{}' (Type: {})", device_name, device_type);
            
            device_locked.capture_image(
                &output_dir_clone,
                &ts_str_clone,
                &image_format_for_device,
                jpeg_quality_clone,
                png_compression_clone,
            ).await
             .map_err(|e| {
                error!("Capture failed for device '{}': {}", device_name, e);
                e
            })
        }));
    }

    let capture_results_outer = join_all(capture_handles).await;
    
    let mut successful_frame_data_bundles: Vec<FrameDataBundle> = Vec::new();
    let mut capture_errors_count = 0;

    for (idx, join_handle_result) in capture_results_outer.into_iter().enumerate() {
        match join_handle_result {
            Ok(capture_result_inner) => {
                match capture_result_inner {
                    Ok(frame_data_bundle) => {
                        info!("Successfully captured data for device (index {}) -> {} frame(s) in bundle.", idx, frame_data_bundle.frames.len());
                        successful_frame_data_bundles.push(frame_data_bundle);
                    }
                    Err(e) => {
                        error!("Error during capture for device (index {}): {:?}", idx, e);
                        capture_errors_count += 1;
                    }
                }
            }
            Err(e) => {
                error!("JoinError for capture task (device index {}): {:?}", idx, e);
                capture_errors_count += 1;
            }
        }
    }
    
    if capture_errors_count > 0 {
        warn!("Encountered {} error(s) during image capture from devices.", capture_errors_count);
    }
    if successful_frame_data_bundles.is_empty() && capture_errors_count > 0 {
        error!("All image capture attempts failed. Nothing to log to Rerun.");
        return Err(anyhow!("All image capture attempts failed."));
    }
    if successful_frame_data_bundles.is_empty() {
        info!("No images were successfully captured from any device. Nothing to log to Rerun.");
    }

    if let Some(rec_stream) = &rec_stream_opt {
        if successful_frame_data_bundles.is_empty() {
            info!("Rerun: No successful image data bundles to log.");
        } else {
            info!("Rerun: Processing {} successful frame data bundle(s) for logging...", successful_frame_data_bundles.len());
        }

        rec_stream.set_duration_secs("capture_op_time", op_start_time.elapsed().as_secs_f64());

        for frame_bundle in successful_frame_data_bundles {
            for frame_data_item in frame_bundle.frames {
                match frame_data_item {
                    FrameData::IpCameraImage { name, path, format } => {
                        let entity_path_str = format!("device/{}/image", name);
                        debug!("Rerun: Attempting to log IP camera image {} to entity path: {}", path.display(), entity_path_str);
                        
                        match std::fs::read(&path) {
                            Ok(image_bytes) => {
                                let _image_format_hint = match format.to_lowercase().as_str() {
                                    "jpg" | "jpeg" => Some(ImageCrateFormat::Jpeg),
                                    "png" => Some(ImageCrateFormat::Png),
                                     _ => {
                                        warn!("Rerun: IP Cam image format string '{}' not recognized for explicit loading. Will attempt Rerun auto-detection.", format);
                                        None
                                    }
                                };

                                let encoded_image_archetype = RerunEncodedImage::from_file_contents(image_bytes);
                                // The from_file_contents does not return a Result according to the compiler error E0061 & its signature hint.
                                // It also doesn't take a format hint.

                                if let Err(e) = rec_stream.log(&*entity_path_str, &encoded_image_archetype) {
                                    error!("Rerun: Failed to log IP camera image to Rerun for {}: {}", name, e);
                                } else {
                                    info!("Rerun: Logged IP camera image for '{}' from {}", name, path.display());
                                }
                            }
                            Err(e) => {
                                error!("Rerun: Failed to read IP camera image file {} for logging: {}", path.display(), e);
                            }
                        }
                    }
                    FrameData::RealsenseFrames { name, color_frame, depth_frame } => {
                        if let Some(color_info) = color_frame {
                            let entity_path_str = format!("device/{}/rgb_image", name);
                            debug!("Rerun: Logging Realsense color image for '{}' to entity path: {}", name, entity_path_str);
                            let tensor_data = TensorData::new(
                                vec![color_info.height as u64, color_info.width as u64, 3],
                                TensorBuffer::U8(color_info.rgb_data.into()),
                            );
                            match RerunImage::from_color_model_and_tensor(ColorModel::RGB, tensor_data) {
                                Ok(rerun_image) => {
                                    if let Err(e) = rec_stream.log(&*entity_path_str, &rerun_image) {
                                        error!("Rerun: Failed to log Realsense color image for {}: {}", name, e);
                                    } else {
                                        info!("Rerun: Logged Realsense color image for '{}'", name);
                                    }
                                }
                                Err(e) => error!("Rerun: Failed to create Realsense color RerunImage for {}: {:?}", name, e),
                            }
                        }

                        if let Some(depth_info) = depth_frame {
                            let entity_path_str = format!("device/{}/depth_image", name);
                            debug!("Rerun: Logging Realsense depth image for '{}' to entity path: {}", name, entity_path_str);
                            let tensor_data = TensorData::new(
                                vec![
                                    depth_info.height.into(),
                                    depth_info.width.into(),
                                ],
                                TensorBuffer::U16(depth_info.depth_data.into()),
                            );

                            match RerunDepthImage::try_from(tensor_data) {
                                Ok(depth_archetype) => {
                                    let depth_archetype_with_meter = depth_archetype.with_meter(depth_info.depth_units);
                                    
                                    if let Err(e) = rec_stream.log(&*entity_path_str, &depth_archetype_with_meter) {
                                        error!("Rerun: Failed to log Realsense depth image for {}: {}", name, e);
                                    } else {
                                        info!("Rerun: Logged Realsense depth image for '{}' (units: {}m)", name, depth_info.depth_units);
                                    }
                                }
                                Err(e) => {
                                    error!("Rerun: Failed to create DepthImage from tensor for {}: {:?}", name, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        info!("Rerun: Attempting to flush all logged data...");
        rec_stream.flush_blocking();
        info!("Rerun: Flush completed.");
    }

    info!("🖼️ All image capture operations completed in {:?}.", op_start_time.elapsed());
    if capture_errors_count > 0 {
         warn!("Finished with {} capture error(s). Please check logs.", capture_errors_count);
    }
    Ok(())
} 