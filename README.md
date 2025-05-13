# `rcam` 📸

A tool for capturing images and videos from IP cameras using Rust.

## Environment Variables ⚙️

This project uses a `.env` file to manage environment-specific configurations. Before running the application, ensure you have a `.env` file in the project root. If you have an example file (e.g., `.env.example`), copy it to `.env`:

```bash
cp .env.example .env
```

Then, source the environment variables:

```bash
source .env
```

## Building 🛠️

- For a development build:
  ```bash
  cargo build
  ```
- For a release (production) build:
  ```bash
  cargo build --release
  ```

The executable will be located at `target/debug/rcam` for development builds and `target/release/rcam` for release builds.

## Testing 🧪

To run the test suite:
```bash
cargo test
```

## Example Usage 🚀

The main executable is `rcam`.

**Common Flags:**
- `-c, --config <FILE>`: Sets a custom configuration file (e.g., `rcam --config config/cameras.yaml capture-image`).
- `-d, --debug`: Enables debug logging.

**Subcommands:**

### `capture-image` 🖼️
Captures a single image from specified or all cameras.

- Capture from all cameras:
  ```bash
  rcam capture-image
  ```
- Capture from specific cameras (e.g., `front-door`, `backyard`):
  ```bash
  rcam capture-image --cameras front-door,backyard
  ```
- Specify an output directory:
  ```bash
  rcam capture-image --output /path/to/save/images
  ```
- Add a delay before capturing (in seconds):
  ```bash
  rcam capture-image --delay 5
  ```
- Capture and log to Rerun viewer:
  ```bash
  rcam capture-image --cameras front-door --rerun
  ```

### `capture-video` 📹
Records a video segment from specified or all cameras.

- Record a 60-second video from all cameras:
  ```bash
  rcam capture-video --duration 60
  ```
- Record from specific cameras and set output directory:
  ```bash
  rcam capture-video --cameras front-door --duration 120 --output /path/to/save/videos
  ```
- Record a 30-second video from `cam1` and log frames to Rerun viewer:
  ```bash
  rcam capture-video --cameras cam1 --duration 30 --rerun
  ```

### `verify-times` ⏱️
Verifies time synchronization across all configured cameras.
```bash
rcam verify-times
```

### `test` 🩺
Runs a diagnostic test suite.
```bash
rcam test
```

## Rerun Integration 📊

This tool supports logging images and video frames to the [Rerun](https://www.rerun.io/) viewer for enhanced visualization and debugging.

To enable Rerun logging, use the `--rerun` flag with the `capture-image` or `capture-video` subcommands. If the flag is provided, `rcam` will attempt to spawn a Rerun viewer and stream the captured data to it.

**Installing the Rerun Viewer:**

The Rerun SDK for Rust (which `rcam` uses) typically requires a separate installation of the Rerun viewer binary

- Using cargo:
  ```bash
  cargo install rerun-cli --locked
  ```
  For potentially better video decoding performance, you might need `nasm` installed and use:
  ```bash
  cargo install rerun-cli --locked --features nasm
  ```

After installation, you should be able to run `rerun --help` in your terminal.

Refer to the official [Rerun documentation](https://www.rerun.io/docs) for more details on using the Rerun viewer.

You might need to run the executable directly from the target folder if it's not in your PATH:
- `target/debug/rcam <subcommand> [options]` (for development)
- `target/release/rcam <subcommand> [options]` (for release)
