use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::errors::AppError;
use clap::ArgMatches;
use log::{info, warn, error};
use std::path::PathBuf;
use std::time::Duration;

// Import operation handlers
use super::image_capture_op;
use super::video_record_op;
use super::time_sync_op;
use super::camera_control_op;

struct DiagnosticResult {
    test_name: String,
    success: bool,
    details: String,
}

pub async fn handle_diagnostic_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    _args: &ArgMatches, // CLI args for diagnostics, if any are added later
) -> Result<(), AppError> {
    info!("Starting diagnostic test suite...");
    let mut results: Vec<DiagnosticResult> = Vec::new();

    // 1. Test time synchronization for all cameras
    info!("DIAGNOSTIC: Running time synchronization test...");
    match time_sync_op::handle_verify_times_cli(master_config, camera_manager).await {
        Ok(_) => results.push(DiagnosticResult {
            test_name: "Time Synchronization (All Cameras)".to_string(),
            success: true,
            details: "Completed. Check logs for sync status.".to_string(),
        }),
        Err(e) => results.push(DiagnosticResult {
            test_name: "Time Synchronization (All Cameras)".to_string(),
            success: false,
            details: format!("Failed: {}", e),
        }),
    }

    let diagnostic_output_dir = PathBuf::from(&master_config.app_settings.output_directory).join("diagnostics");
    if !diagnostic_output_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&diagnostic_output_dir) {
            error!("Failed to create diagnostic output directory: {}. Some tests might fail to save files.", e);
            // Decide if we should stop or continue. For now, continue and let individual tests handle it.
        }
    }
    info!("Diagnostic outputs will be saved to: {}", diagnostic_output_dir.display());

    let all_cameras = camera_manager.get_all_cameras().await;
    if all_cameras.is_empty() {
        warn!("DIAGNOSTIC: No cameras configured. Skipping per-camera tests.");
    }

    for cam_arc in &all_cameras {
        let cam_name = cam_arc.lock().await.config.name.clone();
        info!("DIAGNOSTIC: Running tests for camera: {}", cam_name);

        // Create a temporary ArgMatches for single camera operations if needed,
        // or adapt operation handlers to accept direct camera names/entities.
        // For simplicity, we'll construct minimal ArgMatches for now.
        // This is a bit of a hack; ideally, op handlers would also have non-CLI entry points.

        // 2. Test single image capture per camera
        let image_diag_output_dir = diagnostic_output_dir.join(&cam_name).join("image");
        std::fs::create_dir_all(&image_diag_output_dir).ok(); // Best effort
        
        // Construct mock CLI args for single image capture
        let single_cam_name_vec = vec![("cameras".to_string(), cam_name.clone())];
        let output_arg_vec = vec![("output".to_string(), image_diag_output_dir.to_string_lossy().to_string())];
        let image_args_map: std::collections::HashMap<String, clap::parser::RawValues> = 
            single_cam_name_vec.into_iter().chain(output_arg_vec.into_iter())
            .map(|(k,v)| (k, clap::parser::RawValues::new(vec![v.into()])))
            .collect();
        let image_args = ArgMatches::from_iter_map(image_args_map);

        info!("DIAGNOSTIC [{}]: Running image capture test...", cam_name);
        match image_capture_op::handle_capture_image_cli(master_config, camera_manager, &image_args).await {
            Ok(_) => results.push(DiagnosticResult {
                test_name: format!("Image Capture ('{}')", cam_name),
                success: true,
                details: format!("Completed. Image saved in {}", image_diag_output_dir.display()),
            }),
            Err(e) => results.push(DiagnosticResult {
                test_name: format!("Image Capture ('{}')", cam_name),
                success: false,
                details: format!("Failed: {}", e),
            }),
        }

        // 3. Test short video capture per camera
        let video_diag_output_dir = diagnostic_output_dir.join(&cam_name).join("video");
        std::fs::create_dir_all(&video_diag_output_dir).ok(); // Best effort
        let video_duration_arg_vec = vec![("duration".to_string(), "5".to_string())]; // 5 second video
        let video_output_arg_vec = vec![("output".to_string(), video_diag_output_dir.to_string_lossy().to_string())];
        let video_args_map: std::collections::HashMap<String, clap::parser::RawValues> = 
            single_cam_name_vec.iter().cloned().chain(video_duration_arg_vec.into_iter()).chain(video_output_arg_vec.into_iter())
            .map(|(k,v)| (k, clap::parser::RawValues::new(vec![v.into()])))
            .collect();
        let video_args = ArgMatches::from_iter_map(video_args_map);
        
        info!("DIAGNOSTIC [{}]: Running short video capture test (5s)...", cam_name);
        match video_record_op::handle_record_video_cli(master_config, camera_manager, &video_args).await {
            Ok(_) => results.push(DiagnosticResult {
                test_name: format!("Video Record ('{}', 5s)", cam_name),
                success: true,
                details: format!("Completed. Video saved in {}", video_diag_output_dir.display()),
            }),
            Err(e) => results.push(DiagnosticResult {
                test_name: format!("Video Record ('{}', 5s)", cam_name),
                success: false,
                details: format!("Failed: {}", e),
            }),
        }

        // 4. Test enable/disable stream (will likely show warnings/errors)
        for action_bool in [true, false] {
            let action_str = if action_bool { "enable" } else { "disable" };
            let control_action_arg_vec = vec![("action".to_string(), action_str.to_string())];
            let control_args_map: std::collections::HashMap<String, clap::parser::RawValues> = 
                single_cam_name_vec.iter().cloned().chain(control_action_arg_vec.into_iter())
                .map(|(k,v)| (k, clap::parser::RawValues::new(vec![v.into()])))
                .collect();
            let control_args = ArgMatches::from_iter_map(control_args_map);

            info!("DIAGNOSTIC [{}]: Running control action: {}...", cam_name, action_str);
            match camera_control_op::handle_control_camera_cli(master_config, camera_manager, &control_args).await {
                Ok(_) => results.push(DiagnosticResult {
                    test_name: format!("Control Action ('{}': {})", cam_name, action_str),
                    success: true, // Success means the op itself didn't error, not that the camera necessarily obeyed
                    details: "Completed. Check logs for camera response.".to_string(),
                }),
                Err(e) => results.push(DiagnosticResult {
                    test_name: format!("Control Action ('{}': {})", cam_name, action_str),
                    success: false,
                    details: format!("Failed: {}", e),
                }),
            }
        }
    }

    info!("\n----- Diagnostic Test Summary -----");
    let mut overall_success = true;
    for result in results {
        let status = if result.success { "PASS" } else { "FAIL" };
        info!("Test: {:<40} | Status: {:<4} | Details: {}", result.test_name, status, result.details);
        if !result.success {
            overall_success = false;
        }
    }
    info!("-----------------------------------");
    if overall_success {
        info!("All diagnostic tests passed (or completed without internal errors).");
    } else {
        error!("One or more diagnostic tests failed or encountered errors. Please review logs.");
    }
    info!("Diagnostic test suite finished.");
    Ok(())
} 