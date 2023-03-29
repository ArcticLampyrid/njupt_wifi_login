#![windows_subsystem = "windows"]

mod dns_resolver;
mod win32_network_connectivity_hint_changed;
mod login;
mod network_changed_event;

use dns_resolver::CustomTrustDnsResolver;
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
static DNS_RESOLVER: Lazy<Arc<CustomTrustDnsResolver>> = Lazy::new(|| {
    Arc::new(
        CustomTrustDnsResolver::new(ResolverConfig::google(), ResolverOpts::default()).unwrap(),
    )
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

async fn login_wifi(isp: IspType, userid: &str, password: &str) -> Result<(), WifiLoginError> {
    let actual_userid = match isp {
        IspType::EDU => format!(",0,{}", userid),
        IspType::CMCC => format!(",0,{}@cmcc", userid),
        IspType::CT => format!(",0,{}@njxy", userid),
    };
    let dormitory_pattern: regex::Regex =
        Regex::new("ip=(.*?)&wlanacip=(.*?)&wlanacname=(.*?)\"").unwrap();
    let library_pattern: regex::Regex = Regex::new("UserIP=(.*?)&wlanacname=(.*?)&(.*?)=").unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone())
        .build()?;
    let login_page = client.get(URL_GENERATE_204).send().await?;
    match login_page.status() {
        reqwest::StatusCode::NO_CONTENT => {
            // Network has been available
            Ok(())
        }
        reqwest::StatusCode::OK => {
            let content = login_page.text().await?;
            let captures_box = dormitory_pattern
                .captures(content.as_str())
                .or_else(|| library_pattern.captures(content.as_str()));
            match captures_box {
                Some(captures) => {
                    let ip = captures.get(1).map_or("", |m| m.as_str());
                    let wlanacip = captures.get(2).map_or("", |m| m.as_str());
                    let wlanacname = captures.get(3).map_or("", |m| m.as_str());
                    send_login_request(actual_userid.as_str(), password, ip, wlanacip, wlanacname)
                        .await?;
                    Ok(())
                }
                None => Err(WifiLoginError::AuthenticationFailed()),
            }
        }
        _ => Err(WifiLoginError::Disconnect()),
    }
}

async fn send_login_request(
    userid: &str,
    password: &str,
    ip: &str,
    wlanacip: &str,
    wlanacname: &str,
) -> Result<(), WifiLoginError> {
    let url = format!("http://p.njupt.edu.cn:801/eportal/?c=ACSetting&a=Login&protocol=http:&hostname=p.njupt.edu.cn&iTermType=1&wlanuserip={}&wlanacip={}&wlanacname={}&mac=00-00-00-00-00-00&ip={}&enAdvert=0&queryACIP=0&loginMethod=1", ip, wlanacip, wlanacname, ip);
    let params = [
        ("R1", "0"),
        ("R2", "0"),
        ("R3", "0"),
        ("R6", "0"),
        ("para", "0"),
        ("0MKKey", "123456"),
        ("buttonClicked", ""),
        ("redirect_url", ""),
        ("err_flag", ""),
        ("username", ""),
        ("password", ""),
        ("user", ""),
        ("cmd", ""),
        ("Login", ""),
        ("v6ip", ""),
        ("DDDDD", userid),
        ("upass", password),
    ];
    let client = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone())
        .build()?;
    let resp = client.post(url).form(&params).send().await?;
    if resp.status().is_success() && resp.text().await?.contains("成功") {
        return Ok(());
    }
    if client.get(URL_GENERATE_204).send().await?.status() == reqwest::StatusCode::NO_CONTENT {
        // Fallback
        return Ok(());
    }
    Err(WifiLoginError::AuthenticationFailed())
}
