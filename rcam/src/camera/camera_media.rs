use crate::camera::camera_entity::{CameraEntity, CameraState};
use crate::app_config::ApplicationConfig;
use crate::errors::AppError;
use log::{info, warn, error};
use std::path::PathBuf;
use std::time::Duration;
use opencv::{
    prelude::*,
    videoio,
    imgcodecs,
    core as opencv_core
};

// MediaBackend enum removed

#[derive(Clone)] // Added Clone for use in operations modules
pub struct CameraMediaManager {
    // No backend field needed if always OpenCV
}

impl CameraMediaManager {
    pub fn new() -> Self { // No backend argument
        CameraMediaManager {}
    }

    pub async fn capture_image(
        &self,
        camera_entity: &mut CameraEntity, 
        app_config: &ApplicationConfig,
        output_path: PathBuf,
        delay: Option<Duration>, // Retained delay parameter
    ) -> Result<PathBuf, AppError> {
        let cam_name = camera_entity.config.name.clone();
        info!(
            "Attempting OpenCV image capture for camera: '{}' to {}",
            cam_name,
            output_path.display()
        );
        camera_entity.update_state(CameraState::Connecting);

        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }

        let rtsp_url = camera_entity.get_rtsp_url()?;
        let output_path_clone = output_path.clone();
        let image_format = app_config.image_format.clone(); // Clone for closure

