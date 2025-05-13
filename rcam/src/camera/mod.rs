pub mod camera_entity;
pub mod camera_controller;
pub mod camera_media;

// Re-export key structs/enums for easier access from outside the camera module
pub use camera_entity::CameraEntity;
pub use camera_controller::CameraController; // Or specific functions/structs from it
pub use camera_media::CameraMediaManager;   // Or specific functions/structs from it 