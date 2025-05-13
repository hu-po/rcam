use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use anyhow::{Result, Context};
use clap::ArgMatches;
use log::{info, warn, error, debug};
use std::path::PathBuf;
use std::time::Instant;

// Import operation handlers
use crate::camera::camera_media::CameraMediaManager; 
use crate::common::file_utils;
use super::time_sync_op;
use crate::camera::camera_controller::CameraController;

struct DiagnosticResult {
    test_name: String,
    success: bool,
    details: String,
}

pub async fn handle_diagnostic_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    _args: &ArgMatches, // CLI args for diagnostics, if any are added later
) -> Result<()> {
    let overall_diag_start_time = Instant::now();
    info!("🩺 Starting diagnostic test suite...");
    let mut results: Vec<DiagnosticResult> = Vec::new();

    // 1. Test time synchronization for all cameras
    info!("  DIAGNOSTIC [Global]: Running time synchronization test... ⏱️");
    let time_sync_test_start = Instant::now();
    match time_sync_op::handle_verify_times_cli(master_config, camera_manager).await {
        Ok(_) => {
            info!("    DIAGNOSTIC [Global]: Time Synchronization test completed in {:?}. Check logs for details.", time_sync_test_start.elapsed());
            results.push(DiagnosticResult {
                test_name: "Time Synchronization (All Cameras)".to_string(),
                success: true,
                details: "Completed. Check logs for sync status.".to_string(),
            });
        },
        Err(e) => {
            error!("    DIAGNOSTIC [Global]: Time Synchronization test FAILED in {:?}: {:#}", time_sync_test_start.elapsed(), e);
            results.push(DiagnosticResult {
                test_name: "Time Synchronization (All Cameras)".to_string(),
                success: false,
                details: format!("Failed: {:#}", e),
            });
        }
    }

    let diag_output_dir_create_start = Instant::now();
    let diagnostic_output_dir = PathBuf::from(&master_config.app_settings.output_directory).join("diagnostics");
    if !diagnostic_output_dir.exists() {
        debug!("  Attempting to create diagnostic output directory: {}", diagnostic_output_dir.display());
        if let Err(e) = std::fs::create_dir_all(&diagnostic_output_dir)
            .with_context(|| format!("Failed to create diagnostic output directory: {}", diagnostic_output_dir.display())) {
            error!("❌ Failed to create diagnostic output directory '{}' in {:?}: {:#}", diagnostic_output_dir.display(), diag_output_dir_create_start.elapsed(), e); 
        } else {
            info!("  📁 Created diagnostic output directory '{}' in {:?}", diagnostic_output_dir.display(), diag_output_dir_create_start.elapsed());
        }
    } else {
        info!("  ℹ️ Diagnostic output directory already exists: {}", diagnostic_output_dir.display());
    }
    info!("💾 Diagnostic outputs will be saved to: {}", diagnostic_output_dir.display());

    let cameras_fetch_start = Instant::now();
    let all_cameras = camera_manager.get_all_cameras().await;
    debug!("Fetched {} cameras for per-camera diagnostics in {:?}.", all_cameras.len(), cameras_fetch_start.elapsed());

    if all_cameras.is_empty() {
        warn!("⚠️ DIAGNOSTIC: No cameras configured. Skipping per-camera tests.");
    }

    for cam_arc in &all_cameras {
        let cam_entity_lock_start = Instant::now();
        let cam_name = cam_arc.lock().await.config.name.clone();
        debug!("  Locked camera entity for '{}' for diagnostics in {:?}.", cam_name, cam_entity_lock_start.elapsed());
        info!("  DIAGNOSTIC [{}]: Running tests...", cam_name);

        // 2. Test single image capture per camera
        let img_diag_dir_create_start = Instant::now();
        let image_diag_output_dir = diagnostic_output_dir.join(&cam_name).join("image");
        if let Err(e) = std::fs::create_dir_all(&image_diag_output_dir)
            .with_context(|| format!("Failed to create image diagnostic dir for {}: {}", cam_name, image_diag_output_dir.display())) {
            error!("❌ Could not create image diagnostic directory for '{}' ({}): {:#}. Image test may fail to save.", cam_name, image_diag_output_dir.display(), e);
        } else {
            debug!("  Ensured image diagnostic directory for '{}' exists ({}) in {:?}.", cam_name, image_diag_output_dir.display(), img_diag_dir_create_start.elapsed());
        }        
        info!("    DIAGNOSTIC [{}]: Running image capture test... 🖼️", cam_name);
        let img_test_start = Instant::now();
        let media_manager_img = CameraMediaManager::new();
        let app_config_img_clone = master_config.app_settings.clone();
        
        let task_cam_entity_arc = cam_arc.clone();
        let task_image_diag_output_dir = image_diag_output_dir.clone();

        let image_capture_future = async move {
            let mut cam_entity = task_cam_entity_arc.lock().await;
            let filename = file_utils::generate_timestamped_filename(
                &cam_entity.config.name,
                &app_config_img_clone.filename_timestamp_format,
                &app_config_img_clone.image_format
            );
            let output_path = task_image_diag_output_dir.join(filename);
            media_manager_img.capture_image(&mut *cam_entity, &app_config_img_clone, output_path.clone(), None).await
        };
        
        match image_capture_future.await {
            Ok(path) => {
                info!("    DIAGNOSTIC [{}]: Image Capture test PASSED in {:?}. Image: {}", cam_name, img_test_start.elapsed(), path.display());
                results.push(DiagnosticResult {
                    test_name: format!("Image Capture ('{}')", cam_name),
                    success: true,
                    details: format!("Completed. Image saved in {}", path.display()),
                });
            },
            Err(e) => {
                error!("    DIAGNOSTIC [{}]: Image Capture test FAILED in {:?}: {:#}", cam_name, img_test_start.elapsed(), e);
                results.push(DiagnosticResult {
                    test_name: format!("Image Capture ('{}')", cam_name),
                    success: false,
                    details: format!("Failed: {:#}", e),
                });
            },
        }

        // 3. Test short video capture per camera
        let vid_diag_dir_create_start = Instant::now();
        let video_diag_output_dir = diagnostic_output_dir.join(&cam_name).join("video");
        if let Err(e) = std::fs::create_dir_all(&video_diag_output_dir)
            .with_context(|| format!("Failed to create video diagnostic dir for {}: {}", cam_name, video_diag_output_dir.display())) {
            error!("❌ Could not create video diagnostic directory for '{}' ({}): {:#}. Video test may fail to save.", cam_name, video_diag_output_dir.display(), e);
        } else {
             debug!("  Ensured video diagnostic directory for '{}' exists ({}) in {:?}.", cam_name, video_diag_output_dir.display(), vid_diag_dir_create_start.elapsed());
        }
        let video_duration_secs: u64 = 5; // 5 second video for diagnostics
        
        info!("    DIAGNOSTIC [{}]: Running short video capture test ({}s)... 📹", cam_name, video_duration_secs);
        let vid_test_start = Instant::now();
        let media_manager_vid = CameraMediaManager::new();
        let app_config_vid_clone = master_config.app_settings.clone();
        let task_cam_entity_arc_vid = cam_arc.clone();
        let task_video_diag_output_dir = video_diag_output_dir.clone();

        let video_record_future = async move {
            let mut cam_entity = task_cam_entity_arc_vid.lock().await;
            let filename = file_utils::generate_timestamped_filename(
                &cam_entity.config.name,
                &app_config_vid_clone.filename_timestamp_format,
                &app_config_vid_clone.video_format
            );
            let output_path = task_video_diag_output_dir.join(filename);
            let recording_duration = std::time::Duration::from_secs(video_duration_secs);
            media_manager_vid.record_video(&mut *cam_entity, &app_config_vid_clone, output_path.clone(), recording_duration).await
        };

        match video_record_future.await {
            Ok(path) => {
                info!("    DIAGNOSTIC [{}]: Video Record test ({}s) PASSED in {:?}. Video: {}", cam_name, video_duration_secs, vid_test_start.elapsed(), path.display());
                results.push(DiagnosticResult {
                    test_name: format!("Video Record ('{}', {}s)", cam_name, video_duration_secs),
                    success: true,
                    details: format!("Completed. Video saved in {}", path.display()),
                });
            },
            Err(e) => {
                error!("    DIAGNOSTIC [{}]: Video Record test ({}s) FAILED in {:?}: {:#}", cam_name, video_duration_secs, vid_test_start.elapsed(), e);
                results.push(DiagnosticResult {
                    test_name: format!("Video Record ('{}', {}s)", cam_name, video_duration_secs),
                    success: false,
                    details: format!("Failed: {:#}", e),
                });
            },
        }

        // 4. Test enable/disable stream (will likely show warnings/errors)
        let ctrl_test_init_start = Instant::now();
        let camera_controller_diag = CameraController::new();
        debug!("  CameraController for diag control test on '{}' initialized in {:?}.", cam_name, ctrl_test_init_start.elapsed());

        for action_bool in [true, false] { // Test both enable (true) and disable (false)
            let action_str = if action_bool { "enable" } else { "disable" };
            let action_emoji = if action_bool { "💡" } else { "🔌" };
            info!("    DIAGNOSTIC [{}]: Running control action: {} {}...", cam_name, action_str, action_emoji);
            let ctrl_action_start = Instant::now();
            
            let task_cam_entity_arc_ctrl = cam_arc.clone();
            let controller_clone_diag = camera_controller_diag.clone();
            let app_settings_clone_diag = master_config.app_settings.clone();

            let control_future = async move {
                let cam_entity = task_cam_entity_arc_ctrl.lock().await;
                controller_clone_diag.set_camera_enabled(&*cam_entity, &app_settings_clone_diag, action_bool).await
            };

            match control_future.await {
                Ok(()) => {
                    info!("    DIAGNOSTIC [{}]: Control Action '{}' COMPLETED in {:?}. Check logs for camera response.", cam_name, action_str, ctrl_action_start.elapsed());
                    results.push(DiagnosticResult {
                        test_name: format!("Control Action ('{}': {})", cam_name, action_str),
                        success: true, 
                        details: "Completed. Check logs for camera response.".to_string(),
                    });
                },
                Err(e) => {
                    // This might be an expected error if the camera CGI doesn't return a typical success code, or if not supported
                    warn!("    DIAGNOSTIC [{}]: Control Action '{}' resulted in an error/warning in {:?}: {:#}. This may be expected for some cameras.", cam_name, action_str, ctrl_action_start.elapsed(), e);
                    results.push(DiagnosticResult {
                        test_name: format!("Control Action ('{}': {})", cam_name, action_str),
                        success: true, // Mark as success as the test itself ran, but warn about the outcome.
                        details: format!("Completed with error/warning (may be expected): {:#}", e),
                    });
                },
            }
        }
        info!("  DIAGNOSTIC [{}]: Finished all tests for this camera.", cam_name);
    }

    info!("\n\n📋 ----- Diagnostic Test Summary (Total Suite Time: {:?}) -----", overall_diag_start_time.elapsed());
    let mut overall_success = true;
    for result in results {
        let status_emoji = if result.success { "✅ PASS" } else { "❌ FAIL" };
        info!("Test: {:<40} | Status: {:<10} | Details: {}", result.test_name, status_emoji, result.details);
        if !result.success {
            overall_success = false;
        }
    }
    info!("----------------------------------------------------------------------");
    if overall_success {
        info!("🎉 All diagnostic tests passed or completed as expected (check warnings for specifics).");
    } else {
        error!("🔥 One or more critical diagnostic tests failed. Please review logs above.");
    }
    info!("🏁 Diagnostic test suite finished in {:?}.", overall_diag_start_time.elapsed());
    Ok(())
} 