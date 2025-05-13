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

### `verify-times` ⏱️
Verifies time synchronization across all configured cameras.
```bash
rcam verify-times
```

### `control` 🕹️
Controls camera functionalities like enabling or disabling them.

- Disable `cam1`:
  ```bash
  rcam control --action disable --cameras cam1
  ```
- Enable all cameras:
  ```bash
  rcam control --action enable
  ```

### `test` 🩺
Runs a diagnostic test suite.
```bash
rcam test
```

You might need to run the executable directly from the target folder if it's not in your PATH:
- `target/debug/rcam <subcommand> [options]` (for development)
- `target/release/rcam <subcommand> [options]` (for release)
