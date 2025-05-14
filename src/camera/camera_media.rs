// use crate::app_config::ApplicationConfig; // This import is unused
use crate::config_loader::AppSettings;
use anyhow::{Context, Result, anyhow};
use log::{info, warn, error, debug};
use std::path::PathBuf;
use std::time::Duration;
use opencv::{
    prelude::*,
    videoio,
    imgcodecs,
    core as opencv_core
};
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::sync::Arc;
use chrono::Utc;
use futures::future::join_all;
use chrono::DateTime;
use std::sync::Barrier;
use std::time::Instant;


#[derive(Clone)]
pub struct CameraMediaManager {
    captures: Arc<Mutex<HashMap<String, Arc<Mutex<videoio::VideoCapture>>>>>,
}

impl CameraMediaManager {
    pub fn new() -> Self {
        debug!("🖼️📹 Initializing CameraMediaManager...");
        let start_time = std::time::Instant::now();
        let manager = CameraMediaManager {
            captures: Arc::new(Mutex::new(HashMap::new())),
        };
        debug!("✅ CameraMediaManager initialized in {:?}", start_time.elapsed());
        manager
    }

    async fn get_or_init_capture(&self, camera_name: &str, rtsp_url: &str) -> Result<Arc<Mutex<videoio::VideoCapture>>> {
        let mut captures_map = self.captures.lock().await;
        if let Some(cap_mutex) = captures_map.get(camera_name) {
            debug!("Found existing VideoCapture for '{}'", camera_name);
            return Ok(cap_mutex.clone());
        }

        debug!("Creating new VideoCapture for '{}' with URL: {}", camera_name, rtsp_url);
        let cap_create_start = std::time::Instant::now();
        
        let rtsp_url_clone = rtsp_url.to_string();
        let cap = tokio::task::spawn_blocking(move || {
            videoio::VideoCapture::from_file(&rtsp_url_clone, videoio::CAP_ANY)
        }).await??;
        
        debug!("  VideoCapture created for '{}' in {:?}", camera_name, cap_create_start.elapsed());

        let opened_check_start = std::time::Instant::now();
        let camera_name_for_open_check = camera_name.to_string();
        let rtsp_url_for_open_check = rtsp_url.to_string();
        
        let is_cap_opened = {
            let opened = videoio::VideoCapture::is_opened(&cap)
                 .map_err(|e| anyhow!(e).context(format!("OpenCV: Failed to check if VideoCapture is opened for '{}'", camera_name_for_open_check)))?;
            debug!("  VideoCapture::is_opened check for '{}' in {:?} (executed synchronously after cap creation)", camera_name, opened_check_start.elapsed());
            if !opened {
                error!("❌ Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", camera_name, rtsp_url_for_open_check);
                return Err(anyhow!("Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", camera_name, rtsp_url_for_open_check));
            }
            info!("👍 RTSP stream opened and initialized for '{}'", camera_name);
            Ok::<_, anyhow::Error>(())
        };
        is_cap_opened?;

        let cap_mutex = Arc::new(Mutex::new(cap));
        captures_map.insert(camera_name.to_string(), cap_mutex.clone());
        Ok(cap_mutex)
    }

