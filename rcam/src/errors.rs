use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("Network Error: {0}")]
    Network(String),

    #[error("RTSP Error: {0}")]
    Rtsp(String),

    #[error("HTTP CGI Error: {0}")]
    HttpCgi(String),

    #[error("File I/O Error: {0}")]
    Io(String),

    #[error("Media Processing Error: {0}")]
    Media(String),

    #[error("Camera Control Error: {0}")]
    Control(String),
    
    #[error("Operation Error: {0}")]
    Operation(String),

    #[error("Task Execution Error: {0}")]
    Task(String),

    #[error("Resource Not Found: {0}")]
    NotFound(String),

    #[error("Authentication Failed for camera {camera_name}: {details}")]
    Authentication { camera_name: String, details: String },

    #[error("OpenCV Error: {0}")]
    OpenCV(String),

    #[error("An unexpected error occurred: {0}")]
    Unexpected(String),
}

// Allow conversion from std::io::Error to AppError::Io
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}