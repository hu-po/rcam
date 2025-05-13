Rust Panoram: System Design & Requirements Document
Version: 1.0
Date: 2025-05-12

1. Introduction
This document outlines the requirements and system design for rust-panoram, a Rust application intended to replace and enhance the functionality of the Python panoram.py script. The primary goal is to record images and videos concurrently from multiple network IP cameras configured via a YAML file, leveraging Rust's strengths in performance, safety, and concurrency.

2. Goals
Replicate the core functionality of panoram.py: concurrent image/video capture from multiple RTSP cameras defined in a config file.

Improve robustness, performance, and error handling compared to the Python version.

Leverage modern Rust practices and libraries, particularly for async operations and media handling.

Introduce standardized camera interaction where possible (e.g., ONVIF).

Provide a clear Command-Line Interface (CLI) for various operations.

3. Core Feature Requirements
3.1. Configuration Management
Source: Load configuration from a YAML file (config.yaml or specified via CLI).

Camera Definition: Define cameras with attributes:

name: Unique identifier (String).

ip: IP address or hostname (String).

username: Camera login username (String).

mac_address: (Optional) MAC address (String).

interface_preference: (Optional) Enum/String (Onvif, HttpCgi, OnvifThenHttp, HttpThenOnvif) - Default: OnvifThenHttp. Specifies preferred method for control/info commands.

rtsp_path_override: (Optional) Full RTSP path (e.g., /cam/realmonitor?channel=1&subtype=0) if ONVIF discovery fails or is not preferred.

Credentials: Load camera passwords securely (e.g., from environment variables like CAMERANAME_PASSWORD, potentially support system keyring or dedicated secrets file in future). Passwords MUST NOT be stored in plain text in the main YAML.

Global Settings: Define application-wide defaults in YAML (overridable by CLI where applicable):

output_directory: Path for saving images/videos.

image_format: e.g., "jpg", "png".

video_format: Container, e.g., "mp4", "avi", "mkv".

video_codec: e.g., "h264", "mjpeg", "copy" (if supported by container/stream).

video_fps: Target recording FPS (float).

video_duration_default: Default recording duration in seconds (integer).

filename_timestamp_format: strftime format string.

rtsp_transport: Preferred RTSP transport (tcp, udp - default: tcp).

time_sync_tolerance_seconds: Tolerance for verify-time command (float).

Validation: Validate configuration on load (required fields, path writability).

3.2. Camera Interaction & Control
Stream URI Discovery:

Attempt to fetch RTSP stream URIs using ONVIF discovery/queries based on interface_preference.

Fall back to constructing the URL using rtsp_path_override or a default suffix if ONVIF fails or is not preferred.

Time Synchronization:

Implement get_camera_time function using ONVIF (preferred) or specific HTTP CGI endpoint (fallback).

Implement verify-all-times command to check sync across all cameras within tolerance.

Stream Toggle:

Implement enable-camera/disable-camera commands (for single or all cameras) using ONVIF (preferred) or specific HTTP CGI endpoint (fallback). Requires HTTP Digest Authentication for CGI.

3.3. Media Capture
RTSP Connection: Connect to camera RTSP streams concurrently using the determined URIs and transport preference.

Image Capture:

Capture a single frame from one or all cameras concurrently.

Save frames to the output directory using configured format and timestamp.

Support an optional delay (--delay) before capture.

Video Recording:

Record video segments of a specified duration (--duration or default) from one or all cameras concurrently.

Save segments to the output directory using configured format, codec, FPS, and timestamp.

Handle stream interruptions and empty recordings gracefully.

3.4. Command-Line Interface (CLI)
Provide subcommands for different actions:

capture-image: Capture images from all cameras (optional --delay).

capture-video: Capture videos from all cameras (optional --duration).

verify-times: Check time synchronization across all cameras.

control:

--action <enable|disable>

--cameras <all|name1,name2,...> (Default: all)

test: Run a diagnostic test suite (capture single image/video, toggle, multi-capture).

discover: (Optional Future Enhancement) Use ONVIF WS-Discovery to find cameras on the network.

Global Options:

--config <path>: Specify configuration file path.

--output <path>: Override output directory.

--debug / --verbose: Increase logging verbosity.

3.5. Logging & Error Handling
Implement structured logging (e.g., using tracing or log+env_logger).

