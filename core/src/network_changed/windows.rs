use std::{ffi::c_void, ptr};

use windows::Win32::{
    Foundation::HANDLE,
    NetworkManagement::IpHelper::{CancelMibChangeNotify2, NotifyNetworkConnectivityHintChange},
    Networking::WinSock::NL_NETWORK_CONNECTIVITY_HINT,
};

#[must_use]
pub struct NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    _func: &'a F,
    handle: HANDLE,
}

impl<'a, F> NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    pub fn register(func: &'a F, initial_notification: bool) -> windows::core::Result<Self> {
        let mut handle = HANDLE::default();
        unsafe {
            NotifyNetworkConnectivityHintChange(
                Some(Self::on_network_connectivity_hint_changed),
                Some(func as *const F as *const c_void),
                initial_notification,
                ptr::addr_of_mut!(handle),
            )?;
        }
        Ok(Self {
            _func: func,
            handle,
        })
    }
    unsafe extern "system" fn on_network_connectivity_hint_changed(
        caller_context: *const c_void,
        connectivity_hint: NL_NETWORK_CONNECTIVITY_HINT,
    ) {
        let caller_context: &F = &*(caller_context as *const F);
        caller_context(connectivity_hint);
    }
}

impl<'a, F> Drop for NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
    }
}

#![cfg_attr(windows, windows_subsystem = "windows")]

mod login;
#[cfg_attr(windows, path = "network_changed/windows.rs")]
#[cfg_attr(target_os = "linux", path = "network_changed/linux.rs")]
mod network_changed;

use log::*;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, sync::Arc};
use thiserror::Error;
use tokio::sync::mpsc;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use windows::NetworkConnectivityHintChangedHandle;
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

const URL_GENERATE_204: &str = "http://connect.rom.miui.com/generate_204";

#[derive(Serialize, Deserialize, Debug)]
pub struct MyConfig {
    userid: String,
    password: String,
    isp: IspType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IspType {
    EDU,
    CMCC,
    CT,
}

#[derive(Error, Debug)]
pub enum WifiLoginError {
    #[error("network disconnected")]
    Disconnect(),
    #[error("http request failed: {0}")]
    HttpRequestFailed(#[from] reqwest::Error),
    #[error("authentication failed")]
    AuthenticationFailed(),
}

#[derive(Debug)]
pub enum ActionInfo {
    Login(),
}

fn read_my_config() -> Result<MyConfig, Box<dyn std::error::Error>> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let config: MyConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_log = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(LOG_PATH.as_path())
        .unwrap();

    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("file_log", Box::new(file_log)))
        .build(
            Root::builder()
                .appender("file_log")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    let _ = log4rs::init_config(log_config).unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel::<ActionInfo>();
    let my_config = read_my_config().unwrap_or_else(|error| {
        error!("Failed to read config: {}", error);
        panic!("{}", error)
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
            tx.send(ActionInfo::Login()).unwrap();
        }
    };
    let _network_connectivity_hint_changed_handle = NetworkConnectivityHintChangedHandle::register(
        &on_network_connectivity_hint_changed,
        true,
    )?;
    info!("Network connectivity hint changed notification registered");
    loop {
        match rx.recv().await {
            Some(ActionInfo::Login()) => {
                info!("Start to login");
                match login_wifi(
                    my_config.isp,
                    my_config.userid.as_str(),
                    my_config.password.as_str(),
                )
                .await
                {
                    Ok(_) => {
                        info!("Connected");
                    }
                    Err(err) => {
                        error!("Failed to connect: {}", err);
                    }
                };
            }
            None => break,
        }
    }
    Ok(())
}