    pub async fn capture_image(
        &self,
        cameras_info: &[(String, String)], // List of (camera_name, rtsp_url)
        app_config: &AppSettings,
        output_dir: PathBuf,
    ) -> Result<Vec<PathBuf>> {
        info!("📸 Attempting image capture for {} cameras.", cameras_info.len());
        let overall_start_time = std::time::Instant::now();

        if cameras_info.is_empty() {
            warn!("🖼️ No cameras provided for image capture.");
            return Ok(Vec::new());
        }

        // 1. Get or initialize all captures (Parallelized)
        let mut capture_init_futures = Vec::new();
        let mut temp_camera_names_ordered = Vec::new(); // To keep order for matching results

        for (name, url) in cameras_info {
            debug!("  Queueing capture initialization for image capture: {} ({})", name, url);
            temp_camera_names_ordered.push(name.clone());
            capture_init_futures.push(self.get_or_init_capture(name, url));
        }

        info!("  Initializing {} camera stream(s) for image capture concurrently...", capture_init_futures.len());
        let init_results = join_all(capture_init_futures).await;
        info!("  All camera stream initialization attempts for image capture completed.");

        let mut capture_arcs = Vec::new();
        let mut camera_names_ordered = Vec::new(); // For successfully initialized cameras

        for (i, result) in init_results.into_iter().enumerate() {
            let cam_name = &temp_camera_names_ordered[i];
            match result {
                Ok(cap_arc) => {
                    debug!("Successfully initialized capture for '{}' for image capture.", cam_name);
                    capture_arcs.push(cap_arc);
                    camera_names_ordered.push(cam_name.clone());
                }
                Err(e) => {
                    error!("Failed to get/init capture for camera '{}' for image capture: {:#}. Skipping this camera.", cam_name, e);
                }
            }
        }

        if capture_arcs.is_empty() {
            warn!("🖼️ No camera streams could be initialized for image capture. Aborting.");
            return Ok(Vec::new());
        }
        info!("Successfully initialized {} out of {} camera streams for image capture.", capture_arcs.len(), cameras_info.len());

        // 2. Prepare output directory
        if !output_dir.exists() {
            debug!("Creating output directory for images: {}", output_dir.display());
            std::fs::create_dir_all(&output_dir)
                .with_context(|| format!("Failed to create output directory for images: {}", output_dir.display()))?;
        }
        
        // 3. Parallel Frame Reading and Saving
        let mut read_tasks = Vec::new();
        info!("🖼️ Spawning parallel frame read/save tasks for {} cameras.", capture_arcs.len());

        let barrier = Arc::new(Barrier::new(capture_arcs.len()));

        for (idx, cap_arc_clone) in capture_arcs.iter().cloned().enumerate() {
            let cam_name = camera_names_ordered[idx].clone();
            let app_config_task_clone = app_config.clone();
            let output_dir_task_clone = output_dir.clone();
            let barrier_clone = barrier.clone();

            let task = tokio::task::spawn_blocking(move || -> Result<(PathBuf, String, DateTime<Utc>)> {
                barrier_clone.wait();
                
                let mut frame = opencv_core::Mat::default();
                
                // Lock inside task
                // Note: futures::executor::block_on is used here because spawn_blocking runs in a
                // separate thread pool that doesn't have a Tokio runtime context by default.
                // Locking an async Mutex from a synchronous context requires a bridge like block_on.
                let mut cap_guard = match futures::executor::block_on(cap_arc_clone.lock()) {
                    guard => guard, // This part seems a bit off, direct assignment is fine if lock() returns the guard
                };

                let read_start_time = std::time::Instant::now();
                if !cap_guard.read(&mut frame).map_err(|e| anyhow!(e).context(format!("OpenCV: Read failed for {}", cam_name)))? {
                    return Err(anyhow!("OpenCV: Failed to read frame for '{}'", cam_name));
                }
                let capture_utc_ts = Utc::now(); // Timestamp immediately after read
                debug!("OpenCV (blocking): Frame read for '{}' in {:?}, captured at {}", cam_name, read_start_time.elapsed(), capture_utc_ts);


                if frame.empty() {
                    return Err(anyhow!("OpenCV: Captured frame is empty for '{}'", cam_name));
                }

                // Generate filename using the precise capture_utc_ts
                let local_ts_for_filename = DateTime::<chrono::Local>::from(capture_utc_ts);
                let filename_ts_str = local_ts_for_filename.format(&app_config_task_clone.filename_timestamp_format).to_string();
                let filename = format!("{}_{}.{}", cam_name, filename_ts_str, app_config_task_clone.image_format);
                let output_path = output_dir_task_clone.join(&filename);

                // Ensure parent directory exists (it should due to earlier check, but good for safety)
                if let Some(parent_dir) = output_path.parent() {
                    if !parent_dir.exists() { // Redundant if output_dir itself was created, but harmless
                         std::fs::create_dir_all(parent_dir)
                             .with_context(|| format!("OpenCV: Failed to create parent for image '{}'", output_path.display()))?;
                    }
                }

                let mut params = opencv_core::Vector::<i32>::new();
                if app_config_task_clone.image_format.to_lowercase() == "jpg" || app_config_task_clone.image_format.to_lowercase() == "jpeg" {
                    params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                    params.push(app_config_task_clone.jpeg_quality.unwrap_or(95) as i32); // Use configured or default
                } else if app_config_task_clone.image_format.to_lowercase() == "png" {
                    params.push(imgcodecs::IMWRITE_PNG_COMPRESSION);
                    params.push(app_config_task_clone.png_compression.unwrap_or(3) as i32); // Use configured or default
                }


                let imwrite_start = std::time::Instant::now();
                imgcodecs::imwrite(output_path.to_str().context("Invalid path (not UTF-8) for imwrite")?, &frame, &params)
                    .map_err(|e| anyhow!(e).context(format!("OpenCV: Imwrite failed for {} to {}", cam_name, output_path.display())))?;
                debug!("OpenCV (blocking): Image written for '{}' in {:?}", cam_name, imwrite_start.elapsed());
                
                Ok((output_path, cam_name, capture_utc_ts))
            });
            read_tasks.push(task);
        }

        let mut saved_image_details: Vec<(PathBuf, String, DateTime<Utc>)> = Vec::new();
        let frame_save_results = join_all(read_tasks).await;

        info!("🏁 All parallel image capture/save tasks completed processing.");
        for (idx, result_outer) in frame_save_results.into_iter().enumerate() {
            let cam_name_for_log = &camera_names_ordered.get(idx).map_or_else(|| "unknown_camera".to_string(), |cn| cn.clone());
            match result_outer { // Handle JoinError from spawn_blocking
                Ok(Ok((path, name, ts))) => {
                    // Log success with consistent camera name from original order if available
                    info!("✅ Image saved for '{}' to {} (captured at {} UTC)", name, path.display(), ts.to_rfc3339());
                    saved_image_details.push((path, name, ts));
                }
                Ok(Err(e)) => { // Error from the task's Result
                    error!("❌ Error capturing/saving frame for camera '{}': {:#}", cam_name_for_log, e);
                }
                Err(e) => { // Task panicked
                    error!("❌ Image capture task for camera '{}' panicked: {:#}", cam_name_for_log, e);
                }
            }
        }
        
        let saved_image_paths: Vec<PathBuf> = saved_image_details.iter().map(|(p, _, _)| p.clone()).collect();

        if saved_image_paths.is_empty() && !cameras_info.is_empty() && !capture_arcs.is_empty() {
             warn!(
                "📸 Parallel image capture tasks completed, but no files were produced from {} successfully initialized streams. This might indicate issues during read/save for all processed cameras.",
                capture_arcs.len()
            );
        } else if saved_image_paths.is_empty() && capture_arcs.is_empty() {
            // This case should be covered by the earlier check on capture_arcs.is_empty(), but for robustness:
            info!("📸 Image capture: No camera streams were available or initialized successfully.");
        } else {
            info!(
                "✅ Successfully captured and saved {} image file(s) from {} camera streams in {:?}.",
                saved_image_paths.len(),
                capture_arcs.len(), // Log how many streams were attempted in parallel
                overall_start_time.elapsed()
            );
        }
        Ok(saved_image_paths)
    }

