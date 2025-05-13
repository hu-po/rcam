use crate::config_loader::MasterConfig;
use crate::app_config::ApplicationConfig;
use crate::core::camera_manager::{CameraManager, parse_camera_names_arg};
use crate::camera::camera_entity::CameraEntity;
use anyhow::{Context, Result};
use clap::ArgMatches;
use futures::future::join_all;
use log::{info, error, warn, debug};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use std::time::Instant;

/// Helper function to orchestrate an operation across multiple cameras.
///
/// This function handles:
/// - Parsing camera selection arguments.
/// - Fetching target cameras.
/// - Determining and creating the base output directory for the operation.
/// - Spawning a Tokio task for each camera to execute the provided `per_camera_op`.
/// - Collecting results and logging errors.
///
/// # Arguments
/// * `master_config`: The application's master configuration.
/// * `camera_manager`: The camera manager instance.
/// * `args`: Command-line arguments.
/// * `operation_display_name`: A user-friendly name for the operation (e.g., "Image Capture").
/// * `output_cli_arg_key`: The key for the CLI argument specifying the output path (e.g., "output").
/// * `default_output_subdir`: An optional subdirectory to be appended to the master config's
///   default output directory if no specific output path is provided via CLI.
///   If `None`, the master config's default output directory is used directly.
/// * `per_camera_op`: An asynchronous closure that defines the work to be done for each camera.
///   It receives:
///     - `Arc<Mutex<CameraEntity>>`: The camera to operate on.
///     - `Arc<ApplicationConfig>`: Shared application settings.
///     - `PathBuf`: The base output directory for the operation.
///
/// # Type Parameters
/// * `F`: The type of the `per_camera_op` closure.
/// * `Fut`: The type of the future returned by `per_camera_op`.
pub async fn run_generic_camera_op<F, Fut>(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
    operation_display_name: &str,
    output_cli_arg_key: &str,
    default_output_subdir: Option<&str>,
    per_camera_op: F,
) -> Result<()>
where
    F: Fn(Arc<Mutex<CameraEntity>>, Arc<ApplicationConfig>, PathBuf) -> Fut + Send + Sync + 'static + Clone,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let op_helper_start_time = Instant::now();
    info!("🛠️ Starting generic operation: '{}'...", operation_display_name);

    // 1. Parse camera selection
    let camera_parse_start = Instant::now();
    let specific_cameras_arg = args.get_one::<String>("cameras"); // Assuming "cameras" is the standard key
    let camera_names_to_process = parse_camera_names_arg(specific_cameras_arg);
    debug!(
        "  Parsed camera selection for '{}' in {:?}. CLI arg: {:?}, Parsed: {:?}",
        operation_display_name, camera_parse_start.elapsed(), specific_cameras_arg, camera_names_to_process
    );

    let cameras_fetch_start = Instant::now();
    let cameras_to_target = match camera_names_to_process {
        Some(ref names) => {
            debug!("  Fetching specific cameras by names: {:?} for '{}'", names, operation_display_name);
            camera_manager.get_cameras_by_names(names).await
        }
        None => {
            debug!("  Fetching all available cameras for '{}'", operation_display_name);
            camera_manager.get_all_cameras().await
        }
    };
    debug!("  Fetched {} target cameras for '{}' in {:?}.", cameras_to_target.len(), operation_display_name, cameras_fetch_start.elapsed());

    if cameras_to_target.is_empty() {
        if let Some(names) = camera_names_to_process {
            warn!(
                "⚠️ No cameras found matching names: {:?} for '{}'. Operation finished in {:?}",
                names, operation_display_name, op_helper_start_time.elapsed()
            );
        } else {
            warn!(
                "⚠️ No cameras configured or found for '{}'. Operation finished in {:?}",
                operation_display_name, op_helper_start_time.elapsed()
            );
        }
        return Ok(());
    }
    info!("🎯 Targeting {} camera(s) for {}.", cameras_to_target.len(), operation_display_name);

    // 2. Determine and create output directory
    let output_dir_determine_start = Instant::now();
    let operation_base_output_dir: PathBuf = match args.get_one::<String>(output_cli_arg_key) {
        Some(path_str) => {
            debug!("  Output directory specified via CLI for '{}': {}", operation_display_name, path_str);
            PathBuf::from(path_str)
        }
        None => {
            let mut dir = PathBuf::from(&master_config.app_settings.output_directory);
            if let Some(subdir) = default_output_subdir {
                dir.push(subdir);
                debug!("  Using default output directory with subdir for '{}': {}", operation_display_name, dir.display());
            } else {
                debug!("  Using default output directory for '{}': {}", operation_display_name, dir.display());
            }
            dir
        }
    };
    debug!("  Determined operation base output directory for '{}' as '{}' in {:?}.", operation_display_name, operation_base_output_dir.display(), output_dir_determine_start.elapsed());

    if !operation_base_output_dir.exists() {
        info!("📁 Output directory {} does not exist. Creating it for '{}'.", operation_base_output_dir.display(), operation_display_name);
        let create_dir_start = Instant::now();
        std::fs::create_dir_all(&operation_base_output_dir)
            .with_context(|| format!(
                    "❌ Failed to create output directory '{}' for '{}'",
                    operation_base_output_dir.display(), operation_display_name
            ))?;
        info!("  ✅ Created output directory '{}' for '{}' in {:?}", operation_base_output_dir.display(), operation_display_name, create_dir_start.elapsed());
    } else {
        info!("ℹ️ Using existing output directory: {} for '{}'", operation_base_output_dir.display(), operation_display_name);
    }

    // 3. Spawn tasks
    let task_spawn_loop_start = Instant::now();
    let mut tasks: Vec<JoinHandle<Result<()>>> = Vec::new();
    let app_settings_arc = Arc::new(master_config.app_settings.clone());

    for cam_entity_arc in cameras_to_target.iter() { // Iterate over owned Vec
        let op_clone = per_camera_op.clone();
        let task_app_settings = Arc::clone(&app_settings_arc);
        let task_output_dir = operation_base_output_dir.clone();
        let cam_arc_clone = Arc::clone(cam_entity_arc); // This is Arc<Mutex<CameraEntity>>, clone the Arc
        let operation_display_name_owned = operation_display_name.to_string(); // Clone for the task

        let per_cam_task_spawn_start = Instant::now();
        tasks.push(tokio::spawn(async move {
            // It's good practice to capture the camera name early for logging if the op_clone panics
            let camera_name_for_log = cam_arc_clone.lock().await.config.name.clone();
            debug!("    Task for camera '{}' (operation '{}') started.", camera_name_for_log, operation_display_name_owned);
            let res = op_clone(cam_arc_clone, task_app_settings, task_output_dir).await;
            if res.is_err() {
                debug!("    Task for camera '{}' (operation '{}') finished with an error.", camera_name_for_log, operation_display_name_owned);
            } else {
                debug!("    Task for camera '{}' (operation '{}') finished successfully.", camera_name_for_log, operation_display_name_owned);
            }
            res
        }));
        debug!("  Spawned task for camera (op: '{}') in {:?}. Total tasks: {}", operation_display_name, per_cam_task_spawn_start.elapsed(), tasks.len());
    }
    debug!("Finished spawning all {} tasks for '{}' in {:?}.", tasks.len(), operation_display_name, task_spawn_loop_start.elapsed());

    // 4. Join tasks and collect results
    info!("🔄 Waiting for all {} tasks to complete for '{}'...", tasks.len(), operation_display_name);
    let join_all_start = Instant::now();
    let results = join_all(tasks).await;
    debug!("Joined all {} tasks for '{}' in {:?}. Processing results...", results.len(), operation_display_name, join_all_start.elapsed());

    let mut operation_errors = 0;

    for (i, task_result) in results.into_iter().enumerate() {
        match task_result {
            Ok(Ok(())) => {
                debug!("  Task {} for '{}' completed successfully.", i + 1, operation_display_name);
            }
            Ok(Err(op_err)) => {
                error!("❌ Error during '{}' for camera task {}: {:#}", operation_display_name, i + 1, op_err);
                operation_errors += 1;
            }
            Err(join_err) => {
                error!("💀 Task execution failed (panic or cancellation) for '{}' for camera task {}: {:#}", operation_display_name, i + 1, join_err);
                operation_errors += 1;
            }
        }
    }

    if operation_errors == 0 {
        info!("✅ All {} tasks completed successfully for {} camera(s) for '{}'.", operation_display_name, cameras_to_target.len(), operation_display_name);
    } else {
        warn!(
            "⚠️ '{}' operation completed with {} error(s) out of {} task(s). Please check logs.",
            operation_display_name,
            operation_errors,
            cameras_to_target.len()
        );
    }
    info!("🏁 '{}' operation finished in {:?}.", operation_display_name, op_helper_start_time.elapsed());
    Ok(())
} 