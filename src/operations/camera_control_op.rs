use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_controller::CameraController;
use crate::errors::AppError;
use crate::operations::op_helper::run_generic_camera_op;
use clap::ArgMatches;
use log::{info, error};

pub async fn handle_control_camera_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<(), AppError> {
    let action_str = args.get_one::<String>("action").ok_or_else(|| {
        AppError::Operation("Missing --action argument for control command".to_string())
    })?;
    let enable = match action_str.to_lowercase().as_str() {
        "enable" => true,
        "disable" => false,
        _ => {
            return Err(AppError::Operation(format!(
                "Invalid action '{}'. Must be 'enable' or 'disable'.",
                action_str
            )))
        }
    };

    let camera_controller = CameraController::new();

    run_generic_camera_op(
        master_config,
        camera_manager,
        args,
        "Camera Control",
        "output",
        None,
        move |cam_entity_arc, _app_settings_arc, _operation_output_dir| {
            let controller_clone = camera_controller.clone();
            let enable_clone = enable;

            async move {
                let cam_entity = cam_entity_arc.lock().await;
                let cam_name = &cam_entity.config.name;
                let action_verb = if enable_clone { "enable" } else { "disable" };
                
                info!("Attempting to {} camera: '{}'", action_verb, cam_name);

                match controller_clone.set_camera_enabled(&*cam_entity, enable_clone).await {
                    Ok(()) => {
                        info!("Successfully {}d camera '{}'", action_verb, cam_name);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to {} camera '{}': {}", action_verb, cam_name, e);
                        Err(e)
                    }
                }
            }
        },
    )
    .await
} 