use crate::app_config::ApplicationConfig;
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
        app_config: &ApplicationConfig,
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
            warn!("��️ No camera streams could be initialized for image capture. Aborting.");
            return Ok(Vec::new());
        }
        info!("Successfully initialized {} out of {} camera streams for image capture.", capture_arcs.len(), cameras_info.len());

        // 2. Prepare output directory (output_paths will be generated inside spawn_blocking)
        if !output_dir.exists() {
            debug!("Creating output directory for images: {}", output_dir.display());
            std::fs::create_dir_all(&output_dir)
                .with_context(|| format!("Failed to create output directory for images: {}", output_dir.display()))?;
        }
        
        let app_config_clone = app_config.clone();

        // 3. Spawn blocking task for capture
        let output_dir_clone = output_dir.clone(); // Clone for spawn_blocking
        let task_future = tokio::task::spawn_blocking(move || -> Result<Vec<PathBuf>> {
            let blocking_code_start_time = std::time::Instant::now();
            info!("🖼️ OpenCV (blocking): Starting image capture task for {} cameras.", capture_arcs.len());

            let mut locked_caps = Vec::new();
            for (i, cap_arc) in capture_arcs.iter().enumerate() {
                let cam_name = &camera_names_ordered[i];
                debug!("  OpenCV (blocking): Locking capture for {}", cam_name);
                match futures::executor::block_on(cap_arc.lock()) {
                    guard => locked_caps.push(guard),
                }
            }
            debug!("  OpenCV (blocking): All {} captures locked.", locked_caps.len());

            let mut saved_image_paths: Vec<PathBuf> = Vec::new();
            let mut frame = opencv_core::Mat::default(); // Define frame outside the loop

            for (i, mut cap_guard) in locked_caps.into_iter().enumerate() {
                let cam_name = &camera_names_ordered[i];
                debug!("  OpenCV (blocking): Preparing to capture image for '{}'", cam_name);
                
                let frame_read_start = std::time::Instant::now();
                if !cap_guard.read(&mut frame).map_err(|e| anyhow!(e).context(format!("OpenCV: Failed to read frame from '{}' for capture", cam_name)))? {
                    error!("❌ OpenCV (blocking): Failed to read frame for '{}'. Skipping this camera.", cam_name);
                    continue;
                }
                // let frame_read_timestamp = std::time::Instant::now(); // Or use chrono for wall clock. Using Instant for debug, filename uses chrono.
                debug!("  OpenCV (blocking): Frame read for '{}' in {:?}", cam_name, frame_read_start.elapsed());

                if frame.empty() {
                    error!("❌ OpenCV (blocking): Captured frame is empty for '{}'. Stream might be unstable or finished. Skipping.", cam_name);
                    continue;
                }
                info!("🖼️ OpenCV (blocking): Frame read successfully for '{}' (size: {}x{})", cam_name, frame.cols(), frame.rows());

                // ---- Generate timestamp AFTER successful read ----
                // Using crate::common::timestamp_utils::current_local_timestamp_str as per conceptual change.
                // Ensure this utility function exists and is accessible.
                // If not, replace with:
                // let current_capture_timestamp_str = chrono::Local::now().format(&app_config_clone.filename_timestamp_format).to_string();
                let current_capture_timestamp_str = crate::common::timestamp_utils::current_local_timestamp_str(
                    &app_config_clone.filename_timestamp_format
                );
                let filename = format!("{}_{}.{}", cam_name, current_capture_timestamp_str, app_config_clone.image_format);
                let output_path = output_dir_clone.join(&filename);
                // ---- End timestamp generation ----

                debug!("  OpenCV (blocking): Saving frame for '{}' to {} (timestamp: {})", cam_name, output_path.display(), current_capture_timestamp_str);

                if let Some(parent_dir) = output_path.parent() {
                    if !parent_dir.exists() {
                        std::fs::create_dir_all(parent_dir)
                            .with_context(|| format!("OpenCV: Failed to create parent directory for image '{}'", output_path.display()))?;
                    }
                }
                
                let mut params = opencv_core::Vector::<i32>::new();
                if app_config_clone.image_format.to_lowercase() == "jpg" || app_config_clone.image_format.to_lowercase() == "jpeg" {
                    params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                    params.push(95);
                }

                let imwrite_start = std::time::Instant::now();
                imgcodecs::imwrite(output_path.to_str().context("Invalid output path for image (not UTF-8)")?, &frame, &params)
                    .map_err(|e| anyhow!(e).context(format!("OpenCV: Failed to save image for '{}' to '{}'", cam_name, output_path.display())))?;
                debug!("  OpenCV (blocking): Image written for '{}' in {:?}", cam_name, imwrite_start.elapsed());
                info!("✅ OpenCV (blocking): Image saved for '{}' to {}", cam_name, output_path.display());
                saved_image_paths.push(output_path.clone());
            }

            info!("🏁 OpenCV (blocking): Finished image capture task for {} cameras in {:?}. Saved {} images.", 
                camera_names_ordered.len(), blocking_code_start_time.elapsed(), saved_image_paths.len());
            Ok(saved_image_paths)
        });

        match task_future.await {
            Ok(Ok(paths)) => {
                if paths.is_empty() && !cameras_info.is_empty() {
                    warn!(
                        "📸 Image capture completed but no files were produced. This might indicate an issue during capture for all cameras."
                    );
                } else if paths.is_empty() && cameras_info.is_empty() {
                    info!("📸 Image capture: No cameras were processed (likely due to RTSP URL issues).");
                } else {
                    info!(
                        "✅ Successfully captured {} image file(s) in {:?}",
                        paths.len(),
                        overall_start_time.elapsed()
                    );
                }
                Ok(paths)
            }
            Ok(Err(e)) => {
                error!("❌ Error during image capture blocking task: {:#}", e);
                Err(e)
            }
            Err(e) => {
                error!("❌ Image capture task panicked or was cancelled: {:#}", e);
                Err(anyhow!(e).context("Image capture task failed"))
            }
        }
    }

    pub async fn record_video(
        &self,
        cameras_info: &[(String, String)],
        app_config: &ApplicationConfig,
        output_dir: PathBuf,
        duration: Duration,
    ) -> Result<Vec<PathBuf>> {
        info!("📹 Attempting video recording for {} cameras for {:?}", cameras_info.len(), duration);
        let overall_start_time = std::time::Instant::now();

        if cameras_info.is_empty() {
            warn!("🎬 No cameras provided for recording.");
            return Ok(Vec::new());
        }

        // Parallelize capture initialization
        let mut capture_init_futures = Vec::new();
        let mut temp_camera_names_ordered = Vec::new(); // To keep order for matching results

        for (name, url) in cameras_info {
            debug!("  Queueing capture initialization for recording: {} ({})", name, url);
            temp_camera_names_ordered.push(name.clone());
            capture_init_futures.push(self.get_or_init_capture(name, url));
        }

        info!("  Initializing {} camera stream(s) for video recording concurrently...", capture_init_futures.len());
        let init_results = join_all(capture_init_futures).await;
        info!("  All camera stream initialization attempts for video recording completed.");

        let mut capture_arcs = Vec::new();
        let mut camera_names_ordered = Vec::new(); // For successfully initialized cameras

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

        let mut output_paths = Vec::new();
        if !output_dir.exists() {
            debug!("Creating output directory for videos: {}", output_dir.display());
            std::fs::create_dir_all(&output_dir)
                .with_context(|| format!("Failed to create output directory for videos: {}", output_dir.display()))?;
        }

        for name in &camera_names_ordered {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%f");
            let filename = format!("{}_{}.{}", name, timestamp, app_config.video_format);
            output_paths.push(output_dir.join(filename));
        }
        
        let app_config_clone = app_config.clone();
        let output_paths_clone = output_paths.clone();

        let task_future = tokio::task::spawn_blocking(move || -> Result<Vec<PathBuf>> {
            let blocking_code_start_time = std::time::Instant::now();
            info!("🎬 OpenCV (blocking): Starting recording task for {} cameras.", capture_arcs.len());

            let mut locked_caps = Vec::new();
            for (i, cap_arc) in capture_arcs.iter().enumerate() {
                let cam_name = &camera_names_ordered[i];
                debug!("  OpenCV (blocking): Locking capture for {}", cam_name);
                match futures::executor::block_on(cap_arc.lock()) {
                    guard => locked_caps.push(guard),
                }
            }
            debug!("  OpenCV (blocking): All {} captures locked.", locked_caps.len());

            let mut video_writers: Vec<videoio::VideoWriter> = Vec::new();
            for (i, cap_guard) in locked_caps.iter_mut().enumerate() {
                let cam_name = &camera_names_ordered[i];
                let output_file_path = &output_paths_clone[i];
                debug!("  OpenCV (blocking): Initializing VideoWriter for {} to {}", cam_name, output_file_path.display());

                let frame_width = cap_guard.get(videoio::CAP_PROP_FRAME_WIDTH).map_err(|e| anyhow!(e))? as i32;
                let frame_height = cap_guard.get(videoio::CAP_PROP_FRAME_HEIGHT).map_err(|e| anyhow!(e))? as i32;
                let mut fps = cap_guard.get(videoio::CAP_PROP_FPS).map_err(|e| anyhow!(e))?;
                
                if frame_width <= 0 || frame_height <= 0 {
                    error!("❌ OpenCV (blocking): Invalid frame dimensions ({}x{}) for camera '{}'. Aborting writer creation.", frame_width, frame_height, cam_name);
                    return Err(anyhow!("Invalid frame dimensions ({}x{}) for camera '{}'", frame_width, frame_height, cam_name));
                }

                if fps <= 0.0 {
                    warn!("⚠️ Camera '{}' reported FPS <= 0 ({}). Using configured FPS: {}", cam_name, fps, app_config_clone.video_fps);
                    fps = app_config_clone.video_fps as f64;
                } else {
                    debug!("  Camera '{}' reported FPS {}. Recording will use common FPS: {}", cam_name, fps, app_config_clone.video_fps);
                    fps = app_config_clone.video_fps as f64;
                }

                let fourcc_str = match app_config_clone.video_codec.to_lowercase().as_str() {
                    "mjpg" | "mjpeg" => "MJPG",
                    "xvid" => "XVID",
                    "mp4v" => "MP4V",
                    "h264" if app_config_clone.video_format.to_lowercase() == "avi" => "H264",
                    "h264" if app_config_clone.video_format.to_lowercase() == "mp4" => "avc1",
                    codec_val => {
                        warn!("⚠️ Unsupported video_codec '{}' for OpenCV VideoWriter with format '{}' for '{}'. Defaulting to MJPG.", codec_val, app_config_clone.video_format, cam_name);
                        "MJPG"
                    }
                };
                let fourcc = videoio::VideoWriter::fourcc(fourcc_str.chars().nth(0).unwrap_or('M'), fourcc_str.chars().nth(1).unwrap_or('J'), fourcc_str.chars().nth(2).unwrap_or('P'), fourcc_str.chars().nth(3).unwrap_or('G'))?;

                let writer = videoio::VideoWriter::new(
                    output_file_path.to_str().context("Invalid output path for video (not UTF-8)")?,
                    fourcc,
                    fps,
                    opencv_core::Size::new(frame_width, frame_height),
                    true,
                )?;

                if !videoio::VideoWriter::is_opened(&writer)? {
                    error!("❌ OpenCV (blocking): Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_file_path.display());
                    return Err(anyhow!("Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_file_path.display()));
                }
                info!("✍️ OpenCV (blocking): VideoWriter opened for '{}' to {}", cam_name, output_file_path.display());
                video_writers.push(writer);
            }

            if video_writers.len() != locked_caps.len() {
                error!("❌ OpenCV (blocking): Failed to initialize all VideoWriters. Expected {}, got {}. Aborting.", locked_caps.len(), video_writers.len());
                return Err(anyhow!("Failed to initialize all video writers for recording."));
            }
            debug!("  OpenCV (blocking): All {} VideoWriters initialized.", video_writers.len());

            let common_fps = app_config_clone.video_fps as f64;
            if common_fps <= 0.0 {
                error!("❌ Common FPS for recording is invalid ({}). Aborting.", common_fps);
                return Err(anyhow!("Invalid common FPS for recording: {}", common_fps));
            }
            let num_frames = (duration.as_secs_f64() * common_fps).round() as u64;
            info!("  OpenCV (blocking): Starting recording loop for {} frames (duration: {:?}, fps: {}).", num_frames, duration, common_fps);

            let mut _last_error_log_time = std::time::Instant::now();

            for frame_idx in 0..num_frames {
                for (i, cap_guard) in locked_caps.iter_mut().enumerate() {
                    let cam_name = &camera_names_ordered[i];
                    if !cap_guard.grab().with_context(|| format!("OpenCV: Grab failed for camera '{}'", cam_name))? {
                        if _last_error_log_time.elapsed().as_secs() > 2 {
                           error!("🚫 OpenCV (blocking): Failed to grab frame for '{}' (stream might have ended). Frame index: {}", cam_name, frame_idx);
                           _last_error_log_time = std::time::Instant::now();
                        }
                        return Err(anyhow!("Failed to grab frame for camera '{}' at frame index {}. Aborting recording.", cam_name, frame_idx));
                    }
                }

                let mut temp_frame = opencv_core::Mat::default();
                for (i, cap_guard) in locked_caps.iter_mut().enumerate() {
                    let cam_name = &camera_names_ordered[i];
                    if cap_guard.retrieve(&mut temp_frame, 0).with_context(|| format!("OpenCV: Retrieve failed for '{}'", cam_name))? {
                        if temp_frame.empty() {
                            if _last_error_log_time.elapsed().as_secs() > 2 {
                                warn!("👻 OpenCV (blocking): Retrieved empty frame for '{}' at frame index {}. Skipping write.", cam_name, frame_idx);
                                _last_error_log_time = std::time::Instant::now();
                            }
                            continue; 
                        }
                        video_writers[i].write(&temp_frame).with_context(|| format!("OpenCV: Write failed for '{}' to '{}'", cam_name, output_paths_clone[i].display()))?;
                    } else {
                        if _last_error_log_time.elapsed().as_secs() > 2 {
                           error!("🚫 OpenCV (blocking): Failed to retrieve frame for '{}' after grab (frame index {}).", cam_name, frame_idx);
                            _last_error_log_time = std::time::Instant::now();
                        }
                        return Err(anyhow!("Failed to retrieve frame for camera '{}' at frame index {}. Aborting recording.", cam_name, frame_idx));
                    }
                }
                
                if frame_idx > 0 && frame_idx % (common_fps.round() as u64 * 5) == 0 {
                    debug!("  OpenCV (blocking): Recorded frame {} / {} ({:.1}%)", frame_idx + 1, num_frames, (frame_idx + 1) as f64 / num_frames as f64 * 100.0);
                }
            }

            for (i, writer) in video_writers.into_iter().enumerate() {
                drop(writer);
                debug!("  OpenCV (blocking): VideoWriter for {} released.", camera_names_ordered[i]);
            }
            
            info!("🏁 OpenCV (blocking): Finished recording task for {} cameras in {:?}. Output files: {:?}", 
                camera_names_ordered.len(), blocking_code_start_time.elapsed(), output_paths_clone);
            Ok(output_paths_clone)
        });

        task_future.await
            .with_context(|| "Video recording task panicked or was cancelled")?
            .map(|paths| {
                info!("🎉 Successfully completed video recording for {} cameras in {:?}.", cameras_info.len(), overall_start_time.elapsed());
                paths
            })
            .map_err(|e| {
                error!("❌ Video recording failed: {:?}", e);
                for path in &output_paths {
                     if path.exists() {
                        debug!("Attempting to delete partially created file on error: {}", path.display());
                        if let Err(del_err) = std::fs::remove_file(path) {
                            warn!("Failed to delete partial file {}: {}", path.display(), del_err);
                        }
                    }
                }
                e
            })
    }
} 