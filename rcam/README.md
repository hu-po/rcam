# RCam - Multi-Camera Recording System

RCam is a Rust application for recording images and videos concurrently from multiple network IP cameras.

## Features

- Configuration via YAML file.
- Concurrent image and video capture.
- RTSP stream handling.
- ONVIF/HTTP CGI camera control (planned).
- Robust error handling and logging.

## Setup

1.  Install Rust: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
2.  Clone the repository: `git clone <repository-url>`
3.  Navigate to the project directory: `cd rcam`
4.  Build the project: `cargo build`

## Usage

```bash
./target/debug/rcam --config config/default_config.yaml <subcommand>
```

Refer to `./target/debug/rcam --help` for available subcommands and options.

## Configuration

See `config/default_config.yaml` for an example configuration. 