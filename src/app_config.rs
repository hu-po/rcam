use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationConfig {
    pub output_directory_base: String,
    pub default_config_path: String,
    pub image_format: String, // e.g., "jpg", "png"
    pub jpeg_quality: Option<u8>, // JPEG quality (0-100)
    pub png_compression: Option<u8>, // PNG compression level (0-9 for zopfli/libdeflate, or specific to encoder)
    pub video_format: String, // Container, e.g., "mp4", "mkv"
    pub video_codec: String,  // e.g., "h264", "mjpeg", "copy"
    pub video_fps: f32,
    pub video_duration_default_seconds: u32,
    pub filename_timestamp_format: String, // strftime format string
    pub time_sync_tolerance_seconds: f32,
    pub log_level: Option<String>, // Making it optional to potentially use CLI or env var as primary
    pub cgi_time_path: String,
    pub rerun_flush_timeout_secs: Option<f32>,
    pub rerun_memory_limit: Option<String>, // e.g., "50%", "2GB"
    pub rerun_drop_at_latency: Option<String>, // e.g., "100ms", "1s"
    pub realsense_api_version: Option<String>, // Target librealsense version
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        ApplicationConfig {
            output_directory_base: "./output".to_string(),
            default_config_path: "config/tatbot.yaml".to_string(), // Added default
            image_format: "jpg".to_string(),
            jpeg_quality: Some(95), // Default JPEG quality
            png_compression: Some(3), // Default PNG compression
            video_format: "mp4".to_string(),
            video_codec: "copy".to_string(),
            video_fps: 25.0,
            video_duration_default_seconds: 300,
            filename_timestamp_format: "%Yy%mm%dd%Hh%Mm%Ss".to_string(),
            time_sync_tolerance_seconds: 5.0,
            log_level: Some("info".to_string()),
            cgi_time_path: "/cgi-bin/global.cgi?action=getCurrentTime".to_string(),
            rerun_flush_timeout_secs: Some(10.0),
            rerun_memory_limit: Some("90%".to_string()), // Default Rerun memory limit
            rerun_drop_at_latency: Some("500ms".to_string()), // Default: no drop-at-latency
            realsense_api_version: None, // Default to None
        }
    }
} 