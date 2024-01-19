#![windows_subsystem = "windows"]
mod dns_resolver;
mod login;
mod off_hours_cache;
mod win32_network_connectivity_hint_changed;
use crate::login::{get_network_status, send_login_request, WifiLoginError};
use crate::off_hours_cache::OffHoursCache;
use clap::Parser;
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
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use win32_network_connectivity_hint_changed::NetworkConnectivityHintChangedHandle;
use windows::Win32::Networking::WinSock::{
    NetworkConnectivityLevelHintConstrainedInternetAccess, NetworkConnectivityLevelHintLocalAccess,
    NL_NETWORK_CONNECTIVITY_HINT,
};
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

static OFF_HOURS_CACHE: Lazy<Mutex<OffHoursCache>> = Lazy::new(|| Mutex::new(OffHoursCache::new()));

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
}

fn read_my_config() -> Result<LoginConfig, Box<dyn std::error::Error>> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let config: LoginConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let (tx, mut rx) = mpsc::unbounded_channel::<ActionInfo>();
    let my_config = read_my_config().unwrap_or_else(|error| {
        error!("Failed to read config: {}", error);
        panic!("{}", error)
    });

    tokio::spawn({
        let tx = tx.clone();
        async move {
            tokio::time::sleep(Duration::from_secs(20 * 60)).await;
            loop {
                let expiration = OFF_HOURS_CACHE.lock().await.expiration();
                if expiration.is_zero() {
                    let _ = tx.send(ActionInfo::CheckAndLogin());
                    tokio::time::sleep(Duration::from_secs(20 * 60)).await;
                } else {
                    tokio::time::sleep(std::cmp::min(expiration, Duration::from_secs(20 * 60)))
                        .await;
                }
            }
        }
    });

    let on_network_connectivity_hint_changed = |connectivity_hint: NL_NETWORK_CONNECTIVITY_HINT| {
        info!(
            "ConnectivityLevel = {}",
            connectivity_hint.ConnectivityLevel.0
        );
        if connectivity_hint.ConnectivityLevel
            == NetworkConnectivityLevelHintConstrainedInternetAccess
            || connectivity_hint.ConnectivityLevel == NetworkConnectivityLevelHintLocalAccess
        {
            tx.send(ActionInfo::CheckAndLogin()).unwrap();
        }
    };
    let _network_connectivity_hint_changed_handle = NetworkConnectivityHintChangedHandle::register(
        &on_network_connectivity_hint_changed,
        true,
    )?;
    info!("Network connectivity hint changed notification registered");

    let mut last_check_at: Option<std::time::Instant> = None;

    while let Some(action) = rx.recv().await {
        match action {
            ActionInfo::CheckAndLogin() => {
                {
                    // debounce
                    let check_at = std::time::Instant::now();
                    if let Some(last_check_at) = last_check_at {
                        if check_at.duration_since(last_check_at) < Duration::from_secs(5) {
                            continue;
                        }
                    }
                    last_check_at = Some(check_at);
                }

                info!("Start to check network status");
                let network_status = get_network_status().await;
                info!("Network status: {:?}", network_status);
                if let login::NetworkStatus::AuthenticationNJUPT(ap_info) = network_status {
                    info!("Start to login");
                    match send_login_request(&my_config.credential, &ap_info).await {
                        Ok(_) => {
                            info!("Connected");
                            OFF_HOURS_CACHE.lock().await.clear();
                        }
                        Err(err) => {
                            error!("Failed to connect: {}", err);
                            if let WifiLoginError::OffHours() = err {
                                OFF_HOURS_CACHE.lock().await.set();
                            }
                        }
                    };
                }
            }
        }
    }
    Ok(())
}
