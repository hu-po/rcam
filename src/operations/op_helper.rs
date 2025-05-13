use crate::config_loader::MasterConfig;
use crate::app_config::ApplicationConfig;
use crate::core::camera_manager::{CameraManager, parse_camera_names_arg};
use crate::camera::camera_entity::CameraEntity;
use crate::errors::AppError;
use clap::ArgMatches;
use futures::future::join_all;
use log::{info, error, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

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
) -> Result<(), AppError>
where
    F: Fn(Arc<Mutex<CameraEntity>>, Arc<ApplicationConfig>, PathBuf) -> Fut + Send + Sync + 'static + Clone,
    Fut: std::future::Future<Output = Result<(), AppError>> + Send + 'static,
{
    info!("Starting {} operation...", operation_display_name);

    // 1. Parse camera selection
    let specific_cameras_arg = args.get_one::<String>("cameras"); // Assuming "cameras" is the standard key
    let camera_names_to_process = parse_camera_names_arg(specific_cameras_arg);

    let cameras_to_target = match camera_names_to_process {
        Some(ref names) => camera_manager.get_cameras_by_names(names).await,
        None => camera_manager.get_all_cameras().await,
    };

    if cameras_to_target.is_empty() {
        if let Some(names) = camera_names_to_process {
            warn!(
                "No cameras found matching names: {:?}. Please check camera names and configuration for {}.",
                names, operation_display_name
            );
        } else {
            warn!("No cameras configured or found for {}.", operation_display_name);
        }
        return Ok(()); 
    }
    info!("Targeting {} camera(s) for {}.", cameras_to_target.len(), operation_display_name);

    // 2. Determine and create output directory
    let operation_base_output_dir: PathBuf = match args.get_one::<String>(output_cli_arg_key) {
        Some(path_str) => PathBuf::from(path_str),
        None => {
            let mut dir = PathBuf::from(&master_config.app_settings.output_directory);
            if let Some(subdir) = default_output_subdir {
                dir.push(subdir);
            }
            dir
        }
    };

    if !operation_base_output_dir.exists() {
        info!("Output directory {} does not exist. Creating it.", operation_base_output_dir.display());
        std::fs::create_dir_all(&operation_base_output_dir).map_err(|e| {
            AppError::Io(format!(
                "Failed to create output directory \'{}\': {}",
                operation_base_output_dir.display(),
                e
            ))
        })?;
    } else {
        info!("Using existing output directory: {}", operation_base_output_dir.display());
    }

    // 3. Spawn tasks
    let mut tasks: Vec<JoinHandle<Result<(), AppError>>> = Vec::new();
    let app_settings_arc = Arc::new(master_config.app_settings.clone());

    for cam_entity_arc in cameras_to_target.iter() { // Iterate over owned Vec
        let op_clone = per_camera_op.clone();
        let task_app_settings = Arc::clone(&app_settings_arc);
        let task_output_dir = operation_base_output_dir.clone();
        let cam_arc_clone = Arc::clone(cam_entity_arc); // This is Arc<Mutex<CameraEntity>>, clone the Arc

        tasks.push(tokio::spawn(async move {
            op_clone(cam_arc_clone, task_app_settings, task_output_dir).await
        }));
    }

    // 4. Join tasks and collect results
    let results = join_all(tasks).await;
    let mut operation_errors = 0;

    for (i, task_result) in results.into_iter().enumerate() {
        match task_result {
            Ok(Ok(())) => { /* Per-camera operation successful */ }
            Ok(Err(op_err)) => {
                // Error from within per_camera_op
                error!("Error during {} for camera task {}: {}", operation_display_name, i + 1, op_err);
                operation_errors += 1;
            }
            Err(join_err) => {
                // Tokio task join error (e.g., panic in task)
                error!("Task execution failed for {} for camera task {}: {}", operation_display_name, i + 1, join_err);
                operation_errors += 1;
            }
        }
    }

    if operation_errors == 0 {
        info!("All {} tasks completed successfully for {} camera(s).", operation_display_name, cameras_to_target.len());
    } else {
        warn!(
            "{} operation completed with {} error(s) out of {} task(s). Please check logs.",
            operation_display_name,
            operation_errors,
            cameras_to_target.len()
        );
    }
    info!("{} operation finished.", operation_display_name);
    Ok(())
} 