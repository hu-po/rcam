use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_controller::CameraController;
// use crate::errors::AppError; // AppError might be replaced by anyhow
use anyhow::Result; // Import anyhow::Result
use chrono::{Utc, DateTime};
use log::{info, warn, error, debug};
use futures::future::join_all;
use tokio::task::JoinHandle; // For explicit JoinHandle type
use std::time::Instant; // Added Instant

pub async fn handle_verify_times_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    // _args: &clap::ArgMatches, // Not used for now, but could be for specific options
) -> Result<()> { // Changed return type to anyhow::Result
    let op_start_time = Instant::now();
    info!("⏱️ Handling verify-times command...");

    let controller_init_start = Instant::now();
    let camera_controller = CameraController::new(); 
    debug!("CameraController initialized for time verification in {:?}.", controller_init_start.elapsed());

    let cameras_fetch_start = Instant::now();
    let cameras_to_target = camera_manager.get_all_cameras().await;
    debug!("Fetched {} cameras to target in {:?}.", cameras_to_target.len(), cameras_fetch_start.elapsed());

    if cameras_to_target.is_empty() {
        warn!("🤔 No cameras configured to verify time synchronization. Operation finished in {:?}.", op_start_time.elapsed());
        return Ok(());
    }

    let system_time_now = Utc::now();
    info!("🌍 Current system time (UTC): {}", system_time_now.to_rfc3339());

    let mut time_check_tasks: Vec<JoinHandle<Result<(String, DateTime<Utc>)>>> = Vec::new();
    let task_creation_start_time = Instant::now();

    for cam_entity_arc in cameras_to_target {
        let controller_clone = camera_controller.clone();
        let app_settings_clone = master_config.app_settings.clone();
        
        let task_spawn_start = Instant::now();
        let task = tokio::spawn(async move {
            let cam_entity_lock_start = Instant::now();
            let cam_entity = cam_entity_arc.lock().await;
            let cam_name_clone = cam_entity.config.name.clone();
            debug!("  Task for '{}': Locked camera entity in {:?}.", cam_name_clone, cam_entity_lock_start.elapsed());
            info!("  Querying time for camera: '{}' 🛰️", cam_name_clone);
            let get_time_start = Instant::now();
            
            // Assuming get_camera_time will be updated to return anyhow::Result<DateTime<Utc>>
            // The error will be an anyhow::Error, which includes context.
            match controller_clone.get_camera_time(&*cam_entity, &app_settings_clone).await {
                Ok(camera_time) => {
                    info!("  ✅ Camera '{}' time (UTC): {}. Fetched in {:?}.", cam_name_clone, camera_time.to_rfc3339(), get_time_start.elapsed());
                    Ok((cam_name_clone, camera_time))
                }
                Err(e) => {
                    error!("  ❌ Failed to get time for camera '{}' after {:?}: {:#}", cam_name_clone, get_time_start.elapsed(), e);
                    Err(e) // Propagate anyhow::Error from the task
                }
            }
        });
        time_check_tasks.push(task);
        debug!("  Spawned time check task for a camera in {:?}. Total tasks: {}", task_spawn_start.elapsed(), time_check_tasks.len());
    }
    debug!("All time check tasks ({}) spawned in {:?}.", time_check_tasks.len(), task_creation_start_time.elapsed());

    let join_all_start_time = Instant::now();
    let results = join_all(time_check_tasks).await;
    debug!("Joined all ({}) time check tasks in {:?}.", results.len(), join_all_start_time.elapsed());

    let mut successful_times: Vec<(String, DateTime<Utc>)> = Vec::new();
    let mut task_errors = 0;

    for result in results { // result is Result<Result<(String, DateTime<Utc>), anyhow::Error>, JoinError>
        match result {
            Ok(Ok(time_data)) => successful_times.push(time_data),
            Ok(Err(_op_err)) => { // op_err is anyhow::Error, already logged by the task
                task_errors += 1;
                debug!("  Encountered an operation error within a task.");
            }
            Err(join_err) => { // This is a JoinError (panic)
                error!("💀 Task panicked while getting camera time: {:#}", join_err);
                task_errors += 1;
            }
        }
    }

    if successful_times.is_empty() {
        if task_errors > 0 {
            warn!("⚠️ Could not retrieve time from any camera due to {} errors. Operation finished in {:?}.", task_errors, op_start_time.elapsed());
        } else {
            warn!("🤔 No camera times were successfully retrieved (no cameras or other issue). Operation finished in {:?}.", op_start_time.elapsed());
        }
        // Still returns Ok(()) as per original logic, but logs indicate issues.
        // If an error should be propagated: return Err(anyhow::anyhow!("Failed to retrieve time from any camera"));
        return Ok(());
    }

    let tolerance_seconds = master_config.app_settings.time_sync_tolerance_seconds as i64;
    info!("🕒 Time synchronization tolerance: {} seconds", tolerance_seconds);

    let mut all_synced_system = true;
    let system_sync_check_start = Instant::now();
    for (name, cam_time) in &successful_times {
        let diff_seconds = (cam_time.timestamp() - system_time_now.timestamp()).abs();
        if diff_seconds > tolerance_seconds {
            warn!(
                "❌ Camera '{}' time ({}) is OUT OF SYNC with system time ({}) by {} seconds (tolerance: {}s).",
                name, cam_time.to_rfc3339(), system_time_now.to_rfc3339(), diff_seconds, tolerance_seconds
            );
            all_synced_system = false;
        } else {
            info!(
                "✅ Camera '{}' time ({}) is IN SYNC with system time ({} seconds difference, tolerance: {}s).",
                name, cam_time.to_rfc3339(), diff_seconds, tolerance_seconds
            );
        }
    }
    debug!("System time synchronization check completed in {:?}. All synced with system: {}", system_sync_check_start.elapsed(), all_synced_system);

    if all_synced_system {
        info!("👍 All successfully queried cameras are synchronized with the system time within the tolerance.");
    } else {
        warn!("👎 One or more cameras are out of synchronization with the system time. Review warnings above.");
    }

    let mut all_cameras_inter_synced = true;
    if successful_times.len() > 1 {
        info!("��️ Performing inter-camera time synchronization checks...");
        let inter_sync_check_start = Instant::now();
        for i in 0..successful_times.len() {
            for j in (i + 1)..successful_times.len() {
                let (name1, time1) = &successful_times[i];
                let (name2, time2) = &successful_times[j];
                let diff_seconds = (time1.timestamp() - time2.timestamp()).abs();
                if diff_seconds > tolerance_seconds {
                    warn!(
                        "❌ Camera '{}' time ({}) is OUT OF SYNC with camera '{}' time ({}) by {} seconds (tolerance: {}s).",
                        name1, time1.to_rfc3339(), name2, time2.to_rfc3339(), diff_seconds, tolerance_seconds
                    );
                    all_cameras_inter_synced = false;
                } else {
                    info!(
                        "✅ Camera '{}' time ({}) is IN SYNC with camera '{}' time ({}) ({}s difference, tolerance: {}s).",
                        name1, time1.to_rfc3339(), name2, time2.to_rfc3339(), diff_seconds, tolerance_seconds
                    );
                }
            }
        }
        debug!("Inter-camera time synchronization check completed in {:?}. All cameras inter-synced: {}", inter_sync_check_start.elapsed(), all_cameras_inter_synced);
        if all_cameras_inter_synced {
            info!("👍 All successfully queried cameras are synchronized with each other within the tolerance.");
        } else {
            warn!("👎 One or more cameras are out of synchronization with each other. Review warnings above.");
        }
    } else if successful_times.len() == 1 {
        info!("ℹ️ Only one camera time successfully retrieved, skipping inter-camera sync check.");
    }

    info!("🏁 Verify-times operation finished in {:?}.", op_start_time.elapsed());
    Ok(())
} 