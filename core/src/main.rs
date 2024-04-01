#![windows_subsystem = "windows"]
mod app_events;
mod app_main;
mod app_service_events;
mod dns_resolver;
mod linux_network_listener;
mod login;
mod off_hours_cache;
mod smart_bind_to_interface_ext;
mod win32_network_connectivity_hint_changed;
use app_events::DefaultAppEvents;
use app_main::AppMain;
use clap::{Parser, Subcommand};
use log::*;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};
use njupt_wifi_login_configuration::login_config::LoginConfig;
use once_cell::sync::Lazy;
use std::env;
use std::path::PathBuf;
mod windows_service_command;
#[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
use windows_service_command::{handle_service_command, ServiceCommand};
static CONFIG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.yml");
    path
});
static LOG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.log");
    path
});

#[derive(Debug)]
pub enum ActionInfo {
    CheckAndLogin(),
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Provide more detailed log during execution.
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Windows service mode.
    #[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
    Service {
        #[clap(flatten)]
        args: ServiceCommand,
    },
}

fn read_my_config() -> Result<LoginConfig, Box<dyn std::error::Error>> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let config: LoginConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let args: Args = Args::parse();

    let log_level = if args.verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    let file_log = FileAppender::builder()
        .encoder(Box::<PatternEncoder>::default())
        .build(LOG_PATH.as_path())
        .unwrap();

    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("file_log", Box::new(file_log)))
        .build(Root::builder().appender("file_log").build(log_level))
        .unwrap();

    let _ = log4rs::init_config(log_config).unwrap();

    let my_config = read_my_config().unwrap_or_else(|error| {
        error!("Failed to read config: {}", error);
        panic!("{}", error)
    });

    match args.command {
        #[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
        Some(Command::Service { args }) => handle_service_command(args, my_config)?,
        _ => {
            let app = AppMain::new(my_config);
            app.run(DefaultAppEvents)?;
        }
    }
    Ok(())
}
