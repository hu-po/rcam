use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct CameraConfig {
    pub name: String,
    pub ip: String,
    pub username: String,
    // Password will be loaded from an environment variable, not stored here.
    // CAMERANAME_PASSWORD (e.g., CAM1_LIVINGROOM_PASSWORD)
    pub mac_address: Option<String>,
    pub rtsp_path_override: Option<String>, // e.g., /cam/realmonitor?channel=1&subtype=0
} 