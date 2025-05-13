use crate::core::camera_manager::{CameraManager, parse_camera_names_arg};
use crate::camera::camera_controller::CameraController;
use crate::errors::AppError;
use clap::ArgMatches;
use log::{info, error, warn};
use futures::future::join_all;

pub async fn handle_control_camera_cli(
    _master_config: &crate::config_loader::MasterConfig, // May not be needed directly, but good to have access
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<(), AppError> {
    info!("Handling camera-control command...");

    let action_str = args.get_one::<String>("action").ok_or_else(|| AppError::Operation("Missing --action argument for control command".to_string()))?;
    let enable = match action_str.to_lowercase().as_str() {
        "enable" => true,
        "disable" => false,
        _ => return Err(AppError::Operation(format!("Invalid action '{}'. Must be 'enable' or 'disable'.", action_str))),
    };

    let camera_controller = CameraController::new();

    let specific_cameras_arg = args.get_one::<String>("cameras");
    let camera_names_to_process = parse_camera_names_arg(specific_cameras_arg);

    let cameras_to_target = match camera_names_to_process {
        Some(ref names) => camera_manager.get_cameras_by_names(names).await,
        None => camera_manager.get_all_cameras().await,
    };

    if cameras_to_target.is_empty() {
        if let Some(names) = camera_names_to_process {
            warn!("No cameras found matching names: {:?}. Please check your camera names and configuration.", names);
        } else {
            warn!("No cameras configured or matched for control operation.");
        }
        return Ok(());
    }

    let mut control_tasks = Vec::new();

    for cam_entity_arc in cameras_to_target {
        let controller_clone = camera_controller.clone(); // Assuming CameraController is Clone or Arc-able
        // let cam_name_clone = cam_entity_arc.lock().await.config.name.clone(); // Done in task

        let task = tokio::spawn(async move {
            let cam_entity = cam_entity_arc.lock().await;
            info!("Attempting to {} camera: '{}'", if enable {"enable"} else {"disable"}, cam_entity.config.name);
            match controller_clone.set_camera_enabled(&*cam_entity, enable).await {
                Ok(()) => info!("Successfully {}d camera '{}'", if enable {"enable"} else {"disable"}, cam_entity.config.name),
                Err(e) => error!("Failed to {} camera '{}': {}", if enable {"enable"} else {"disable"}, cam_entity.config.name, e),
            }
        });
        control_tasks.push(task);
    }

    join_all(control_tasks).await;
    info!("Camera control tasks completed.");

    Ok(())
} 