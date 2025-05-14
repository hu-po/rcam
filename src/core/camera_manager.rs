use crate::config_loader::{MasterConfig, CaptureDeviceConfig};
use crate::core::capture_source::CaptureSource;
use crate::camera::ip_camera_device::IpCameraDevice;
use crate::camera::realsense_device::RealsenseDevice;
use anyhow::{Result, bail};
use log::{info, debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

pub struct CameraManager {
    // Stores different types of camera devices that implement the CaptureSource trait
    cameras: HashMap<String, Arc<Mutex<dyn CaptureSource + Send>>>,
}

impl CameraManager {
    pub fn new(master_config: &MasterConfig) -> Result<Self> {
        debug!("🛠️ Initializing CameraManager with new trait-based architecture...");
        let start_time = Instant::now();
        let mut cameras: HashMap<String, Arc<Mutex<dyn CaptureSource + Send>>> = HashMap::new();

        if master_config.cameras.is_empty() {
            warn!("CameraManager: No cameras defined in the configuration. Manager will be empty.");
        }

        for device_config in &master_config.cameras {
            let device_name = device_config.get_name().clone();
            debug!("  Processing device config for: '{}'", device_name);

            if cameras.contains_key(&device_name) {
                bail!("❌ Duplicate camera/device name found in configuration: {}", device_name);
            }

            let capture_source_device: Arc<Mutex<dyn CaptureSource + Send>> = match device_config {
                CaptureDeviceConfig::IpCamera { name, specifics } => {
                    info!("    Type: IP Camera. Creating IpCameraDevice for '{}' with IP {}", name, specifics.ip);
                    let ip_cam_device = IpCameraDevice::new(name.clone(), specifics.clone());
                    Arc::new(Mutex::new(ip_cam_device))
                }
                CaptureDeviceConfig::RealsenseCamera { name, specifics } => {
                    info!("    Type: Realsense Camera. Creating RealsenseDevice for '{}'. Serial: {:?}", 
                           name, specifics.serial_number.as_deref().unwrap_or("any"));
                    let rs_device = RealsenseDevice::new(name.clone(), specifics.clone());
                    Arc::new(Mutex::new(rs_device))
                }
            };
            
            cameras.insert(device_name.clone(), capture_source_device);
            debug!("  Added device '{}' to manager.", device_name);
        }

        info!(
            "✅ CameraManager initialized with {} device(s) ({:?}) in {:?}.",
            cameras.len(),
            cameras.keys().collect::<Vec<&String>>(), // Log names of initialized devices
            start_time.elapsed()
        );
        Ok(CameraManager { cameras })
    }

    pub async fn get_all_devices(&self) -> Vec<Arc<Mutex<dyn CaptureSource + Send>>> {
        debug!("📷 Retrieving all configured devices ({})", self.cameras.len());
        let start_time = Instant::now();
        let all_devices = self.cameras.values().cloned().collect();
        debug!("Retrieved all devices in {:?}", start_time.elapsed());
        all_devices
    }

    pub async fn get_devices_by_names(&self, names: &[String]) -> Vec<Arc<Mutex<dyn CaptureSource + Send>>> {
        debug!("📷 Retrieving devices by names: {:?}", names);
        let start_time = Instant::now();
        let mut result = Vec::new();
        for name in names {
            if let Some(device_arc) = self.cameras.get(name) {
                result.push(device_arc.clone());
                debug!("  Found device: {}", name);
            } else {
                warn!("  Device not found by name: '{}'", name); // Changed to warn as this might be an issue
            }
        }
        debug!("Retrieved {} devices by names in {:?}", result.len(), start_time.elapsed());
        result
    }
}