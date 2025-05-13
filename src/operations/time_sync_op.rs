use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_controller::CameraController;
// use crate::errors::AppError; // AppError might be replaced by anyhow
use anyhow::Result; // Import anyhow::Result
use chrono::{Utc, DateTime};
use log::{info, warn, error};
use futures::future::join_all;
use tokio::task::JoinHandle; // For explicit JoinHandle type

pub async fn handle_verify_times_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    // _args: &clap::ArgMatches, // Not used for now, but could be for specific options
) -> Result<()> { // Changed return type to anyhow::Result
    info!("Handling verify-times command...");

    let camera_controller = CameraController::new(); 
    let cameras_to_target = camera_manager.get_all_cameras().await;

    if cameras_to_target.is_empty() {
        warn!("No cameras configured to verify time synchronization.");
        return Ok(());
    }

    let system_time_now = Utc::now();
    info!("Current system time (UTC): {}", system_time_now.to_rfc3339());

    let mut time_check_tasks: Vec<JoinHandle<Result<(String, DateTime<Utc>)>>> = Vec::new();

    for cam_entity_arc in cameras_to_target {
        let controller_clone = camera_controller.clone();
        let app_settings_clone = master_config.app_settings.clone();
        
        let task = tokio::spawn(async move {
            let cam_entity = cam_entity_arc.lock().await;
            let cam_name_clone = cam_entity.config.name.clone();
            info!("Querying time for camera: '{}'", cam_name_clone);
            
            // Assuming get_camera_time will be updated to return anyhow::Result<DateTime<Utc>>
            // The error will be an anyhow::Error, which includes context.
            match controller_clone.get_camera_time(&*cam_entity, &app_settings_clone).await {
                Ok(camera_time) => {
                    info!("Camera '{}' time (UTC): {}", cam_name_clone, camera_time.to_rfc3339());
                    Ok((cam_name_clone, camera_time))
                }
                Err(e) => {
                    error!("Failed to get time for camera '{}': {:#}", cam_name_clone, e);
                    Err(e) // Propagate anyhow::Error from the task
                }
            }
        });
        time_check_tasks.push(task);
    }

    let results = join_all(time_check_tasks).await;
    let mut successful_times: Vec<(String, DateTime<Utc>)> = Vec::new();
    let mut task_errors = 0;

    for result in results { // result is Result<Result<(String, DateTime<Utc>), anyhow::Error>, JoinError>
        match result {
            Ok(Ok(time_data)) => successful_times.push(time_data),
            Ok(Err(_op_err)) => { // op_err is anyhow::Error, already logged by the task
                task_errors += 1;
            }
            Err(join_err) => { // This is a JoinError (panic)
                error!("Task panicked while getting camera time: {:#}", join_err);
                task_errors += 1;
            }
        }
    }

    if successful_times.is_empty() {
        if task_errors > 0 {
            warn!("Could not retrieve time from any camera due to errors.");
        } else {
            warn!("No camera times were successfully retrieved (no cameras or other issue).");
        }
        // Still returns Ok(()) as per original logic, but logs indicate issues.
        // If an error should be propagated: return Err(anyhow::anyhow!("Failed to retrieve time from any camera"));
        return Ok(());
    }

    let tolerance_seconds = master_config.app_settings.time_sync_tolerance_seconds as i64;
    info!("Time synchronization tolerance: {} seconds", tolerance_seconds);

    let mut all_synced = true;
    for (name, cam_time) in &successful_times {
        let diff_seconds = (cam_time.timestamp() - system_time_now.timestamp()).abs();
        if diff_seconds > tolerance_seconds {
            warn!(
                "Camera '{}' time ({}) is OUT OF SYNC with system time ({}) by {} seconds.",
                name, cam_time.to_rfc3339(), system_time_now.to_rfc3339(), diff_seconds
            );
            all_synced = false;
        } else {
            info!(
                "Camera '{}' time ({}) is IN SYNC with system time ({} seconds difference).",
                name, cam_time.to_rfc3339(), diff_seconds
            );
        }
    }

    if all_synced {
        info!("All successfully queried cameras are synchronized within the tolerance.");
    } else {
        warn!("One or more cameras are out of synchronization. Review warnings above.");
    }

    if successful_times.len() > 1 {
        for i in 0..successful_times.len() {
            for j in (i + 1)..successful_times.len() {
                let (name1, time1) = &successful_times[i];
                let (name2, time2) = &successful_times[j];
                let diff_seconds = (time1.timestamp() - time2.timestamp()).abs();
                if diff_seconds > tolerance_seconds {
                    warn!(
                        "Camera '{}' time ({}) is OUT OF SYNC with camera '{}' time ({}) by {} seconds.",
                        name1, time1.to_rfc3339(), name2, time2.to_rfc3339(), diff_seconds
                    );
                } else {
                    info!(
                        "Camera '{}' time ({}) is IN SYNC with camera '{}' time ({}) within {} seconds.",
                        name1, time1.to_rfc3339(), name2, time2.to_rfc3339(), diff_seconds
                    );
                }
            }
        }
    }

    Ok(())
} 