Define custom, specific error types (using thiserror) for different failure modes (Config, Network, RTSP, ONVIF, HTTP, File I/O, Media Encoding/Decoding).

Provide clear error messages to the user.

4. Non-Functional Requirements
Performance: Efficiently handle multiple concurrent streams without excessive CPU/memory usage. Async runtime should not be a bottleneck.

Reliability: Robustly handle network errors, camera timeouts, stream disconnections, file system errors, and media processing issues. Avoid panics in recoverable situations.

Maintainability: Well-structured, modular codebase with clear separation of concerns. Idiomatic Rust code with comprehensive comments and documentation. Unit and integration tests.

Usability: Intuitive CLI, informative logging.

5. System Architecture & Design Choices
5.1. Core Libraries
Async Runtime: tokio (multi-threaded runtime).

CLI Parsing: clap.

Configuration: serde, serde_yaml.

Logging: tracing or log + env_logger.

Date/Time: chrono.

HTTP Client: reqwest (with digest-auth feature or manual implementation if needed).

ONVIF Client: onvif-rs, onvif_cam_rs, or similar maintained crate.

RTSP/Media Handling: gstreamer-rs (Primary Choice). Leverage GStreamer pipelines for:

RTSP connection (rtspsrc or potentially rtspsrc2 via gst-plugin-rtsp).

Decoding (e.g., avdec_h264, jpegdec).

Encoding (e.g., x264enc, jpegenc).

Muxing (e.g., mp4mux, avimux).

Saving (filesink).

Frame grabbing for images (appsink).

Requires GStreamer development libraries (>= 1.14 recommended) installed on the system.

Image Saving: image crate (for saving frames obtained via GStreamer appsink).

Error Handling: thiserror.

5.2. Concurrency Model
Utilize tokio::spawn to launch tasks for each concurrent operation (e.g., one task per camera for recording, one task per camera for image capture).

Use futures::future::join_all or similar to await completion of multiple tasks.

Employ tokio::sync::mpsc channels for communication if needed (e.g., status updates from long-running recording tasks).

Use tokio::spawn_blocking only if unavoidable blocking operations are identified within async tasks (GStreamer pipeline handling should generally be non-blocking if managed correctly via its bus).

5.3. Module Structure (Proposed)
rust-panoram/
├── Cargo.toml
├── config.yaml          # Example configuration
├── src/
│   ├── main.rs          # Entry point, CLI parsing, runtime setup
│   ├── cli.rs           # clap definitions
│   ├── config.rs        # Config structs, loading, validation
│   ├── error.rs         # Custom error enum (thiserror)
│   ├── camera/
│   │   ├── mod.rs
│   │   ├── manager.rs   # Orchestrates operations across cameras
│   │   ├── stream.rs    # GStreamer pipeline logic per camera
│   │   ├── control.rs   # ONVIF/HTTP interaction per camera
│   │   └── types.rs     # Camera struct, common types
│   ├── tasks/
│   │   ├── mod.rs
│   │   ├── capture_image.rs
│   │   ├── record_video.rs
│   │   ├── verify_time.rs
│   │   └── control_camera.rs
│   └── util.rs          # Logging setup, timestamp helpers
└── tests/                 # Integration tests

5.4. Key Workflows
Initialization: Parse CLI args -> Load/Validate Config -> Setup Logging -> Initialize Tokio Runtime -> Initialize GStreamer.

Capture All Images: Manager spawns an async_capture_image task for each camera -> Each task sets up a short-lived GStreamer pipeline (rtspsrc ! ... ! appsink) -> Grabs one buffer -> Saves using image crate -> Task completes -> join_all waits.

Record All Videos: Manager spawns an async_record_video task for each camera -> Each task sets up a GStreamer pipeline (rtspsrc ! ... ! encoder ! muxer ! filesink) -> Pipeline runs for specified duration (managed via GStreamer messages/timers or a separate tokio::time::sleep) -> Pipeline state is changed to stop recording -> Task completes -> join_all waits.

Control Command: Manager spawns an async_control_camera task for each specified camera -> Task uses camera::control module (ONVIF/HTTP) -> Task completes -> join_all waits.

6. Future Considerations
Support for more complex GStreamer pipelines defined in config.

System keyring integration for passwords.

Web interface for status/control.

Motion detection hooks.

ONVIF discovery implementation.

More robust handling of diverse camera capabilities and quirks.