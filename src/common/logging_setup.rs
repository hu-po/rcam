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

    builder.try_init().unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Logging might not work as expected.", e);
    });
}