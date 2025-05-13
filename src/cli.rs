use clap::{Arg, Command, ArgAction};
use log::debug;
use std::time::Instant;

pub fn build_cli() -> Command {
    debug!("⚙️ Building CLI interface...");
    let start_time = Instant::now();
    let cmd = Command::new("rcam")
        .version("0.1.0")
        .author("RCam Developers")
        .about("A Rust application for recording images and videos from multiple IP cameras.")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom configuration file")
                .action(ArgAction::Set)
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .help("Enable debug logging")
                .action(ArgAction::SetTrue)
        )
        .subcommand(
            Command::new("capture-image")
                .about("Captures a single image from specified or all cameras")
                .arg(Arg::new("cameras").long("cameras").value_name("CAM_NAMES").help("Comma-separated list of camera names to capture from (default: all)").action(ArgAction::Set))
                .arg(Arg::new("delay").long("delay").value_name("SECONDS").help("Delay in seconds before capturing image").value_parser(clap::value_parser!(u64)).action(ArgAction::Set))
                .arg(Arg::new("output").short('o').long("output").value_name("DIR").help("Output directory for images").action(ArgAction::Set))
                .arg(Arg::new("rerun").long("rerun").help("Enable Rerun logging for this capture").action(ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("capture-video")
                .about("Records a video segment from specified or all cameras")
                .arg(Arg::new("cameras").long("cameras").value_name("CAM_NAMES").help("Comma-separated list of camera names to record from (default: all)").action(ArgAction::Set))
                .arg(Arg::new("duration").long("duration").value_name("SECONDS").help("Duration of the video recording in seconds").value_parser(clap::value_parser!(u64)).action(ArgAction::Set))
                .arg(Arg::new("output").short('o').long("output").value_name("DIR").help("Output directory for videos").action(ArgAction::Set))
                .arg(Arg::new("rerun").long("rerun").help("Enable Rerun logging for this recording").action(ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("verify-times")
                .about("Verifies time synchronization across all cameras")
        )
        .subcommand(
            Command::new("control")
                .about("Controls camera functionalities")
                .arg(Arg::new("action").long("action").value_name("ACTION").required(true).help("Action to perform: 'enable' or 'disable'").action(ArgAction::Set))
                .arg(Arg::new("cameras").long("cameras").value_name("CAM_NAMES").help("Comma-separated list of camera names to control (default: all)").action(ArgAction::Set))
        )
        .subcommand(
            Command::new("test")
                .about("Runs a diagnostic test suite")
        );
    debug!("✅ CLI interface built in {:?}", start_time.elapsed());
    cmd
} 