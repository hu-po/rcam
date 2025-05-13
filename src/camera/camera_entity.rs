use crate::camera_config::CameraConfig;
use anyhow::{Result, bail};
use std::env;
use log::{warn, debug};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CameraEntity {
    pub config: CameraConfig,
    password: Option<String>,
}

impl CameraEntity {
    pub fn new(config: CameraConfig) -> Self {
        let start_time = Instant::now();
        debug!("Attempting to create CameraEntity for '{}'", config.name);
        let mut entity = Self {
            config: config.clone(),
            password: None,
        };
        entity.load_password();
        debug!("✅ CameraEntity for '{}' created in {:?}", entity.config.name, start_time.elapsed());
        entity
    }

    fn load_password(&mut self) {
        let env_var_name = format!("{}_PASSWORD", self.config.name.to_uppercase().replace("-", "_"));
        debug!("🔑 Attempting to load password for camera \'{}\' from env var: {}", self.config.name, env_var_name);
        let start_time = Instant::now();
        match env::var(&env_var_name) {
            Ok(pass) => {
                self.password = Some(pass);
                debug!("  Password loaded successfully for \'{}\' in {:?}.", self.config.name, start_time.elapsed());
            },
            Err(_) => warn!(
                "⚠️ Password not found in environment variable \'{}\' for camera \'{}\' (lookup took {:?})",
                env_var_name,
                self.config.name,
                start_time.elapsed()
            ),
        }
    }

    pub fn get_password(&self) -> Option<&str> {
        debug!("🔑 Requesting password for camera: {}", self.config.name);
        self.password.as_deref()
    }

    pub fn get_rtsp_url(&self) -> Result<String> {
        debug!("🔗 Generating RTSP URL for camera: {}", self.config.name);
        let start_time = Instant::now();
        if let Some(pass) = self.get_password() {
            let base_url = format!(
                "rtsp://{}:{}@{}",
                self.config.username,
                pass, 
                self.config.ip
            );

            if let Some(override_path) = &self.config.rtsp_path_override {
                let path = if override_path.starts_with('/') {
                    override_path.clone()
                } else {
                    format!("/{}", override_path.trim_start_matches('/').trim())
                };
                let url = format!("{}{}", base_url, path);
                debug!("  Generated RTSP URL with override: \'{}\' in {:?}", url, start_time.elapsed());
                Ok(url)
            } else {
                warn!(
                    "⚠️ RTSP path override not set for camera \'{}\', using a generic default path. This might fail.", 
                    self.config.name
                );
                let url = format!("{}/cam/realmonitor?channel=1&subtype=0", base_url);
                debug!("  Generated RTSP URL with default path: \'{}\' in {:?}", url, start_time.elapsed());
                Ok(url)
            }
        } else {
            bail!(
                "❌ Password not available for RTSP URL construction for camera \'{}\'. Ensure \'{}\' env var is set.", 
                self.config.name, 
                self.config.name.to_uppercase().replace("-", "_")
            );
        }
    }
} 