        // OpenCV operations are blocking, so spawn_blocking is essential.
        let capture_result = tokio::task::spawn_blocking(move || -> Result<PathBuf, AppError> {
            info!("OpenCV (blocking): Connecting to RTSP URL: {} for image capture for camera '{}'", rtsp_url, cam_name);
            let mut cap = videoio::VideoCapture::from_url(&rtsp_url, videoio::CAP_ANY)
                .map_err(|e| AppError::OpenCV(format!("Failed to create VideoCapture for '{}': {}", cam_name, e)))?;
            
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| AppError::OpenCV(format!("Failed to check if VideoCapture is opened for '{}': {}", cam_name, e)))?;
            if !opened {
                return Err(AppError::OpenCV(format!("Failed to open RTSP stream for '{}': {} - Check camera availability and RTSP path.", cam_name, rtsp_url)));
            }
            info!("OpenCV (blocking): RTSP stream opened for '{}'", cam_name);

            let mut frame = opencv_core::Mat::default();
            cap.read(&mut frame)
                .map_err(|e| AppError::OpenCV(format!("Failed to read frame from '{}': {}", cam_name, e)))?;

            if frame.empty() {
                return Err(AppError::OpenCV(format!("Captured frame is empty for '{}'. Stream might be unstable or finished.", cam_name)));
            }
            info!("OpenCV (blocking): Frame read successfully for '{}'", cam_name);

            // Ensure the output directory exists (it should from the calling operation, but double check)
            if let Some(parent_dir) = output_path_clone.parent() {
                if !parent_dir.exists() {
                    std::fs::create_dir_all(parent_dir).map_err(|e| AppError::Io(format!("Failed to create parent directory for image '{}': {}", output_path_clone.display(),e)))?;
                }
            }

            let mut params = opencv_core::Vector::<i32>::new();
            // Add compression parameters if needed, e.g., for JPEG quality
            if image_format.to_lowercase() == "jpg" || image_format.to_lowercase() == "jpeg" {
                params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                params.push(95); // Example JPEG quality
            }

            imgcodecs::imwrite(output_path_clone.to_str().ok_or_else(|| AppError::Io("Invalid output path for image".to_string()))?, &frame, &params)
                .map_err(|e| AppError::OpenCV(format!("Failed to save image for '{}' to '{}': {}", cam_name, output_path_clone.display(),e)))?;
            
            info!("OpenCV (blocking): Image saved for '{}' to {}", cam_name, output_path_clone.display());
            cap.release().map_err(|e| AppError::OpenCV(format!("Failed to release VideoCapture for '{}': {}", cam_name,e)))?;
            Ok(output_path_clone)
        }).await.map_err(|e| AppError::Task(format!("OpenCV image capture task failed for '{}': {}", cam_name, e)))??; // JoinError then AppError
        
        camera_entity.update_state(CameraState::Connected); // Or Idle if one-off
        Ok(capture_result)
    }

    pub async fn record_video(
        &self,
        camera_entity: &mut CameraEntity, 
        app_config: &ApplicationConfig,
        output_path: PathBuf,
        duration: Duration,
    ) -> Result<PathBuf, AppError> {
        let cam_name = camera_entity.config.name.clone();
        info!(
            "Attempting OpenCV video recording for camera: '{}' to {} for {:?}",
            cam_name,
            output_path.display(),
            duration
        );
        camera_entity.update_state(CameraState::Connecting);

        let rtsp_url = camera_entity.get_rtsp_url()?;
        let output_path_clone = output_path.clone();
        let app_config_clone = app_config.clone(); // Clone for the closure

        // OpenCV operations are blocking
        let record_result = tokio::task::spawn_blocking(move || -> Result<PathBuf, AppError> {
            info!("OpenCV (blocking): Connecting to RTSP URL: {} for video recording for '{}'", rtsp_url, cam_name);
            let mut cap = videoio::VideoCapture::from_url(&rtsp_url, videoio::CAP_ANY)
                .map_err(|e| AppError::OpenCV(format!("VideoCapture creation failed for '{}': {}", cam_name, e)))?;
            let opened = videoio::VideoCapture::is_opened(&cap)
                .map_err(|e| AppError::OpenCV(format!("VideoCapture::is_opened check failed for '{}': {}", cam_name, e)))?;
            if !opened {
                return Err(AppError::OpenCV(format!("Failed to open RTSP stream for video for '{}': {} ", cam_name, rtsp_url)));
            }
            info!("OpenCV (blocking): RTSP stream opened for video for '{}'", cam_name);

            let frame_width = cap.get(videoio::CAP_PROP_FRAME_WIDTH).map_err(|e| AppError::OpenCV(format!("Failed to get frame width for '{}': {}", cam_name, e)))? as i32;
            let frame_height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT).map_err(|e| AppError::OpenCV(format!("Failed to get frame height for '{}': {}", cam_name, e)))? as i32;
            let mut fps = cap.get(videoio::CAP_PROP_FPS).map_err(|e| AppError::OpenCV(format!("Failed to get FPS for '{}': {}", cam_name, e)))?;
            if fps <= 0.0 {
                warn!("Camera '{}' reported FPS <= 0 ({}). Using configured FPS: {}", cam_name, fps, app_config_clone.video_fps);
                fps = app_config_clone.video_fps as f64;
            }

            // Determine FourCC code - this is a simplistic mapping
            let fourcc_str = match app_config_clone.video_codec.to_lowercase().as_str() {
                "mjpg" | "mjpeg" => "MJPG",
                "xvid" => "XVID",
                "mp4v" => "MP4V",
                "h264" if app_config_clone.video_format.to_lowercase() == "avi" => "H264", // H264 in AVI might be specific
                "h264" if app_config_clone.video_format.to_lowercase() == "mp4" => "avc1", // Common for MP4
                 // "copy" is not directly usable for VideoWriter; it implies re-muxing not re-encoding.
                 // OpenCV VideoWriter always re-encodes. If "copy" is desired, FFmpeg direct is better.
                 // For now, default to MJPG if "copy" or unknown, as it's widely compatible.
                _ => {
                    warn!("Unsupported video_codec '{}' for OpenCV VideoWriter with format '{}'. Defaulting to MJPG.", app_config_clone.video_codec, app_config_clone.video_format);
                    "MJPG"
                }
            };
            let fourcc = videoio::VideoWriter::fourcc(fourcc_str.chars().nth(0).unwrap_or('M'), fourcc_str.chars().nth(1).unwrap_or('J'), fourcc_str.chars().nth(2).unwrap_or('P'), fourcc_str.chars().nth(3).unwrap_or('G'))
                .map_err(|e| AppError::OpenCV(format!("Failed to create FourCC for '{}' with codec '{}': {}", cam_name, fourcc_str, e)))?;

            let mut writer = videoio::VideoWriter::new(
                output_path_clone.to_str().ok_or_else(|| AppError::Io("Invalid output path for video".to_string()))?,
                fourcc,
                fps,
                opencv_core::Size::new(frame_width, frame_height),
                true, // isColor
            ).map_err(|e| AppError::OpenCV(format!("VideoWriter creation failed for '{}': {}", cam_name, e)))?;

            let opened_writer = videoio::VideoWriter::is_opened(&writer)
                 .map_err(|e| AppError::OpenCV(format!("VideoWriter::is_opened check failed for '{}': {}", cam_name, e)))?;
            if !opened_writer {
                return Err(AppError::OpenCV(format!("Failed to open VideoWriter for '{}' at path '{}'", cam_name, output_path_clone.display())));
            }
            info!("OpenCV (blocking): VideoWriter opened for '{}' to {}", cam_name, output_path_clone.display());

            let start_time = std::time::Instant::now();
            let mut frame_count = 0;
            let mut last_error_log_time = std::time::Instant::now();

            while start_time.elapsed() < duration {
                let mut frame = opencv_core::Mat::default();
                if !cap.read(&mut frame).map_err(|e| AppError::OpenCV(format!("Frame read error mid-stream for '{}': {}", cam_name, e)))? {
                    // Log this only periodically to avoid spamming if the stream is truly down.
                    if last_error_log_time.elapsed().as_secs() > 5 {
                        error!("Camera '{}': Failed to read frame or stream ended prematurely.", cam_name);
                        last_error_log_time = std::time::Instant::now();
                    }
                    // Decide if we should break or try to re-establish. For now, break.
                    break; 
                }
                if frame.empty() {
                     if last_error_log_time.elapsed().as_secs() > 5 {
                        warn!("Camera '{}': Captured empty frame mid-stream.", cam_name);
                        last_error_log_time = std::time::Instant::now();
                    }
                    continue; // Skip empty frame
                }

                writer.write(&frame).map_err(|e| AppError::OpenCV(format!("VideoWriter write failed for '{}': {}", cam_name, e)))?;
                frame_count += 1;
            }
            info!("OpenCV (blocking): Finished recording for '{}'. Recorded {} frames.", cam_name, frame_count);

            writer.release().map_err(|e| AppError::OpenCV(format!("VideoWriter release failed for '{}': {}", cam_name, e)))?;
            cap.release().map_err(|e| AppError::OpenCV(format!("VideoCapture release failed for '{}': {}", cam_name, e)))?;
            
            if frame_count == 0 {
                warn!("Camera '{}': Video recording resulted in 0 frames. The output file might be empty or invalid.", cam_name);
                // Optionally delete the empty file: std::fs::remove_file(&output_path_clone)?;
            }

            Ok(output_path_clone)
        }).await.map_err(|e| AppError::Task(format!("OpenCV video recording task failed for '{}': {}", cam_name, e)))??;

        camera_entity.update_state(CameraState::Connected); // Or Idle
        Ok(record_result)
    }
} 