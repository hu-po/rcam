use crate::camera::camera_entity::{CameraEntity, CameraState};
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


#[derive(Clone)]
pub struct CameraMediaManager { }

impl CameraMediaManager {
    pub fn new() -> Self {
        debug!("🖼️📹 Initializing CameraMediaManager...");
        let start_time = std::time::Instant::now();
        let manager = CameraMediaManager {};
        debug!("✅ CameraMediaManager initialized in {:?}", start_time.elapsed());
        manager
    }

    pub async fn capture_image(
        &self,
        camera_entity: &mut CameraEntity, 
        app_config: &ApplicationConfig,
        output_path: PathBuf,
        delay: Option<Duration>,
    ) -> Result<PathBuf> {
        let cam_name_outer = camera_entity.config.name.clone();
        let overall_start_time = std::time::Instant::now();
        info!(
            "📸 Attempting OpenCV image capture for camera: '{}' to {}",
            cam_name_outer,
            output_path.display()
        );
        camera_entity.update_state(CameraState::Connecting);

        if let Some(d) = delay {
            debug!("  Applying delay of {:?} before capture for '{}'", d, cam_name_outer);
            tokio::time::sleep(d).await;
        }

        let rtsp_url_fetch_start = std::time::Instant::now();
        let rtsp_url = camera_entity.get_rtsp_url()?;
        debug!("  Fetched RTSP URL for '{}' in {:?}: {}", cam_name_outer, rtsp_url_fetch_start.elapsed(), rtsp_url);
        let output_path_clone = output_path.clone();
        let image_format = app_config.image_format.clone();

        let cam_name_for_block = cam_name_outer.clone();
        
        let join_handle = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            let cam_name = cam_name_for_block;
            let blocking_code_start_time = std::time::Instant::now();
            info!("🖼️ OpenCV (blocking): Connecting to RTSP URL: {} for image capture for camera '{}'", rtsp_url, cam_name);
            
            let cap_create_start = std::time::Instant::now();
            let mut cap = videoio::VideoCapture::from_file(rtsp_url.as_str(), videoio::CAP_ANY)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: Failed to create VideoCapture for '{}' 💀", cam_name))?;
            debug!("  OpenCV (blocking): VideoCapture created for '{}' in {:?}", cam_name, cap_create_start.elapsed());
            
            let opened_check_start = std::time::Instant::now();
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: Failed to check if VideoCapture is opened for '{}' ❓", cam_name))?;
            debug!("  OpenCV (blocking): VideoCapture::is_opened check for '{}' in {:?}", cam_name, opened_check_start.elapsed());
            if !opened {
                error!("❌ OpenCV (blocking): Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", cam_name, rtsp_url);
                return Err(anyhow!("Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", cam_name, rtsp_url));
            }
            info!("👍 OpenCV (blocking): RTSP stream opened for '{}'", cam_name);

            let mut frame = opencv_core::Mat::default();
            let frame_read_start = std::time::Instant::now();
            cap.read(&mut frame)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: Failed to read frame from '{}' ��️💥", cam_name))?;
            debug!("  OpenCV (blocking): Frame read for '{}' in {:?}", cam_name, frame_read_start.elapsed());

            if frame.empty() {
                error!("❌ OpenCV (blocking): Captured frame is empty for '{}'. Stream might be unstable or finished.", cam_name);
                return Err(anyhow!("Captured frame is empty for '{}'. Stream might be unstable or finished.", cam_name));
            }
            info!("🖼️ OpenCV (blocking): Frame read successfully for '{}' (size: {}x{})", cam_name, frame.cols(), frame.rows());

            if let Some(parent_dir) = output_path_clone.parent() {
                if !parent_dir.exists() {
                    debug!("  OpenCV (blocking): Creating parent directory for image: {}", parent_dir.display());
                    std::fs::create_dir_all(parent_dir)
                        .with_context(|| format!("OpenCV: Failed to create parent directory for image '{}' 📁💥", output_path_clone.display()))?;
                }
            }

            let mut params = opencv_core::Vector::<i32>::new();
            if image_format.to_lowercase() == "jpg" || image_format.to_lowercase() == "jpeg" {
                debug!("  OpenCV (blocking): Setting JPEG quality to 95 for '{}'", cam_name);
                params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                params.push(95);
            }

            let imwrite_start = std::time::Instant::now();
            imgcodecs::imwrite(output_path_clone.to_str().context("Invalid output path for image (not UTF-8) 🛤️❌")?, &frame, &params)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: Failed to save image for '{}' to '{}' 💾💥", cam_name, output_path_clone.display()))?;
            debug!("  OpenCV (blocking): Image written for '{}' in {:?}", cam_name, imwrite_start.elapsed());
            
            info!("✅ OpenCV (blocking): Image saved for '{}' to {}. Total blocking task time: {:?}", cam_name, output_path_clone.display(), blocking_code_start_time.elapsed());
            let release_start = std::time::Instant::now();
            cap.release()
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: Failed to release VideoCapture for '{}' 🚫", cam_name))?;
            debug!("  OpenCV (blocking): VideoCapture released for '{}' in {:?}", cam_name, release_start.elapsed());
            Ok(output_path_clone)
        });

        let capture_result = join_handle.await
            .with_context(|| format!("💀 OpenCV image capture task for '{}' was cancelled or panicked", cam_name_outer))?
            .with_context(|| format!("💀 OpenCV image capture task for '{}' failed internally", cam_name_outer))?;

        camera_entity.update_state(CameraState::Connected);
        info!("🎉 Successfully captured image for '{}' in {:?}", cam_name_outer, overall_start_time.elapsed());
        Ok(capture_result)
    }

    pub async fn record_video(
        &self,
        camera_entity: &mut CameraEntity, 
        app_config: &ApplicationConfig,
        output_path: PathBuf,
        duration: Duration,
    ) -> Result<PathBuf> {
        let cam_name_outer = camera_entity.config.name.clone();
        let overall_start_time = std::time::Instant::now();
        info!(
            "📹 Attempting OpenCV video recording for camera: '{}' to {} for {:?}",
            cam_name_outer,
            output_path.display(),
            duration
        );
        camera_entity.update_state(CameraState::Connecting);

        let rtsp_url_fetch_start = std::time::Instant::now();
        let rtsp_url = camera_entity.get_rtsp_url()?;
        debug!("  Fetched RTSP URL for '{}' in {:?}: {}", cam_name_outer, rtsp_url_fetch_start.elapsed(), rtsp_url);
        let output_path_clone = output_path.clone();
        let app_config_clone = app_config.clone();

        let cam_name_for_block = cam_name_outer.clone();
        let join_handle = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            let cam_name = cam_name_for_block;
            let blocking_code_start_time = std::time::Instant::now();
            info!("🎬 OpenCV (blocking): Connecting to RTSP URL: {} for video recording for '{}'", rtsp_url, cam_name);
            let cap_create_start = std::time::Instant::now();
            let mut cap = videoio::VideoCapture::from_file(rtsp_url.as_str(), videoio::CAP_ANY)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: VideoCapture creation failed for '{}' 💀", cam_name))?;
            debug!("  OpenCV (blocking): VideoCapture created for '{}' in {:?}", cam_name, cap_create_start.elapsed());
            let opened_check_start = std::time::Instant::now();
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("OpenCV: VideoCapture::is_opened check failed for '{}' ❓", cam_name))?;
            debug!("  OpenCV (blocking): VideoCapture::is_opened check for '{}' in {:?}", cam_name, opened_check_start.elapsed());
            if !opened {
                error!("❌ OpenCV (blocking): Failed to open RTSP stream for video for '{}': {}", cam_name, rtsp_url);
                return Err(anyhow!("Failed to open RTSP stream for video for '{}': {}", cam_name, rtsp_url));
            }
            info!("👍 OpenCV (blocking): RTSP stream opened for video for '{}'", cam_name);

            let prop_fetch_start = std::time::Instant::now();
            let frame_width = cap.get(videoio::CAP_PROP_FRAME_WIDTH).map_err(|e| anyhow!(e))? as i32;
            let frame_height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT).map_err(|e| anyhow!(e))? as i32;
            let mut fps = cap.get(videoio::CAP_PROP_FPS).map_err(|e| anyhow!(e))?;
            debug!("  OpenCV (blocking): Fetched video properties for '{}' ({}x{}, raw_fps: {}) in {:?}", cam_name, frame_width, frame_height, fps, prop_fetch_start.elapsed());
            if fps <= 0.0 {
                warn!("⚠️ Camera '{}' reported FPS <= 0 ({}). Using configured FPS: {}", cam_name, fps, app_config_clone.video_fps);
                fps = app_config_clone.video_fps as f64;
            }

            let fourcc_str = match app_config_clone.video_codec.to_lowercase().as_str() {
                "mjpg" | "mjpeg" => "MJPG",
                "xvid" => "XVID",
                "mp4v" => "MP4V",
                "h264" if app_config_clone.video_format.to_lowercase() == "avi" => "H264",
                "h264" if app_config_clone.video_format.to_lowercase() == "mp4" => "avc1",
                codec_val => {
                    warn!("⚠️ Unsupported video_codec '{}' for OpenCV VideoWriter with format '{}'. Defaulting to MJPG.", codec_val, app_config_clone.video_format);
                    "MJPG"
                }
            };
            debug!("  OpenCV (blocking): Using FourCC: '{}' for '{}'", fourcc_str, cam_name);
            let fourcc = videoio::VideoWriter::fourcc(fourcc_str.chars().nth(0).unwrap_or('M'), fourcc_str.chars().nth(1).unwrap_or('J'), fourcc_str.chars().nth(2).unwrap_or('P'), fourcc_str.chars().nth(3).unwrap_or('G'))
                .map_err(|e| anyhow!(e))?;

            let writer_create_start = std::time::Instant::now();
            let mut writer = videoio::VideoWriter::new(
                output_path_clone.to_str().context("Invalid output path for video (not UTF-8) 🛤️❌")?,
                fourcc,
                fps,
                opencv_core::Size::new(frame_width, frame_height),
                true,
            ).map_err(|e| anyhow!(e))?;
            debug!("  OpenCV (blocking): VideoWriter created for '{}' in {:?}", cam_name, writer_create_start.elapsed());

            let opened_writer_check_start = std::time::Instant::now();
            let opened_writer = videoio::VideoWriter::is_opened(&writer)
                 .map_err(|e| anyhow!(e))?;
            debug!("  OpenCV (blocking): VideoWriter::is_opened check for '{}' in {:?}", cam_name, opened_writer_check_start.elapsed());
            if !opened_writer {
                error!("❌ OpenCV (blocking): Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_path_clone.display());
                return Err(anyhow!("Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_path_clone.display()));
            }
            info!("✍️ OpenCV (blocking): VideoWriter opened for '{}' to {}", cam_name, output_path_clone.display());

            let start_time = std::time::Instant::now();
            let mut frame_count = 0;
            let mut _last_error_log_time = std::time::Instant::now();

            while start_time.elapsed() < duration {
                let mut frame = opencv_core::Mat::default();
                let frame_read_start_iter = std::time::Instant::now();
                if !cap.read(&mut frame).map_err(|e| anyhow!(e))? {
                    if _last_error_log_time.elapsed().as_secs() > 5 {
                        error!("🚫 Camera '{}': Failed to read frame or stream ended prematurely (after {} frames, read attempt took {:?}).", cam_name, frame_count, frame_read_start_iter.elapsed());
                        _last_error_log_time = std::time::Instant::now();
                    }
                    break; 
                }
                if frame.empty() {
                     if _last_error_log_time.elapsed().as_secs() > 5 {
                        warn!("👻 Camera '{}': Captured empty frame mid-stream (frame #{}, read attempt took {:?}).", cam_name, frame_count + 1, frame_read_start_iter.elapsed());
                        _last_error_log_time = std::time::Instant::now();
                    }
                    continue;
                }

                let write_frame_start = std::time::Instant::now();
                writer.write(&frame).map_err(|e| anyhow!(e))?;
                frame_count += 1;
                if frame_count % 100 == 0 {
                    debug!("  OpenCV (blocking) [{}]: Wrote frame #{}, took {:?}. Total elapsed: {:?}", cam_name, frame_count, write_frame_start.elapsed(), start_time.elapsed());
                }
            }
            info!("🏁 OpenCV (blocking): Finished recording for '{}'. Recorded {} frames in {:?}. Total blocking task time: {:?}", cam_name, frame_count, start_time.elapsed(), blocking_code_start_time.elapsed());

            let release_start = std::time::Instant::now();
            writer.release().map_err(|e| anyhow!(e))?;
            cap.release().map_err(|e| anyhow!(e))?;
            debug!("  OpenCV (blocking): Released VideoWriter and VideoCapture for '{}' in {:?}", cam_name, release_start.elapsed());
            
            if frame_count == 0 {
                warn!("⚠️ Camera '{}': Video recording resulted in 0 frames. The output file might be empty or invalid.", cam_name);
            }

            Ok(output_path_clone)
        });

        let record_result = join_handle.await
            .with_context(|| format!("💀 OpenCV video recording task for '{}' was cancelled or panicked", cam_name_outer))?
            .with_context(|| format!("💀 OpenCV video recording task for '{}' failed internally", cam_name_outer))?;

        camera_entity.update_state(CameraState::Connected);
        info!("🎉 Successfully recorded video for '{}' in {:?}", cam_name_outer, overall_start_time.elapsed());
        Ok(record_result)
    }
} 