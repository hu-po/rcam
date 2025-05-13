use serde::Deserialize;
use std::fs;
use std::path::Path;
use crate::app_config::ApplicationConfig;
use crate::camera_config::CameraConfig;
use anyhow::{Result, Context, bail};
use std::collections::HashSet;
use std::net::IpAddr;
use log::{debug, info};
use std::time::Instant;

#[derive(Debug, Deserialize, Clone)] pub struct MasterConfig {
    #[serde(rename = "application")]
    pub app_settings: ApplicationConfig,
    pub cameras: Vec<CameraConfig>,
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
    if config.app_settings.output_directory.is_empty() {
        bail!("❌ Application output_directory cannot be empty.");
    }
    let output_path = Path::new(&config.app_settings.output_directory);
    if !config.app_settings.output_directory.starts_with("./") && 
       !config.app_settings.output_directory.starts_with("../") &&
       !output_path.exists() {
        debug!("Output directory \'{}\' does not exist. Attempting to create it.", config.app_settings.output_directory);
        fs::create_dir_all(output_path)
            .with_context(|| format!("Output directory \'{}\' is not writable or cannot be created 📂💥", config.app_settings.output_directory))?;
        info!("📁 Created output directory: {}", config.app_settings.output_directory);
    } else if output_path.exists() && !output_path.is_dir() {
        bail!("❌ Output directory \'{}\' exists but is not a directory.", config.app_settings.output_directory);
    }

    if config.app_settings.image_format.is_empty() {
        bail!("❌ Application image_format cannot be empty.");
    }
    if config.app_settings.video_format.is_empty() {
        bail!("❌ Application video_format cannot be empty.");
    }

    if config.cameras.is_empty() {
        bail!("❌ No cameras defined in the configuration. This might be intentional for some operations.");
    }

    let mut camera_names = HashSet::new();
    for (idx, camera) in config.cameras.iter().enumerate() {
        debug!("Validating camera #{}: {}", idx + 1, camera.name);
        if camera.name.is_empty() {
            bail!("❌ Camera name cannot be empty for camera #{}.", idx + 1);
        }
        if !camera_names.insert(&camera.name) {
            bail!("❌ Duplicate camera name found: {}", camera.name);
        }
        if camera.ip.is_empty() {
            bail!("❌ IP address for camera \'{}\' cannot be empty.", camera.name);
        }
        if camera.ip.parse::<IpAddr>().is_err() {
            bail!("❌ Invalid IP address format \'{}\' for camera \'{}\'.", camera.ip, camera.name);
        }
        if camera.username.is_empty() {
             bail!("❌ Username for camera \'{}\' cannot be empty.", camera.name);
        }
        debug!("Camera \'{}\' validated successfully.", camera.name);
    }
    info!("👍 Master configuration validated successfully in {:?}.", validation_start_time.elapsed());
    Ok(())
} 