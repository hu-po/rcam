use serde::Deserialize;
use std::fs;
use std::path::Path;
use crate::app_config::ApplicationConfig;
use crate::camera_config::CameraConfig;
use anyhow::{Result, Context, bail};
use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Debug, Deserialize)] pub struct MasterConfig {
    #[serde(rename = "application")]
    pub app_settings: ApplicationConfig,
    pub cameras: Vec<CameraConfig>,
}

pub fn load_config(path: &str) -> Result<MasterConfig> {
    let config_str = fs::read_to_string(path)
        .with_context(|| format!("Failed to read configuration file '{}'", path))?;
    
    let config: MasterConfig = serde_yaml::from_str(&config_str)
        .with_context(|| format!("Failed to parse YAML configuration from '{}'", path))?;

    validate_master_config(&config).with_context(|| "Master configuration validation failed")?;

    Ok(config)
}

fn validate_master_config(config: &MasterConfig) -> Result<()> {
    if config.app_settings.output_directory.is_empty() {
        bail!("Application output_directory cannot be empty.");
    }
    let output_path = Path::new(&config.app_settings.output_directory);
    if !config.app_settings.output_directory.starts_with("./") && 
       !config.app_settings.output_directory.starts_with("../") &&
       !output_path.exists() {
        fs::create_dir_all(output_path)
            .with_context(|| format!("Output directory '{}' is not writable or cannot be created", config.app_settings.output_directory))?;
    } else if output_path.exists() && !output_path.is_dir() {
        bail!("Output directory '{}' exists but is not a directory.", config.app_settings.output_directory);
    }

    if config.app_settings.image_format.is_empty() {
        bail!("Application image_format cannot be empty.");
    }
    if config.app_settings.video_format.is_empty() {
        bail!("Application video_format cannot be empty.");
    }

    if config.cameras.is_empty() {
        bail!("No cameras defined in the configuration.");
    }

    let mut camera_names = HashSet::new();
    for camera in &config.cameras {
        if camera.name.is_empty() {
            bail!("Camera name cannot be empty.");
        }
        if !camera_names.insert(&camera.name) {
            bail!("Duplicate camera name found: {}", camera.name);
        }
        if camera.ip.is_empty() {
            bail!("IP address for camera '{}' cannot be empty.", camera.name);
        }
        if camera.ip.parse::<IpAddr>().is_err() {
            bail!("Invalid IP address format '{}' for camera '{}'.", camera.ip, camera.name);
        }
        if camera.username.is_empty() {
             bail!("Username for camera '{}' cannot be empty.", camera.name);
        }
    }
    Ok(())
} 