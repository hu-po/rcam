use crate::app_config::ApplicationConfig;
// use crate::camera_config::CameraConfig as RawCameraConfig; // Renamed to avoid conflict if we have another CameraConfig
use crate::camera::camera_entity::CameraEntity;
use crate::camera::CameraConfig as DomainCameraConfig; // Aliasing for clarity
use crate::config_loader::MasterConfig;
use crate::errors::AppError;
use log::{info}; // Removed warn
use std::collections::HashMap;
use std::sync::Arc; // For sharing CameraManager or parts of it across tasks
use tokio::sync::Mutex; // For mutable access to camera entities


pub struct CameraManager {
    // Using Arc<Mutex<...>> for individual cameras to allow concurrent, independent operations.
    cameras: HashMap<String, Arc<Mutex<CameraEntity>>>,
    // app_config: Arc<ApplicationConfig>, // If global app settings are needed frequently
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

    // Example of a method that might orchestrate an action across multiple cameras
    // pub async fn check_all_camera_statuses(&self) {
    //     info!("Checking status for all cameras...");
    //     for (name, cam_mutex) in &self.cameras {
    //         let cam = cam_mutex.lock().await;
    //         info!("Camera '{}' current state: {:?}", name, cam.state);
    //         // Here you could add more detailed checks, like pinging or short connections
    //     }
    // }
}

// Helper to parse comma-separated camera names from CLI
pub fn parse_camera_names_arg(names_str_opt: Option<&String>) -> Option<Vec<String>> {
    names_str_opt.map(|names_str| {
        names_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    })
} 