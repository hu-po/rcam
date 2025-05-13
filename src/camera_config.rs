use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct CameraConfig {
    pub name: String,
    pub ip: String,
    pub username: String,
    pub rtsp_path_override: Option<String>, // e.g., /cam/realmonitor?channel=1&subtype=0
} 