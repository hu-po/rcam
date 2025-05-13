use crate::camera::camera_entity::{CameraEntity, CameraState};
use crate::app_config::ApplicationConfig;
use anyhow::{Context, Result, anyhow};
use log::{info, warn, error};
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
        CameraMediaManager {}
    }

    pub async fn capture_image(
        &self,
        camera_entity: &mut CameraEntity, 
        app_config: &ApplicationConfig,
        output_path: PathBuf,
        delay: Option<Duration>,
    ) -> Result<PathBuf> {
        let cam_name_outer = camera_entity.config.name.clone();
        info!(
            "Attempting OpenCV image capture for camera: '{}' to {}",
            cam_name_outer,
            output_path.display()
        );
        camera_entity.update_state(CameraState::Connecting);

        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }

        let rtsp_url = camera_entity.get_rtsp_url()?;
        let output_path_clone = output_path.clone();
        let image_format = app_config.image_format.clone();

        let cam_name_for_block = cam_name_outer.clone();
        
        let join_handle = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            let cam_name = cam_name_for_block;
            info!("OpenCV (blocking): Connecting to RTSP URL: {} for image capture for camera '{}'", rtsp_url, cam_name);
            
            let mut cap = videoio::VideoCapture::from_file(rtsp_url.as_str(), videoio::CAP_ANY)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Failed to create VideoCapture for '{}'", cam_name))?;
            
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Failed to check if VideoCapture is opened for '{}'", cam_name))?;
            if !opened {
                return Err(anyhow!("Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", cam_name, rtsp_url));
            }
            info!("OpenCV (blocking): RTSP stream opened for '{}'", cam_name);

            let mut frame = opencv_core::Mat::default();
            cap.read(&mut frame)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Failed to read frame from '{}'", cam_name))?;

            if frame.empty() {
                return Err(anyhow!("Captured frame is empty for '{}'. Stream might be unstable or finished.", cam_name));
            }
            info!("OpenCV (blocking): Frame read successfully for '{}'", cam_name);

            if let Some(parent_dir) = output_path_clone.parent() {
                if !parent_dir.exists() {
                    std::fs::create_dir_all(parent_dir)
                        .with_context(|| format!("Failed to create parent directory for image '{}'", output_path_clone.display()))?;
                }
            }

            let mut params = opencv_core::Vector::<i32>::new();
            if image_format.to_lowercase() == "jpg" || image_format.to_lowercase() == "jpeg" {
                params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                params.push(95);
            }

            imgcodecs::imwrite(output_path_clone.to_str().context("Invalid output path for image (not UTF-8)")?, &frame, &params)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Failed to save image for '{}' to '{}'", cam_name, output_path_clone.display()))?;
            
            info!("OpenCV (blocking): Image saved for '{}' to {}", cam_name, output_path_clone.display());
            cap.release()
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Failed to release VideoCapture for '{}'", cam_name))?;
            Ok(output_path_clone)
        });

        let capture_result = join_handle.await
            .with_context(|| format!("OpenCV image capture task for '{}' was cancelled or panicked", cam_name_outer))?
            .with_context(|| format!("OpenCV image capture task for '{}' failed internally", cam_name_outer))?;

        camera_entity.update_state(CameraState::Connected);
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
        info!(
            "Attempting OpenCV video recording for camera: '{}' to {} for {:?}",
            cam_name_outer,
            output_path.display(),
            duration
        );
        camera_entity.update_state(CameraState::Connecting);

        let rtsp_url = camera_entity.get_rtsp_url()?;
        let output_path_clone = output_path.clone();
        let app_config_clone = app_config.clone();

        let cam_name_for_block = cam_name_outer.clone();
        let join_handle = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            let cam_name = cam_name_for_block;
            info!("OpenCV (blocking): Connecting to RTSP URL: {} for video recording for '{}'", rtsp_url, cam_name);
            let mut cap = videoio::VideoCapture::from_file(rtsp_url.as_str(), videoio::CAP_ANY)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("VideoCapture creation failed for '{}'", cam_name))?;
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("VideoCapture::is_opened check failed for '{}'", cam_name))?;
            if !opened {
                return Err(anyhow!("Failed to open RTSP stream for video for '{}': {}", cam_name, rtsp_url));
            }
            info!("OpenCV (blocking): RTSP stream opened for video for '{}'", cam_name);

            let frame_width = cap.get(videoio::CAP_PROP_FRAME_WIDTH).map_err(|e| anyhow!(e))? as i32;
            let frame_height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT).map_err(|e| anyhow!(e))? as i32;
            let mut fps = cap.get(videoio::CAP_PROP_FPS).map_err(|e| anyhow!(e))?;
            if fps <= 0.0 {
                warn!("Camera '{}' reported FPS <= 0 ({}). Using configured FPS: {}", cam_name, fps, app_config_clone.video_fps);
                fps = app_config_clone.video_fps as f64;
            }

            let fourcc_str = match app_config_clone.video_codec.to_lowercase().as_str() {
                "mjpg" | "mjpeg" => "MJPG",
                "xvid" => "XVID",
                "mp4v" => "MP4V",
                "h264" if app_config_clone.video_format.to_lowercase() == "avi" => "H264",
                "h264" if app_config_clone.video_format.to_lowercase() == "mp4" => "avc1",
                _ => {
                    warn!("Unsupported video_codec '{}' for OpenCV VideoWriter with format '{}'. Defaulting to MJPG.", app_config_clone.video_codec, app_config_clone.video_format);
                    "MJPG"
                }
            };
            let fourcc = videoio::VideoWriter::fourcc(fourcc_str.chars().nth(0).unwrap_or('M'), fourcc_str.chars().nth(1).unwrap_or('J'), fourcc_str.chars().nth(2).unwrap_or('P'), fourcc_str.chars().nth(3).unwrap_or('G'))
                .map_err(|e| anyhow!(e))?;

            let mut writer = videoio::VideoWriter::new(
                output_path_clone.to_str().context("Invalid output path for video (not UTF-8)")?,
                fourcc,
                fps,
                opencv_core::Size::new(frame_width, frame_height),
                true,
            ).map_err(|e| anyhow!(e))?;

            let opened_writer = videoio::VideoWriter::is_opened(&writer)
                 .map_err(|e| anyhow!(e))?;
            if !opened_writer {
                return Err(anyhow!("Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_path_clone.display()));
            }
            info!("OpenCV (blocking): VideoWriter opened for '{}' to {}", cam_name, output_path_clone.display());

            let start_time = std::time::Instant::now();
            let mut frame_count = 0;
            let mut _last_error_log_time = std::time::Instant::now();

            while start_time.elapsed() < duration {
                let mut frame = opencv_core::Mat::default();
                if !cap.read(&mut frame).map_err(|e| anyhow!(e))? {
                    if _last_error_log_time.elapsed().as_secs() > 5 {
                        error!("Camera '{}': Failed to read frame or stream ended prematurely.", cam_name);
                        _last_error_log_time = std::time::Instant::now();
                    }
                    break; 
                }
                if frame.empty() {
                     if _last_error_log_time.elapsed().as_secs() > 5 {
                        warn!("Camera '{}': Captured empty frame mid-stream.", cam_name);
                        _last_error_log_time = std::time::Instant::now();
                    }
                    continue;
                }

                writer.write(&frame).map_err(|e| anyhow!(e))?;
                frame_count += 1;
            }
            info!("OpenCV (blocking): Finished recording for '{}'. Recorded {} frames.", cam_name, frame_count);

            writer.release().map_err(|e| anyhow!(e))?;
            cap.release().map_err(|e| anyhow!(e))?;
            
            if frame_count == 0 {
                warn!("Camera '{}': Video recording resulted in 0 frames. The output file might be empty or invalid.", cam_name);
            }

            Ok(output_path_clone)
        });

        let record_result = join_handle.await
            .with_context(|| format!("OpenCV video recording task for '{}' was cancelled or panicked", cam_name_outer))?
            .with_context(|| format!("OpenCV video recording task for '{}' failed internally", cam_name_outer))?;

        camera_entity.update_state(CameraState::Connected);
        Ok(record_result)
    }
} 