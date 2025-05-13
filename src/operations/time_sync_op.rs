use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_controller::CameraController;
use crate::errors::AppError;
use chrono::{Utc, DateTime};
use log::{info, warn, error};
use futures::future::join_all;

pub async fn handle_verify_times_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    // _args: &clap::ArgMatches, // Not used for now, but could be for specific options
) -> Result<(), AppError> {
    info!("Handling verify-times command...");

    let camera_controller = CameraController::new(); 
    let cameras_to_target = camera_manager.get_all_cameras().await;

    if cameras_to_target.is_empty() {
        warn!("No cameras configured to verify time synchronization.");
        return Ok(());
    }

    let system_time_now = Utc::now();
    info!("Current system time (UTC): {}", system_time_now.to_rfc3339());

    let mut time_check_tasks = Vec::new();

    for cam_entity_arc in cameras_to_target {
        // Clone what's needed for the async block
        let controller_clone = camera_controller.clone();
        let app_settings_clone = master_config.app_settings.clone();

        let task = tokio::spawn(async move {
            let cam_entity = cam_entity_arc.lock().await;
            info!("Querying time for camera: '{}'", cam_entity.config.name);
            match controller_clone.get_camera_time(&*cam_entity, &app_settings_clone).await {
                Ok(camera_time) => {
                    info!("Camera '{}' time (UTC): {}", cam_entity.config.name, camera_time.to_rfc3339());
                    Some((cam_entity.config.name.clone(), camera_time))
                }
                Err(e) => {
                    error!("Failed to get time for camera '{}': {}", cam_entity.config.name, e);
                    None
                }
            }
        });
        time_check_tasks.push(task);
    }

    let results = join_all(time_check_tasks).await;
    let mut successful_times: Vec<(String, DateTime<Utc>)> = Vec::new();
    for result in results {
        match result {
            Ok(Some(time_data)) => successful_times.push(time_data),
            Ok(None) => { /* Error already logged */ }
            Err(e) => error!("Task panicked while getting camera time: {}", e), // Should not happen with proper error handling within task
        }
    }

    if successful_times.is_empty() {
        error!("Could not retrieve time from any camera.");
        return Ok(());
    }

    // Perform synchronization check
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
        // Check cameras against each other if more than one responded
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