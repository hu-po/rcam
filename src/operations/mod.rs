pub mod image_capture_op;
pub mod video_record_op;
pub mod time_sync_op;
pub mod camera_control_op;
pub mod diagnostic_op;

// You might re-export functions if they are directly called from main or other top-level modules
// e.g., pub use image_capture_op::handle_capture_image_cli; 