use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs;
use std::path::Path;
use anyhow::{Result, Context, bail};
use std::collections::HashSet;
use std::net::IpAddr;
use log::{debug, info};
use std::time::Instant;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppSettings {
    pub output_directory_base: String,
    pub default_config_path: String, 
    pub filename_timestamp_format: String,
    pub image_format: String,
    pub jpeg_quality: Option<u8>,
    pub png_compression: Option<u32>,
    pub video_format: String, 
    pub video_codec: String,  
    pub video_fps: Option<f32>,
    pub video_duration_default_seconds: u32,
    pub time_sync_tolerance_seconds: Option<f32>,
    pub log_level: Option<String>,
    pub enable_gui: Option<bool>,
    pub rerun_flush_timeout_secs: Option<f32>,
    pub rerun_memory_limit: Option<String>,
    pub rerun_drop_at_latency: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpCameraSpecificConfig {
    pub ip: String,
    pub username: Option<String>,
    pub http_port: Option<u16>,
    pub rtsp_port: Option<u16>,
    pub rtsp_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RealsenseSpecificConfig {
    pub serial_number: Option<String>,
    pub color_width: Option<u32>,
    pub color_height: Option<u32>,
    pub color_fps: Option<u32>,
    pub depth_width: Option<u32>,
    pub depth_height: Option<u32>,
    pub depth_fps: Option<u32>,
    pub enable_color_stream: Option<bool>,
    pub enable_depth_stream: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CaptureDeviceConfig {
    IpCamera {
        name: String,
        #[serde(flatten)]
        specifics: IpCameraSpecificConfig,
    },
    RealsenseCamera {
        name: String,
        #[serde(flatten)]
        specifics: RealsenseSpecificConfig,
    },
}

impl CaptureDeviceConfig {
    pub fn get_name(&self) -> &String {
        match self {
            CaptureDeviceConfig::IpCamera { name, .. } => name,
            CaptureDeviceConfig::RealsenseCamera { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MasterConfig {
    pub application: AppSettings,
    pub cameras: Vec<CaptureDeviceConfig>, // Now a list of different device types
}

pub fn load_config(path: &str) -> Result<MasterConfig> {
    debug!("📄 Attempting to load config from: {}", path);
    let start_time = Instant::now();

    let config_str = fs::read_to_string(path)
        .with_context(|| format!("Failed to read configuration file \'{}\'. 📖", path))?;
    debug!("Read config file in {:?}", start_time.elapsed());
    
    let parse_start_time = Instant::now();
    let config: MasterConfig = serde_yaml::from_str(&config_str)
        .with_context(|| format!("Failed to parse YAML configuration from \'{}\'. 💔", path))?;
    debug!("Parsed YAML in {:?}", parse_start_time.elapsed());

    let validate_start_time = Instant::now();
    validate_master_config(&config).with_context(|| "Master configuration validation failed 👎")?;
    debug!("Validated master config in {:?}", validate_start_time.elapsed());

    info!("✅ Successfully loaded and validated configuration from \'{}\' in {:?}", path, start_time.elapsed());
    Ok(config)
}

fn validate_master_config(config: &MasterConfig) -> Result<()> {
    debug!("🕵️ Validating master configuration...");
    let validation_start_time = Instant::now();
    if config.application.output_directory_base.is_empty() {
        bail!("❌ Application output_directory_base cannot be empty.");
    }

    let output_dir_path = Path::new(&config.application.output_directory_base);

    if !config.application.output_directory_base.starts_with("./") && 
       !config.application.output_directory_base.starts_with("../") &&
       !output_dir_path.exists() {
        debug!("Output directory '{}' does not exist (and matches specific path criteria). Attempting to create it.", config.application.output_directory_base);
        fs::create_dir_all(output_dir_path)
            .with_context(|| format!("Output directory '{}' is not writable or cannot be created 📂💥", config.application.output_directory_base))?;
        info!("📁 Created output directory: {}", config.application.output_directory_base);
    } else if output_dir_path.exists() && !output_dir_path.is_dir() {
        bail!("❌ Output directory '{}' exists but is not a directory.", config.application.output_directory_base);
    }

    if config.application.image_format.is_empty() {
        bail!("❌ Application image_format cannot be empty.");
    }
    if config.application.video_format.is_empty() {
        bail!("❌ Application video_format cannot be empty.");
    }

    if config.cameras.is_empty() {
        bail!("❌ No cameras defined in the configuration. This might be intentional for some operations.");
    }

    let mut camera_names = HashSet::new();
    for (idx, camera) in config.cameras.iter().enumerate() {
        debug!("Validating camera #{}: {}", idx + 1, camera.get_name());
        if camera.get_name().is_empty() {
            bail!("❌ Camera name cannot be empty for camera #{}.", idx + 1);
        }
        if !camera_names.insert(camera.get_name()) {
            bail!("❌ Duplicate camera name found: {}", camera.get_name());
        }

        match camera {
            CaptureDeviceConfig::IpCamera { name, specifics } => {
                if specifics.ip.is_empty() {
                    bail!("❌ IP address for camera '{}' cannot be empty.", name);
                }
                if specifics.ip.parse::<IpAddr>().is_err() {
                    bail!("❌ Invalid IP address format '{}' for camera '{}'.", specifics.ip, name);
                }
                // Username is optional for IpCamera, but if it's None and a password env var exists,
                // it might be an issue for some auth. The warning is in load_master_config.
                // Here, we could choose to enforce it if desired, but current logic makes it optional.
                // For now, only validating it's not empty *if* it was intended to be there but parsing failed (which serde would catch).
                // The load_master_config already warns if username is None.
            }
            CaptureDeviceConfig::RealsenseCamera { name, specifics } => {
                // Add any Realsense specific validations here if needed.
                // For example, check if resolution/fps values are within supported ranges if known.
                debug!("Realsense camera '{}' (Serial: {:?}) specific config validated (currently no specific checks).", name, specifics.serial_number);
            }
        }
        debug!("Camera '{}' validated successfully.", camera.get_name());
    }
    info!("👍 Master configuration validated successfully in {:?}.", validation_start_time.elapsed());
    Ok(())
} 