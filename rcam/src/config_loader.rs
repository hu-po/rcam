use serde::Deserialize;
use std::fs;
use std::path::Path;
use crate::app_config::ApplicationConfig;
use crate::camera_config::CameraConfig;
use crate::errors::AppError;
use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Debug, Deserialize)] pub struct MasterConfig {
    #[serde(rename = "application")]
    pub app_settings: ApplicationConfig,
    pub cameras: Vec<CameraConfig>,
}

pub fn load_config(path: &str) -> Result<MasterConfig, AppError> {
    let config_str = fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("Failed to read configuration file '{}': {}", path, e)))?;
    
    let config: MasterConfig = serde_yaml::from_str(&config_str)
        .map_err(|e| AppError::Config(format!("Failed to parse YAML configuration: {}", e)))?;

    validate_master_config(&config)?;

    Ok(config)
}

fn validate_master_config(config: &MasterConfig) -> Result<(), AppError> {
    // Validate application settings
    if config.app_settings.output_directory.is_empty() {
        return Err(AppError::Config("Application output_directory cannot be empty.".to_string()));
    }
    // Attempt to check writability only if the path is not clearly a placeholder like "."
    // More robust checks might involve trying to create a temporary file.
    let output_path = Path::new(&config.app_settings.output_directory);
    if !config.app_settings.output_directory.starts_with("./") && // common relative placeholder
       !config.app_settings.output_directory.starts_with("../") &&
       !output_path.exists() {
        // If it doesn't exist and isn't a simple relative path, try to create it.
        // This also checks writability to the parent for creation.
        fs::create_dir_all(output_path).map_err(|e| AppError::Config(format!("Output directory '{}' is not writable or cannot be created: {}", config.app_settings.output_directory, e)))?;
    } else if output_path.exists() && !output_path.is_dir() {
        return Err(AppError::Config(format!("Output directory '{}' exists but is not a directory.", config.app_settings.output_directory)));
    }
    // Further writability checks if it exists could be added, e.g., trying to create a temp file.

    if config.app_settings.image_format.is_empty() {
        return Err(AppError::Config("Application image_format cannot be empty.".to_string()));
    }
    if config.app_settings.video_format.is_empty() {
        return Err(AppError::Config("Application video_format cannot be empty.".to_string()));
    }
    // Add more checks for app_settings as needed (e.g., sensible FPS, duration)

    // Validate cameras
    if config.cameras.is_empty() {
        return Err(AppError::Config("No cameras defined in the configuration.".to_string()));
    }

    let mut camera_names = HashSet::new();
    for camera in &config.cameras {
        if camera.name.is_empty() {
            return Err(AppError::Config("Camera name cannot be empty.".to_string()));
        }
        if !camera_names.insert(&camera.name) {
            return Err(AppError::Config(format!("Duplicate camera name found: {}", camera.name)));
        }
        if camera.ip.is_empty() {
            return Err(AppError::Config(format!("IP address for camera '{}' cannot be empty.", camera.name)));
        }
        // Validate IP address format
        if camera.ip.parse::<IpAddr>().is_err() {
            return Err(AppError::Config(format!("Invalid IP address format '{}' for camera '{}'.", camera.ip, camera.name)));
        }
        if camera.username.is_empty() {
             return Err(AppError::Config(format!("Username for camera '{}' cannot be empty.", camera.name)));
        }
        // Password presence is checked at runtime when CameraEntity loads it from env vars.
        // rtsp_path_override is optional, so no check here unless we want to validate its format if present.
    }
    Ok(())
}

// Example validation function (to be expanded)
#[allow(dead_code)]
fn validate_config(config: &MasterConfig) -> Result<(), AppError> {
    // Validate application settings
    if config.app_settings.output_directory.is_empty() {
        return Err(AppError::Config("Application output_directory cannot be empty.".to_string()));
    }

    // Validate each camera
    for camera in &config.cameras {
        if camera.name.is_empty() {
            return Err(AppError::Config("Camera name cannot be empty.".to_string()));
        }
        if camera.ip.is_empty() { // A more robust IP validation is needed
            return Err(AppError::Config(format!("IP address for camera '{}' cannot be empty.", camera.name)));
        }
        // TODO: Validate RTSP path, credentials presence (though not value here)
    }
    Ok(())
} 