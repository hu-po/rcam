use crate::camera::camera_entity::CameraEntity;
use crate::config_loader::MasterConfig;
use anyhow::{Result, bail};
use log::{info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;


pub struct CameraManager {
    cameras: HashMap<String, Arc<Mutex<CameraEntity>>>,
}

impl CameraManager {
    pub fn new(master_config: &MasterConfig) -> Result<Self> {
        let mut cameras = HashMap::new();
        for cam_config in &master_config.cameras {
            if cameras.contains_key(&cam_config.name) {
                bail!("Duplicate camera name found in configuration: {}", cam_config.name);
            }
            let camera_entity = CameraEntity::new(cam_config.clone());
            cameras.insert(cam_config.name.clone(), Arc::new(Mutex::new(camera_entity)));
        }
        info!("CameraManager initialized with {} cameras.", cameras.len());
        Ok(CameraManager {
            cameras,
        })
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