#![windows_subsystem = "windows"]
mod app_events;
mod app_main;
mod app_service_events;
mod dns;
mod linux_network_listener;
mod login;
mod off_hours_cache;
mod smart_bind_to_interface_ext;
mod win32_network_connectivity_hint_changed;
use app_events::DefaultAppEvents;
use app_main::AppMain;
use byte_unit::Byte;
use clap::{Parser, Subcommand};
use display_error_chain::ErrorChainExt;
use log::*;
use log4rs::{
    append::rolling_file::{
        policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        RollingFileAppender,
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};
use njupt_wifi_login_configuration::login_config::LoginConfig;
use std::env;
use std::path::{Path, PathBuf};
mod windows_service_command;
#[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
use windows_service_command::{handle_service_command, ServiceCommand};

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
    /// Set working directory
    #[arg(short('D'), long("directory"))]
    working_directory: Option<String>,
    /// Set configuration file
    #[arg(short, long, default_value = "njupt_wifi.yml")]
    config: String,
    /// Set log file
    #[arg(long, default_value = "njupt_wifi.log")]
    log_file: String,
    #[command(subcommand)]
    command: Option<Command>,
}

impl Args {
    pub fn path_of(&self, f: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        if let Some(working_directory) = self.working_directory.as_ref() {
            Ok(Path::new(working_directory).join(f))
        } else {
            let mut path = env::current_exe()?;
            path.pop();
            Ok(path.join(f))
        }
    }
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

fn read_my_config(args: &Args) -> Result<LoginConfig, Box<dyn std::error::Error + Sync + Send>> {
    let config_path = args.path_of(args.config.as_str())?;
    let f = std::fs::File::open(config_path)?;
    let config: LoginConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

fn init_log(
    args: &Args,
    config: &LoginConfig,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let log_level = if args.verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };
    let log_path = args.path_of(args.config.as_str())?;
    let rolling_pattern = log_path.as_path().to_string_lossy() + ".{}";
    let file_policy = CompoundPolicy::new(
        Box::new(SizeTrigger::new(
            config
                .log_policy
                .size_limit
                .unwrap_or(Byte::from_u64(3 * 1024 * 1024))
                .as_u64(),
        )),
        Box::new(FixedWindowRoller::builder().base(1).build(
            rolling_pattern.as_ref(),
            config.log_policy.file_count.unwrap_or(2),
        )?),
    );
    let file_log = RollingFileAppender::builder()
        .encoder(Box::<PatternEncoder>::default())
        .build(log_path, Box::new(file_policy))?;

    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("file_log", Box::new(file_log)))
        .build(Root::builder().appender("file_log").build(log_level))?;

    let _ = log4rs::init_config(log_config)?;
    Ok(())
}

fn windows_error_dialog(#[allow(unused)] error: &str) {
    #[cfg(windows)]
    {
        // For Windows, no console is available when subsystem is windows.
        // So we use MessageBoxW to show the error message.

        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::MessageBoxW;
        use windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR;
        use windows::Win32::UI::WindowsAndMessaging::MB_OK;

        unsafe {
            let caption: &'static [u16] = &[
                'E' as u16, 'r' as u16, 'r' as u16, 'o' as u16, 'r' as u16, 0,
            ];
            let message = error
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>();
            MessageBoxW(
                None,
                PCWSTR::from_raw(message.as_ptr()),
                PCWSTR::from_raw(caption.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            // Not necessary to print error chain here.
            windows_error_dialog(error.to_string().as_str());
            error.exit();
        }
    };

    let my_config = match read_my_config(&args) {
        Ok(config) => config,
        Err(error) => {
            windows_error_dialog(&format!(
                "Failed to read config: {}",
                error.as_ref().chain()
            ));
            eprintln!("Failed to read config.");
            return Err(error);
        }
    };

    if let Err(error) = init_log(&args, &my_config) {
        windows_error_dialog(&format!("Failed to init log: {}", error.as_ref().chain()));
        eprintln!("Failed to init log.");
        return Err(error);
    }

    let run: Result<(), Box<dyn std::error::Error + Sync + Send>> = match args.command {
        #[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
        Some(Command::Service { args }) => {
            handle_service_command(args, my_config).map_err(|e| e.into())
        }
        _ => {
            let app = AppMain::new(my_config);
            app.run(DefaultAppEvents)
        }
    };
    if let Err(error) = run {
        error!("Unhandled error: {}", error.as_ref().chain());
        return Err(error);
    }
    Ok(())
}
