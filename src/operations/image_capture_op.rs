use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use anyhow::{Result, anyhow};
use crate::operations::op_helper;
use clap::ArgMatches;
use log::{info, error, debug, warn};
use std::time::Instant;
use rerun::RecordingStreamBuilder;
use rerun::datatypes::{TensorData, TensorBuffer, ColorModel};
use rerun::archetypes::Image as RerunImage;
use image;
use image::ImageFormat;
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use std::sync::{Arc, Barrier};
use chrono::Utc;
use diqwest::WithDigestAuth;

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

        let mut opts = rerun::SpawnOptions::default();

        if let Some(limit) = &master_config.app_settings.rerun_memory_limit {
            opts.memory_limit = limit.clone().into();
            debug!("Rerun: Setting memory limit to: {}", limit);
        } else {
            debug!("Rerun: Using default memory limit.");
        }

        if let Some(latency_str) = &master_config.app_settings.rerun_drop_at_latency {
            opts.extra_args.push("--drop-at-latency".into());
            opts.extra_args.push(latency_str.clone().into());
            debug!("Rerun: Setting drop-at-latency to: {}", latency_str);
        } else {
            debug!("Rerun: drop-at-latency not configured.");
        }

        match RecordingStreamBuilder::new("rcam_image_capture")
            .spawn_opts(&opts, Some(std::time::Duration::from_secs_f32(flush_timeout_secs)))
        {
            Ok(stream) => {
                info!("Rerun recording stream initialized for image capture (FlushTimeout: {}s).", flush_timeout_secs);
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
    
    let camera_entities = op_helper::determine_target_cameras(
        camera_manager,
        args.get_one::<String>("cameras"),
        operation_display_name
    ).await?;

    if camera_entities.is_empty() {
        info!("No cameras selected or available for image capture. Exiting.");
        return Ok(());
    }

    let output_dir = op_helper::determine_operation_output_dir(
        master_config,
        args,
        "output",
        Some("images"),
        operation_display_name
    )?;
    
    info!("🖼️ Preparing to capture images via HTTP CGI snapshot.");

    // Build a list of (name, ip, username, password)
    let mut targets = Vec::new();
    for cam_arc in &camera_entities {
        let cam = cam_arc.lock().await;
        let ip   = cam.config.ip.clone();
        let name = cam.config.name.clone();
        let user = cam.config.username.clone();
        let pass = cam.get_password()
            .ok_or_else(|| anyhow!("Missing password for camera {}", name))?
            .to_string();
        targets.push((name, ip, user, pass));
    }

    if targets.is_empty() {
        error!("No cameras have credentials; aborting snapshot.");
        return Err(anyhow!("No cameras available"));
    }

    // Prepare HTTP client + barrier
    let client  = Client::new();
    let barrier = Arc::new(Barrier::new(targets.len()));
    // single timestamp for all files
    let ts_str = Utc::now().format(&master_config.app_settings.filename_timestamp_format).to_string();
    // Get image format string for Rerun logging
    let rerun_image_fmt_str = master_config.app_settings.image_format.clone();

    // Spawn one task per camera
    let mut handles = Vec::with_capacity(targets.len());
    for (name, ip, user, pass) in targets {
        let cli     = client.clone();
        let bar     = barrier.clone();
        let out_dir = output_dir.clone(); // from earlier determine_operation_output_dir
        let img_fmt = master_config.app_settings.image_format.clone();
        let this_name = name.clone();
        let ts_str_clone = ts_str.clone(); // Clone ts_str for each task

        handles.push(tokio::spawn(async move {
            // wait for everyone
            bar.wait();

            // hit snapshot endpoint
            let url = format!("http://{}/cgi-bin/snapshot.cgi?channel=1", ip);
            
            // Use send_with_digest_auth from diqwest
            let resp_result = cli.get(&url)
                .send_with_digest_auth(&user, &pass) // Changed to use Digest Auth
                .await;
            
            let image_content_bytes = match resp_result {
                Ok(response) => {
                    if !response.status().is_success() {
                        error!("HTTP request for {} failed with status: {}", this_name, response.status());
                        return Err(anyhow!("HTTP request failed for {} with status: {}", this_name, response.status()));
                    }
                    match response.bytes().await {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            error!("Failed to get bytes from HTTP response for {}: {}", this_name, e);
                            return Err(anyhow!("Failed to get bytes from {}: {}", this_name, e));
                        }
                    }
                },
                Err(e) => {
                    error!("HTTP request send failed for {}: {}", this_name, e);
                    return Err(anyhow!("HTTP send failed for {}: {}", this_name, e));
                }
            };

            debug!("Received {} bytes from HTTP for camera {}", image_content_bytes.len(), this_name);

            // write file
            let filename = format!("{}_{}.{}", this_name, ts_str_clone, img_fmt);
            let path = out_dir.join(&filename);
            match tokio::fs::File::create(&path).await {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(&image_content_bytes).await {
                        error!("Failed to write image for {}: {}", this_name, e);
                        return Err(anyhow!("Failed to write image for {}: {}", this_name, e));
                    }
                }
                Err(e) => {
                    error!("Failed to create file for {}: {}", this_name, e);
                    return Err(anyhow!("Failed to create file for {}: {}", this_name, e));
                }
            }
            info!("✅ Saved snapshot for '{}' ({} bytes) to {}", this_name, image_content_bytes.len(), path.display());
            Ok::<_, anyhow::Error>(path)
        }));
    }

    // wait for all to finish
    let results = futures::future::try_join_all(handles).await?;
    if let Some(rec_stream) = &rec_stream_opt {
        if results.is_empty() {
            info!("Rerun: No images were captured, nothing to log to Rerun.");
        } else {
            info!("Rerun: Logging {} captured image(s)...", results.len());
        }

        let image_format_hint = match rerun_image_fmt_str.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            "gif" => Some(ImageFormat::Gif),
            "bmp" => Some(ImageFormat::Bmp),
            "ico" => Some(ImageFormat::Ico),
            "tiff" => Some(ImageFormat::Tiff),
            "webp" => Some(ImageFormat::WebP),
            "pnm" => Some(ImageFormat::Pnm),
            "tga" => Some(ImageFormat::Tga),
            "dds" => Some(ImageFormat::Dds),
            "hdr" => Some(ImageFormat::Hdr),
            "farbfeld" => Some(ImageFormat::Farbfeld),
            "avif" => Some(ImageFormat::Avif),
            "qoi" => Some(ImageFormat::Qoi),
            _ => {
                warn!(
                    "Rerun: Image format string '{}' from config not recognized for explicit loading. Will attempt auto-detection.",
                    rerun_image_fmt_str
                );
                None
            }
        };

        for (idx, path_result) in results.iter().enumerate() {
            match path_result {
                Ok(path) => {
                    let camera_name_opt = camera_entities.get(idx).map(|_cam_arc| {
                        // This requires an async block or a different way to access camera name if needed for Rerun
                        // For now, let's use a placeholder or index if direct access is complex
                        // Or, we can retrieve names from `targets` before spawning tasks, if `targets` is accessible here
                        // For simplicity, using index as a fallback like in the original code
                        // let cam_entity = cam_arc.lock().await; // This would require this block to be async or use block_on
                        // cam_entity.config.name.as_str()
                        format!("camera_{}", idx) // Placeholder
                    });
                    
                    let entity_path_str = if let Some(name) = camera_name_opt { // This name is now just "camera_{idx}"
                        format!("camera/{}/image", name)
                    } else {
                        format!("capture/image_{}", idx)
                    };

                    debug!("Rerun: Attempting to log image {} to entity path: {}", path.display(), entity_path_str);

                    let image_bytes_result = std::fs::read(path);
                    if let Err(e) = image_bytes_result {
                        error!("Rerun: Failed to read image file at {}: {}. Skipping Rerun log for this image.", path.display(), e);
                        continue;
                    }
                    let image_bytes = image_bytes_result.unwrap();
                    debug!("Rerun: Read {} bytes from file {} for logging.", image_bytes.len(), path.display());

                    let dynamic_image_result = if let Some(fmt) = image_format_hint {
                        debug!("Rerun: Attempting to load image {} with explicit format: {:?}", path.display(), fmt);
                        image::load_from_memory_with_format(&image_bytes, fmt)
                    } else {
                        debug!("Rerun: Attempting to load image {} with auto-detection.", path.display());
                        image::load_from_memory(&image_bytes)
                    };

                    match dynamic_image_result {
                        Ok(dynamic_image) => {
                            let img_rgb8 = dynamic_image.to_rgb8();
                            let log_cam_name = format!("camera_{}",idx); // Placeholder
                            
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
                            error!(
                                "Rerun: Failed to decode image at {} (format hint: {:?}, attempted method: {}): {}. Skipping Rerun log for this image.",
                                path.display(),
                                image_format_hint, // Debug output for Option<ImageFormat>
                                if image_format_hint.is_some() { "explicit format" } else { "auto-detection" },
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                     error!("An error occurred capturing image for one of the cameras: {}", e);
                }
            }
        }
        info!("Rerun: Attempting to flush all logged data...");
        rec_stream.flush_blocking();
        info!("Rerun: Flush completed.");
    }

    info!("🖼️ All snapshots completed in {:?}.", op_start_time.elapsed());
    Ok(())
} 