Project Overview
The goal is to reimplement the panoram.py Python script in Rust to capture images and videos from PoE IP cameras via RTSP streams, configured through a YAML file. The Rust version will improve performance, safety, and modularity while maintaining feature parity and adding fault tolerance.
System Requirements
Functional Requirements:
Configuration Loading:
Load camera details from $TATBOT_ROOT/config/cameras.yaml.

Fields: name, device_name, ip, mac, username, password (from environment variables, e.g., CAMERA_NAME_PASSWORD).

Validate IPs, required fields, and directory paths.

RTSP URL Generation:
Generate URLs: rtsp://username:password@ip:554/cam/realmonitor?channel=1&subtype=0.

Support configurable suffixes per camera.

Camera Time Synchronization:
Query camera time via HTTP GET (/cgi-bin/global.cgi?action=getCurrentTime).

Verify all cameras are within 5 seconds (configurable tolerance).

Camera Control:
Enable/disable streams via HTTP GET (/cgi-bin/configManager.cgi?action=setConfig&VideoInOptions[0].Enable={0|1}).

Use HTTP Digest Authentication.

Image Capture:
Capture a single image from an RTSP stream.

Save as JPEG with timestamped filename (e.g., 2025y05m12d10h09m45s_multi_cam1.jpg).

Support configurable delay before capture.

Video Capture:
Record video for a specified duration (default: 5 seconds).

Save as AVI with MJPG codec at 30 FPS (configurable codec/FPS).

Use timestamped filename (e.g., 2025y05m12d10h09m45s_multi_cam1.avi).

Concurrent Operations:
Perform capture/control across multiple cameras simultaneously.

Handle up to 16 cameras (configurable limit).

Testing Mode:
Test time sync, single/multi image/video capture, and enable/disable.

Generate a summary report (console and optional JSON output).

CLI:
Support arguments: --mode (image/video), --duration, --output, --control (enable/disable), --test, --delay, --debug.

Use subcommands (e.g., capture image, control enable) for clarity.

Logging:
Log info, debug, and error messages with timestamps.

Support --debug flag for verbose output.

Non-Functional Requirements:
Performance: Handle 16 cameras with minimal latency (<1s for image capture, <10s for 5s video).

Reliability: Retry failed RTSP/HTTP requests (3 attempts, configurable).

Safety: Prevent credential leaks; validate inputs to avoid injection.

Portability: Run on Linux/Windows with minimal setup (Rust, OpenCV dependencies).

Maintainability: Modular code with clear documentation and tests.

Extensibility: Support pluggable capture backends (OpenCV/FFmpeg).

System Design
Architecture:
Modules:
config: Handles YAML loading, validation, and RTSP URL generation.

camera_ops: Manages HTTP-based operations (time sync, control).

capture: Handles image/video capture and file saving.

cli: Parses arguments and dispatches tasks.

errors: Defines custom error types.

tests: Implements test suite with mocks.

Dependencies:
serde, serde_yaml: Configuration parsing.

reqwest: HTTP requests with Digest Authentication.

opencv: RTSP stream capture and file saving.

tokio: Async runtime and concurrency.

clap: CLI parsing.

log, env_logger: Logging.

chrono: Timestamp handling.

wiremock (dev): Mocking for tests.

Concurrency Model:
Use tokio for async tasks (HTTP requests, coordination).

Run OpenCV capture in tokio::task::spawn_blocking to avoid blocking the async runtime.

Limit concurrent tasks with tokio::sync::Semaphore (default: 16 workers).

Error Handling:
Define enum Error { Config, Network, Capture, ... } with thiserror for descriptive errors.

Use Result for all fallible operations.

Log errors with context; exit with status code 1 on fatal errors.

Capture Backend:
Implement a CaptureBackend trait:
rust

