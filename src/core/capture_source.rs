use anyhow::Result;
use std::path::{Path, PathBuf};
use async_trait::async_trait;

// --- Data structures for frame information ---

#[derive(Debug, Clone)]
pub struct RsColorFrameData {
    pub rgb_data: Vec<u8>,    // Raw RGB8 data
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct RsDepthFrameData {
    pub depth_data: Vec<u16>, // Raw Z16 depth data
    pub depth_units: f32,     // Depth units in meters per step
    pub width: u32,
    pub height: u32,
}

// Enum to hold different types of image data results from a capture operation
#[derive(Debug, Clone)]
pub enum FrameData {
    IpCameraImage {
        name: String, // Name of the camera that produced this image
        path: PathBuf, // Path to the saved image file
        format: String, // Image format, e.g., "jpg", "png"
    },
    RealsenseFrames {
        name: String, // Name of the Realsense device
        color_frame: Option<RsColorFrameData>,
        depth_frame: Option<RsDepthFrameData>,
    },
    // Could add other types like Thermal, etc. in the future
}

// A bundle that can contain multiple FrameData, e.g., color and depth from one Realsense
#[derive(Debug, Clone)]
pub struct FrameDataBundle {
    pub frames: Vec<FrameData>, // For a single Realsense, this might contain one RealsenseFrames variant
                                // For an IP camera, it would contain one IpCameraImage variant
}

// --- The CaptureSource Trait ---

#[async_trait]
pub trait CaptureSource {
    fn get_name(&self) -> String;
    fn get_type(&self) -> String; // e.g., "ip-camera", "realsense-camera"

    // Captures one or more images (e.g., color and depth for Realsense)
    // Saves them to the output_dir with filenames derived from timestamp_str
    // Returns a bundle of FrameData describing what was captured and saved.
    async fn capture_image(
        &mut self, 
        output_dir: &Path, 
        timestamp_str: &str,
        image_format_config: &str, // e.g. "png" or "jpg" from app settings for IP cams
        jpeg_quality: Option<u8>,
        png_compression: Option<u32>,
    ) -> Result<FrameDataBundle>;

    // Future methods for video might look like:
    // async fn start_video_stream(&mut self, config: VideoStreamConfig) -> Result<()>;
    // async fn stop_video_stream(&mut self) -> Result<PathBuf>; // Returns path to saved video
    // fn get_stream_capabilities(&self) -> Vec<StreamProfile>;
} 