    pub async fn record_video(
        &self,
        cameras_info: &[(String, String)],
        app_config: &AppSettings,
        output_dir: PathBuf,
        duration: Duration,
    ) -> Result<Vec<PathBuf>> {
        info!("📹 Attempting video recording for {} cameras for {:?}", cameras_info.len(), duration);
        let overall_start_time = std::time::Instant::now();

        if cameras_info.is_empty() {
            warn!("🎬 No cameras provided for recording.");
            return Ok(Vec::new());
        }

        // 1. Get or initialize all captures (Parallelized) - Same as before
        let mut capture_init_futures = Vec::new();
        let mut temp_camera_names_ordered = Vec::new(); 

        for (name, url) in cameras_info {
            debug!("  Queueing capture initialization for recording: {} ({})", name, url);
            temp_camera_names_ordered.push(name.clone());
            capture_init_futures.push(self.get_or_init_capture(name, url));
        }

        info!("  Initializing {} camera stream(s) for video recording concurrently...", capture_init_futures.len());
        let init_results = join_all(capture_init_futures).await;
        info!("  All camera stream initialization attempts for video recording completed.");

        let mut capture_arcs = Vec::new();
        let mut camera_names_ordered = Vec::new(); 

        for (i, result) in init_results.into_iter().enumerate() {
            let cam_name = &temp_camera_names_ordered[i];
            match result {
                Ok(cap_arc) => {
                    debug!("Successfully initialized capture for '{}' for video recording.", cam_name);
                    capture_arcs.push(cap_arc);
                    camera_names_ordered.push(cam_name.clone());
                }
                Err(e) => {
                    error!("Failed to get/init capture for camera '{}' for video recording: {:#}. Skipping this camera.", cam_name, e);
                }
            }
        }

        if capture_arcs.is_empty() {
            warn!("🎬 No camera streams could be initialized for video recording. Aborting.");
            return Ok(Vec::new());
        }
        info!("Successfully initialized {} out of {} camera streams for video recording.", capture_arcs.len(), cameras_info.len());

        // 2. Prepare output directory and output paths per camera
        if !output_dir.exists() {
            debug!("Creating output directory for videos: {}", output_dir.display());
            std::fs::create_dir_all(&output_dir)
                .with_context(|| format!("Failed to create output directory for videos: {}", output_dir.display()))?;
        }

        let mut per_camera_output_paths = Vec::new();
        for name in &camera_names_ordered {
            let timestamp = Utc::now().format(&app_config.filename_timestamp_format).to_string(); // Use consistent timestamp format
            let filename = format!("{}_{}.{}", name, timestamp, app_config.video_format);
            per_camera_output_paths.push(output_dir.join(filename));
        }
        
        // 3. Spawn per-camera recording tasks, synchronized by a barrier
        let mut record_tasks = Vec::new();
        let barrier = Arc::new(Barrier::new(capture_arcs.len()));
        info!("🎬 Spawning parallel video recording tasks for {} cameras, synchronized by a barrier.", capture_arcs.len());

        for i in 0..capture_arcs.len() {
            let cap_arc_clone = capture_arcs[i].clone();
            let cam_name_clone = camera_names_ordered[i].clone();
            let output_path_clone = per_camera_output_paths[i].clone();
            let app_config_clone = app_config.clone();
            let duration_clone = duration;
            let barrier_clone = barrier.clone();

            let task = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
                barrier_clone.wait(); // Synchronize start of blocking work
                let task_start_time = std::time::Instant::now();
                info!("🎬 OpenCV (blocking): Starting recording for camera '{}' to {}", cam_name_clone, output_path_clone.display());

                // cap_arc_clone.blocking_lock() will panic if the mutex is poisoned.
                // This panic will be caught as a JoinError by the task handling logic later.
                let mut cap_guard = cap_arc_clone.blocking_lock(); // Made cap_guard mutable

                let frame_width_f64 = cap_guard.get(videoio::CAP_PROP_FRAME_WIDTH)
                    .map_err(|e| anyhow::Error::from(e).context(format!("OpenCV: Failed to get CAP_PROP_FRAME_WIDTH for '{}'", cam_name_clone)))?;
                let frame_width = frame_width_f64 as i32;

                let frame_height_f64 = cap_guard.get(videoio::CAP_PROP_FRAME_HEIGHT)
                    .map_err(|e| anyhow::Error::from(e).context(format!("OpenCV: Failed to get CAP_PROP_FRAME_HEIGHT for '{}'", cam_name_clone)))?;
                let frame_height = frame_height_f64 as i32;
                
                // Get camera reported FPS for logging, but use configured FPS for consistency in recording.
                let camera_reported_fps: f64 = cap_guard.get(videoio::CAP_PROP_FPS)
                    .map_err(|e| anyhow::Error::from(e).context(format!("OpenCV: Failed to get CAP_PROP_FPS for '{}'", cam_name_clone)))?;
                
                let common_fps = app_config_clone.video_fps.unwrap_or(30.0) as f64; // FPS to be used for recording

                if frame_width <= 0 || frame_height <= 0 {
                    let err_msg = format!("Invalid frame dimensions ({}x{}) for camera '{}'", frame_width, frame_height, cam_name_clone);
                    error!("❌ OpenCV (blocking): {}", err_msg);
                    return Err(anyhow!(err_msg));
                }

                // Log reported FPS vs used FPS
                if camera_reported_fps <= 0.0 {
                    warn!("⚠️ Camera '{}' reported FPS <= 0 ({}). Using configured FPS for recording: {}", cam_name_clone, camera_reported_fps, common_fps);
                } else {
                    debug!("  Camera '{}' reported FPS {}. Recording will use common FPS: {}", cam_name_clone, camera_reported_fps, common_fps);
                }
                
                // Validate the common_fps that will be used for the writer
                if common_fps <= 0.0 {
                     let err_msg = format!("Common FPS for recording is invalid ({}) for camera '{}'. Check app_config.video_fps.", common_fps, cam_name_clone);
                     error!("❌ {}", err_msg);
                     return Err(anyhow!(err_msg));
                }


                let fourcc_str = match app_config_clone.video_codec.to_lowercase().as_str() {
                    "mjpg" | "mjpeg" => "MJPG",
                    "xvid" => "XVID",
                    "mp4v" => "MP4V",
                    "h264" if app_config_clone.video_format.to_lowercase() == "avi" => "H264", // OpenCV's internal H264 for AVI
                    "h264" if app_config_clone.video_format.to_lowercase() == "mp4" => "avc1", // More standard for MP4
                    codec_val => {
                        warn!("⚠️ Unsupported video_codec '{}' for OpenCV VideoWriter with format '{}' for '{}'. Defaulting to MJPG.", codec_val, app_config_clone.video_format, cam_name_clone);
                        "MJPG"
                    }
                };
                let fourcc = videoio::VideoWriter::fourcc(fourcc_str.chars().nth(0).unwrap_or('M'), fourcc_str.chars().nth(1).unwrap_or('J'), fourcc_str.chars().nth(2).unwrap_or('P'), fourcc_str.chars().nth(3).unwrap_or('G'))?;

                let mut writer = videoio::VideoWriter::new(
                    output_path_clone.to_str().context("Invalid output path for video (not UTF-8)")?,
                    fourcc,
                    common_fps, // Use the potentially overridden common_fps
                    opencv_core::Size::new(frame_width, frame_height),
                    true,
                )?;

                if !videoio::VideoWriter::is_opened(&writer)? {
                    let err_msg = format!("Failed to open VideoWriter for '{}' at path '{}'", cam_name_clone, output_path_clone.display());
                    error!("❌ OpenCV (blocking): {}", err_msg);
                    // Attempt to delete the file if writer creation failed but file might have been touched
                    if output_path_clone.exists() {
                        if let Err(del_err) = std::fs::remove_file(&output_path_clone) {
                            warn!("Failed to delete empty/partial file {} after VideoWriter open error: {}", output_path_clone.display(), del_err);
                        }
                    }
                    return Err(anyhow!(err_msg));
                }
                info!("✍️ OpenCV (blocking): VideoWriter opened for '{}' to {}", cam_name_clone, output_path_clone.display());
                
                let num_frames = (duration_clone.as_secs_f64() * common_fps).round() as u64;
                info!("  OpenCV (blocking) [{}]: Starting recording loop for {} frames (duration: {:?}, fps: {}).", cam_name_clone, num_frames, duration_clone, common_fps);

                let mut last_error_log_time = std::time::Instant::now();
                let mut frame_read_error_count = 0;
                const MAX_CONSECUTIVE_READ_ERRORS: u32 = 5; // Allow a few hiccups

                for frame_idx in 0..num_frames {
                    let mut temp_frame = opencv_core::Mat::default();
                    // Grab and Retrieve in one go for simplicity per frame, per camera
                    if !cap_guard.read(&mut temp_frame).with_context(|| format!("OpenCV: Read failed for camera '{}'", cam_name_clone))? {
                         if last_error_log_time.elapsed().as_secs() > 2 || frame_read_error_count == 0 {
                           error!("🚫 OpenCV (blocking) [{}]: Failed to read frame (stream might have ended or temporarily unavailable). Frame index: {}", cam_name_clone, frame_idx);
                           last_error_log_time = std::time::Instant::now();
                        }
                        frame_read_error_count += 1;
                        if frame_read_error_count > MAX_CONSECUTIVE_READ_ERRORS {
                             let err_msg = format!("Aborting recording for '{}' due to {} consecutive frame read errors.", cam_name_clone, MAX_CONSECUTIVE_READ_ERRORS);
                             error!("❌ {}", err_msg);
                             return Err(anyhow!(err_msg));
                        }
                        // Optional: could sleep briefly before retrying grab on next iteration
                        std::thread::sleep(Duration::from_millis(100)); // Small delay before next attempt
                        continue; // Try next frame
                    }
                    frame_read_error_count = 0; // Reset error count on successful read

                    if temp_frame.empty() {
                        if last_error_log_time.elapsed().as_secs() > 2 {
                            warn!("👻 OpenCV (blocking) [{}]: Retrieved empty frame at frame index {}. Skipping write.", cam_name_clone, frame_idx);
                            last_error_log_time = std::time::Instant::now();
                        }
                        continue; 
                    }
                    writer.write(&temp_frame).with_context(|| format!("OpenCV: Write failed for '{}' to '{}'", cam_name_clone, output_path_clone.display()))?;
                    
                    if frame_idx > 0 && frame_idx % (common_fps.round() as u64 * 5) == 0 { // Log every 5 seconds approx
                        debug!("  OpenCV (blocking) [{}]: Recorded frame {} / {} ({:.1}%)", cam_name_clone, frame_idx + 1, num_frames, (frame_idx + 1) as f64 / num_frames as f64 * 100.0);
                    }
                }
                
                // VideoWriter is dropped here, releasing the file.
                info!("🏁 OpenCV (blocking) [{}]: Finished recording task in {:?}. Output file: {}", 
                    cam_name_clone, task_start_time.elapsed(), output_path_clone.display());
                Ok(output_path_clone)
            });
            record_tasks.push(task);
        }

        let task_results = join_all(record_tasks).await;
        let mut successful_paths = Vec::new();
        let mut  had_errors = false;

        info!("🏁 All parallel video recording tasks completed processing.");
        for (idx, result_outer) in task_results.into_iter().enumerate() {
            let cam_name_for_log = &camera_names_ordered.get(idx).map_or_else(|| "unknown_camera".to_string(), |cn| cn.clone());
            let output_path_for_log = &per_camera_output_paths.get(idx).map_or_else(|| PathBuf::from("unknown_path"), |p| p.clone());

            match result_outer { // Handle JoinError from spawn_blocking
                Ok(Ok(path)) => {
                    info!("✅ Successfully recorded video for '{}' to {}", cam_name_for_log, path.display());
                    successful_paths.push(path);
                }
                Ok(Err(e)) => { // Error from the task's Result
                    error!("❌ Error recording video for camera '{}' to '{}': {:#}", cam_name_for_log, output_path_for_log.display(), e);
                    had_errors = true;
                    // Attempt to delete partially created file on specific task error
                    if output_path_for_log.exists() {
                        debug!("Attempting to delete partially created file on error: {}", output_path_for_log.display());
                        if let Err(del_err) = std::fs::remove_file(output_path_for_log) {
                            warn!("Failed to delete partial file {} for camera '{}': {}", output_path_for_log.display(), cam_name_for_log, del_err);
                        }
                    }
                }
                Err(e) => { // Task panicked
                    error!("❌ Video recording task for camera '{}' (targeting '{}') panicked: {:#}", cam_name_for_log, output_path_for_log.display(), e);
                    had_errors = true;
                     if output_path_for_log.exists() {
                        debug!("Attempting to delete partially created file on panic: {}", output_path_for_log.display());
                        if let Err(del_err) = std::fs::remove_file(output_path_for_log) {
                            warn!("Failed to delete partial file {} for camera '{}' after panic: {}", output_path_for_log.display(), cam_name_for_log, del_err);
                        }
                    }
                }
            }
        }

        if successful_paths.is_empty() && !cameras_info.is_empty() && !capture_arcs.is_empty() {
             warn!(
                "🎬 Parallel video recording tasks completed, but no files were successfully produced from {} initialized streams. This might indicate issues during recording for all processed cameras.",
                capture_arcs.len()
            );
        } else if successful_paths.is_empty() && capture_arcs.is_empty() {
            info!("🎬 Video recording: No camera streams were available or initialized successfully.");
        } else if had_errors {
             info!(
                "⚠️ Partially completed video recording for {} out of {} camera streams in {:?}. {} file(s) successfully saved.",
                successful_paths.len(),
                capture_arcs.len(),
                overall_start_time.elapsed(),
                successful_paths.len()
            );
        }
        else {
            info!(
                "🎉 Successfully completed video recording for {} camera stream(s) in {:?}. {} file(s) saved.",
                successful_paths.len(),
                overall_start_time.elapsed(),
                successful_paths.len()
            );
        }
        Ok(successful_paths)
    }
} 