trait CaptureBackend {
    fn capture_image(&self, config: &PanoramConfig, camera: &Camera) -> Result<String, Error>;
    fn capture_video(&self, config: &PanoramConfig, camera: &Camera, duration: i32) -> Result<String, Error>;
}

Default implementation: OpenCvBackend using opencv crate.

Future support: FfmpegBackend for alternative handling.

Data Flow:
Startup: Load YAML config, validate, and fetch passwords from environment.

CLI Parsing: Use clap to parse arguments and dispatch to modes (capture, control, test).

Camera Operations:
Time sync: Async HTTP requests with reqwest, parse responses with chrono.

Control: Async HTTP requests with retry logic.

Capture:
Image: Open RTSP stream, capture frame, save as JPEG.

Video: Open RTSP stream, record frames for duration, save as AVI.

Run captures concurrently with tokio::spawn and spawn_blocking.

Output: Save files to $TATBOT_ROOT/output/panoram (or custom path), print file paths to stdout.

Testing: Run async tests, mock HTTP/RTSP with wiremock, output summary.

Class Diagram (Simplified):
rust

struct PanoramConfig {
    image_format: String,
    video_format: String,
    video_codec: String,
    video_fps: f32,
    video_duration: i32,
    rtsp_url_suffix: String,
    output_dir: String,
    cameras: HashMap<String, Camera>,
}

struct Camera {
    name: String,
    device_name: String,
    ip: String,
    mac: String,
    username: String,
    password: Option<String>,
}

struct OpenCvBackend;

impl CaptureBackend for OpenCvBackend { ... }

Sequence Diagram (Image Capture):
CLI -> main: Parse --mode image.

main -> config: Load PanoramConfig.

main -> capture: Call async_capture_all_images.

capture -> tokio: Spawn tasks for each camera.

Task -> OpenCvBackend: Call capture_image in spawn_blocking.

OpenCvBackend -> OpenCV: Open RTSP stream, capture frame, save JPEG.

capture -> main: Collect file paths, print to stdout.

Implementation Notes
RTSP Fault Tolerance:
Set OPENCV_FFMPEG_CAPTURE_OPTIONS=rtsp_transport;tcp for reliable streaming.

Implement retry logic (3 attempts, 1s backoff) for stream failures.

Performance:
Monitor OpenCV’s memory usage with multiple streams; adjust Semaphore limit if needed.

Cache RTSP connections if OpenCV supports it.

Testing:
Use wiremock to mock HTTP endpoints (/cgi-bin/global.cgi, /cgi-bin/configManager.cgi).

Mock RTSP streams with a local server or test files if feasible.

Security:
Use secrecy crate for password handling to prevent accidental leaks.

Sanitize RTSP URLs to prevent injection.

Deliverables
Rust source code in a modular structure (src/config.rs, src/camera_ops.rs, etc.).

CLI binary (panoram) with subcommands.

Test suite covering configuration, HTTP operations, and capture.

Documentation (README.md, inline comments).

Dependency setup guide for OpenCV (libopencv-dev on Linux, vcpkg on Windows).

Risks and Mitigation
Risk: OpenCV latency with multiple streams.
Mitigation: Test with 16 cameras early; fallback to FFmpeg if needed.

Risk: Camera-specific RTSP quirks.
Mitigation: Add configurable RTSP options (e.g., TCP/UDP, custom suffixes).

Risk: Dependency setup complexity.
Mitigation: Provide clear setup instructions and Docker support.

Conclusion
The final design retains the OpenCV-based approach for compatibility with panoram.py while incorporating lessons from existing projects like Retina (fault tolerance), FFmpeg (robustness), and community discussions (async concurrency). By using tokio, reqwest, and clap, we leverage Rust’s strengths for performance and safety. The pluggable capture backend and retry logic enhance extensibility and reliability. This design balances feature parity with improvements, making it suitable for capturing images/videos from multiple IP cameras.
If you’d like, I can provide a detailed code skeleton for specific modules or help set up a project repository with dependencies. Let me know your next steps!

