// pub mod camera_config; // Removed, as camera_config.rs is at src/ level, not src/camera/
pub mod camera_controller;
pub mod camera_entity;
pub mod camera_media;

// pub use camera_entity::CameraEntity; // Marked as unused
// pub use camera_controller::CameraController; // Marked as unused
// pub use camera_media::CameraMediaManager;   // Marked as unused
pub use crate::camera_config::CameraConfig; // Changed to use crate:: path

// Re-export key structs/enums for easier access from outside the camera module
// pub use camera_entity::CameraEntity;
// pub use camera_controller::CameraController; // Or specific functions/structs from it
// pub use camera_media::CameraMediaManager;   // Or specific functions/structs from it 