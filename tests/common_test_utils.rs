// rcam/tests/common_test_utils.rs

// This file can contain shared helper functions, mock objects, or setup/teardown
// logic for your integration and unit tests.

// Example: Mock Camera Configuration
/*
use rcam::camera_config::{CameraConfig, CameraInterfacePreference};

pub fn mock_camera_config(name: &str, ip: &str) -> CameraConfig {
    CameraConfig {
        name: name.to_string(),
        ip: ip.to_string(),
        username: "testuser".to_string(),
        mac_address: None,
        interface_preference: CameraInterfacePreference::default(),
        rtsp_path_override: Some("/test/stream").to_string(),
    }
}
*/

// Example: Mock HTTP server setup using wiremock (if you add wiremock to dev-dependencies)
/*
use wiremock::MockServer;

pub async fn start_mock_camera_api_server() -> MockServer {
    let server = MockServer::start().await;
    // Setup common mock responses here if needed
    // e.g., server.register( ... ).await;
    server
}
*/

// Add more utilities as your testing needs grow. 