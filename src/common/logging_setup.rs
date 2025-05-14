use env_logger::Builder;
use log::LevelFilter;
use crate::config_loader::MasterConfig;
use anyhow::{Context, Result};

pub fn initialize_logging(config: Option<&MasterConfig>, cli_matches: &clap::ArgMatches) -> Result<()> {
    let mut builder = Builder::new();

    // Configure logger to include timestamps
    builder.format_timestamp_micros();

    // Determine log level from CLI, then config, then default
    let log_level_str = if cli_matches.get_flag("debug") {
        "debug".to_string()
    } else {
        config.map_or_else(
            || "info".to_string(), // Default if config is None
            |c| c.application.log_level.clone().unwrap_or_else(|| "info".to_string()) // Use log_level from config if Some, else default to "info"
        )
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

    builder.try_init().context("Failed to initialize logger")?;
    Ok(())
}