use crate::camera_config::CameraConfig;
use crate::errors::AppError;
use std::env;
use log::{info, warn};

#[derive(Debug, Clone)]
pub enum CameraState {
    Idle,
    Connecting,
    Connected,
    Streaming,
    Recording,
    Error(String),
    Disabled,
}

#[derive(Debug, Clone)]
pub struct CameraEntity {
    pub config: CameraConfig,
    pub state: CameraState,
    password: Option<String>,
}

impl CameraEntity {
    pub fn new(config: CameraConfig) -> Self {
        let mut entity = CameraEntity {
            config,
            state: CameraState::Idle,
            password: None,
        };
        entity.load_password();
        entity
    }

    fn load_password(&mut self) {
        let env_var_name = format!("{}_PASSWORD", self.config.name.to_uppercase().replace("-", "_"));
        match env::var(&env_var_name) {
            Ok(pass) => self.password = Some(pass),
            Err(_) => warn!(
                "Password not found in environment variable '{}' for camera '{}'",
                env_var_name,
                self.config.name
            ),
        }
    }

    pub fn get_password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    pub fn update_state(&mut self, new_state: CameraState) {
        info!("Camera '{}' state changed from {:?} to {:?}", self.config.name, self.state, new_state);
        self.state = new_state;
    }

    pub fn get_rtsp_url(&self) -> Result<String, AppError> {
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
                    format!("/ {}", override_path) // Ensure leading slash
                };
                Ok(format!("{}{}", base_url, path))
            } else {
                // If no override, an error might be more appropriate now, or a very generic default.
                // For now, let's assume an override is usually provided or a common known path exists.
                // This path is an example and might not work for all cameras.
                warn!("RTSP path override not set for camera '{}', using a generic default path. This might fail.", self.config.name);
                Ok(format!("{}/cam/realmonitor?channel=1&subtype=0", base_url)) 
            }
        } else {
            Err(AppError::Authentication {
                camera_name: self.config.name.clone(),
                details: "Password not available for RTSP URL construction".to_string(),
            })
        }
    }
} 