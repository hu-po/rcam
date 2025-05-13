use env_logger::Builder;
use log::LevelFilter;
use crate::config_loader::MasterConfig;
// We might not need cli::build_cli() here if we pass matches directly

pub fn initialize_logging(config: Option<&MasterConfig>, cli_matches: &clap::ArgMatches) {
    let mut builder = Builder::new();

    // Determine log level from CLI, then config, then default
    let log_level_str = if cli_matches.get_flag("debug") {
        "debug".to_string()
    } else {
        config
            .and_then(|c| c.app_settings.log_level.clone())
            .unwrap_or_else(|| "info".to_string()) // Default log level
    };

    match log_level_str.to_lowercase().as_str() {
        "error" => builder.filter_level(LevelFilter::Error),
        "warn" => builder.filter_level(LevelFilter::Warn),
        "info" => builder.filter_level(LevelFilter::Info),
        "debug" => builder.filter_level(LevelFilter::Debug),
        "trace" => builder.filter_level(LevelFilter::Trace),
        s => {
            log::warn!("Unrecognized log level '{}', defaulting to info.", s);
            builder.filter_level(LevelFilter::Info)
        }
    };

    // Customize log format (optional) - example from before
    // builder.format(|buf, record| {
    //     use std::io::Write;
    //     writeln!(
    //         buf,
    //         "{} [{}] - {}",
    //         chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
    //         record.level(),
    //         record.args()
    //     )
    // });

    builder.try_init().unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Logging might not work as expected.", e);
    });
    // Note: We can't use log::info here before logger is fully confirmed to be working,
    // or if try_init fails. The message below might not appear if init fails.
    // Consider a simple println if robust startup message is needed even on logger fail.
    // log::info!("Logging initialized with level: {}", log_level_str);
}

// A simpler init function if you don't want to pass full config and CLI matches around initially.
// This is what was in main.rs initially, using env var for quick setup.
pub fn basic_env_logging_init() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
} 