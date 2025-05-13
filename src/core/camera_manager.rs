use crate::camera::camera_entity::CameraEntity;
use crate::config_loader::MasterConfig;
use crate::errors::AppError;
use log::{info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;


pub struct CameraManager {
    cameras: HashMap<String, Arc<Mutex<CameraEntity>>>,
}

impl CameraManager {
    pub fn new(master_config: &MasterConfig) -> Result<Self, AppError> {
        let mut cameras = HashMap::new();
        for cam_config in &master_config.cameras {
            if cameras.contains_key(&cam_config.name) {
                return Err(AppError::Config(format!(
                    "Duplicate camera name found: {}",
                    cam_config.name
                )));
            }
            let camera_entity = CameraEntity::new(cam_config.clone());
            cameras.insert(cam_config.name.clone(), Arc::new(Mutex::new(camera_entity)));
        }
        info!("CameraManager initialized with {} cameras.", cameras.len());
        Ok(CameraManager {
            cameras,
            // app_config: Arc::new(master_config.app_settings.clone()), // Assuming ApplicationConfig is Clone
        })
    }

    pub async fn get_camera(&self, name: &str) -> Option<Arc<Mutex<CameraEntity>>> {
        self.cameras.get(name).cloned()
    }

    pub async fn get_all_cameras(&self) -> Vec<Arc<Mutex<CameraEntity>>> {
        self.cameras.values().cloned().collect()
    }

    pub async fn get_cameras_by_names(&self, names: &[String]) -> Vec<Arc<Mutex<CameraEntity>>> {
        let mut result = Vec::new();
        for name in names {
            if let Some(cam) = self.cameras.get(name) {
                result.push(cam.clone());
            }
        }
        result
    }

}

// Helper to parse comma-separated camera names from CLI
pub fn parse_camera_names_arg(names_str_opt: Option<&String>) -> Option<Vec<String>> {
    names_str_opt.map(|names_str| {
        names_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    })
} 