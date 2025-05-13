use crate::camera::camera_entity::CameraEntity;
use crate::config_loader::MasterConfig;
use anyhow::{Result, bail};
use log::{info, debug};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;


pub struct CameraManager {
    cameras: HashMap<String, Arc<Mutex<CameraEntity>>>,
}

impl CameraManager {
    pub fn new(master_config: &MasterConfig) -> Result<Self> {
        debug!("🛠️ Initializing CameraManager...");
        let start_time = Instant::now();
        let mut cameras = HashMap::new();
        for (idx, cam_config) in master_config.cameras.iter().enumerate() {
            debug!("  Processing camera config #{}: {}", idx + 1, cam_config.name);
            if cameras.contains_key(&cam_config.name) {
                bail!("❌ Duplicate camera name found in configuration: {}", cam_config.name);
            }
            let camera_entity = CameraEntity::new(cam_config.clone());
            cameras.insert(cam_config.name.clone(), Arc::new(Mutex::new(camera_entity)));
            debug!("  Added camera '{}' to manager.", cam_config.name);
        }
        info!("✅ CameraManager initialized with {} cameras in {:?}.", cameras.len(), start_time.elapsed());
        Ok(CameraManager {
            cameras,
        })
    }

    pub async fn get_all_cameras(&self) -> Vec<Arc<Mutex<CameraEntity>>> {
        debug!("📷 Retrieving all configured cameras ({})", self.cameras.len());
        let start_time = Instant::now();
        let all_cams = self.cameras.values().cloned().collect();
        debug!("Retrieved all cameras in {:?}", start_time.elapsed());
        all_cams
    }

    pub async fn get_cameras_by_names(&self, names: &[String]) -> Vec<Arc<Mutex<CameraEntity>>> {
        debug!("📷 Retrieving cameras by names: {:?}", names);
        let start_time = Instant::now();
        let mut result = Vec::new();
        for name in names {
            if let Some(cam) = self.cameras.get(name) {
                result.push(cam.clone());
                debug!("  Found camera: {}", name);
            } else {
                debug!("  Camera not found: {}", name);
            }
        }
        debug!("Retrieved {} cameras by name in {:?}", result.len(), start_time.elapsed());
        result
    }

}

// Helper to parse comma-separated camera names from CLI
pub fn parse_camera_names_arg(names_str_opt: Option<&String>) -> Option<Vec<String>> {
    debug!("📝 Parsing camera names argument: {:?}", names_str_opt);
    let start_time = Instant::now();
    let result = names_str_opt.map(|names_str| {
        names_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    });
    debug!("Parsed camera names: {:?} in {:?}", result, start_time.elapsed());
    result
} 