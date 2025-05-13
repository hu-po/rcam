use crate::config_loader::MasterConfig;
use crate::core::camera_manager::CameraManager;
use crate::camera::camera_controller::CameraController;
use anyhow::{Result, Context, bail};
use crate::operations::op_helper::run_generic_camera_op;
use clap::ArgMatches;
use log::{info, error, debug};
use std::time::Instant;

pub async fn handle_control_camera_cli(
    master_config: &MasterConfig,
    camera_manager: &CameraManager,
    args: &ArgMatches,
) -> Result<()> {
    let op_start_time = Instant::now();
    let action_str = args.get_one::<String>("action")
        .context("Missing --action argument for control command")?;
    debug!("Control camera action: '{}', Cameras arg: {:?}", action_str, args.get_one::<String>("cameras"));
    
    let enable = match action_str.to_lowercase().as_str() {
        "enable" => true,
        "disable" => false,
        s => {
            error!("❌ Invalid action '{}'. Must be 'enable' or 'disable'.", s);
            bail!("Invalid action '{}'. Must be 'enable' or 'disable'.", s);
        }
    };
    let emoji = if enable { "💡" } else { "🔌" };
    info!("{} Preparing to {} cameras based on CLI arguments.", emoji, if enable {"enable"} else {"disable"});

    let controller_init_start = Instant::now();
    let camera_controller = CameraController::new();
    debug!("CameraController initialized for control operation in {:?}.", controller_init_start.elapsed());

    let result = run_generic_camera_op(
        master_config,
        camera_manager,
        args,
        "Camera Control",
        "output",
        None,
        move |cam_entity_arc, app_settings_arc, _operation_output_dir| {
            let controller_clone = camera_controller.clone();
            let enable_clone = enable;

            async move {
                let cam_op_start_time = Instant::now();
                let cam_entity = cam_entity_arc.lock().await;
                let cam_name = &cam_entity.config.name;
                let action_verb = if enable_clone { "enable" } else { "disable" };
                let op_emoji = if enable_clone { "💡" } else { "🔌" };
                
                info!("{} Attempting to {} camera: '{}'", op_emoji, action_verb, cam_name);

                match controller_clone.set_camera_enabled(&*cam_entity, &app_settings_arc, enable_clone).await {
                    Ok(()) => {
                        info!("✅ Successfully {}d camera '{}' in {:?}.", action_verb, cam_name, cam_op_start_time.elapsed());
                        Ok(())
                    }
                    Err(e) => {
                        error!("❌ Failed to {} camera '{}' after {:?}: {:#}", action_verb, cam_name, cam_op_start_time.elapsed(), e);
                        Err(e)
                    }
                }
            }
        },
    )
    .await;

    if result.is_ok() {
        info!("{} All camera control operations completed successfully in {:?}.", emoji, op_start_time.elapsed());
    } else {
        error!("{} Camera control operation failed after {:?}. See errors above.", emoji, op_start_time.elapsed());
    }
    result
} 