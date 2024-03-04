use crate::dns_resolver::CustomTrustDnsResolver;
use log::*;
use njupt_wifi_login_configuration::{credential::Credential, password::PasswordError};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use thiserror::Error;
use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts, ServerOrderingStrategy},
    system_conf,
};

static DNS_RESOLVER: Lazy<Arc<CustomTrustDnsResolver>> = Lazy::new(|| {
    let mut config = ResolverConfig::new();
    config.add_name_server(NameServerConfig::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
        Protocol::Udp,
    ));
    config.add_name_server(NameServerConfig::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(114, 114, 114, 114)), 53),
        Protocol::Udp,
    ));
    // fallback to system name servers
    if let Ok((system_conf, _)) = system_conf::read_system_conf() {
        system_conf.name_servers().iter().for_each(|name_server| {
            if !config.name_servers().iter().any(|ns| ns == name_server) {
                config.add_name_server(name_server.clone());
            }
        });
    }
    let mut opts = ResolverOpts::default();
    opts.server_ordering_strategy = ServerOrderingStrategy::UserProvidedOrder;
    Arc::new(CustomTrustDnsResolver::new(config, opts).unwrap())
});

const URL_GENERATE_204: &str = "http://connect.rom.miui.com/generate_204";
const URL_AP_PORTAL: &str = "https://p.njupt.edu.cn/a79.htm";
const ERROR_MSG_OFF_HOURS: &str = "Authentication Fail ErrCode=16";

static NJUPT_AUTHENTICATION_PATTERN: Lazy<regex::Regex> = Lazy::new(|| {
    Regex::new("Authentication is required\\. Click <a href=\"(.*?)\">here</a> to open the authentication page\\.").unwrap()
});

static AP_INFO_PATTERN: Lazy<regex::Regex> = Lazy::new(|| Regex::new("v46ip='(.*?)'").unwrap());

#[derive(Debug)]
pub struct ApInfo {
    pub user_ip: String,
}

#[derive(Debug)]
pub enum NetworkStatus {
    Connected,
    AuthenticationNJUPT(ApInfo),
    AuthenticationUnknown,
    Disconnected,
}

#[derive(Error, Debug)]
pub enum WifiLoginError {
    #[error("http request failed: {0}")]
    HttpRequestFailed(#[from] reqwest::Error),
    #[error("authentication failed")]
    AuthenticationFailed(),
    #[error("off hours")]
    OffHours(),
    #[error("authentication server rejected: {0}")]
    ServerRejected(String),
    #[error("failed to get password: {0}")]
    PasswordError(#[from] PasswordError),
}

#[derive(Serialize, Deserialize)]
struct NJUPTAuthenticationResult {
    result: i32,
    msg: String,
    ret_code: Option<i32>,
}

pub async fn get_network_status() -> NetworkStatus {
    let client_builder = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone());
    let client = match client_builder.build() {
        Ok(client) => client,
        Err(_) => return NetworkStatus::Disconnected,
    };
    let generate_204_page = match client.get(URL_GENERATE_204).send().await {
        Ok(generate_204_page) => generate_204_page,
        Err(_) => return NetworkStatus::Disconnected,
    };
    match generate_204_page.status() {
        reqwest::StatusCode::NO_CONTENT => {
            // Network has been available
            NetworkStatus::Connected
        }
        reqwest::StatusCode::OK => {
            let content = match generate_204_page.text().await {
                Ok(content) => content,
                Err(_) => return NetworkStatus::Disconnected,
            };
            if NJUPT_AUTHENTICATION_PATTERN.is_match(content.as_str()) {
                match get_ap_info(client).await {
                    Some(value) => NetworkStatus::AuthenticationNJUPT(value),
                    None => NetworkStatus::AuthenticationUnknown,
                }
            } else {
                NetworkStatus::AuthenticationUnknown
            }
        }
        reqwest::StatusCode::FOUND | reqwest::StatusCode::TEMPORARY_REDIRECT => {
            NetworkStatus::AuthenticationUnknown
        }
        _ => NetworkStatus::Disconnected,
    }
}

async fn get_ap_info(client: reqwest::Client) -> Option<ApInfo> {
    let ap_portal = match client.get(URL_AP_PORTAL).send().await {
        Ok(ap_portal) => ap_portal,
        Err(err) => {
            error!("Failed to get ap info: {}", err);
            return None;
        }
    };
    if ap_portal.status() != reqwest::StatusCode::OK {
        return None;
    }
    let ap_portal_content = match ap_portal.text().await {
        Ok(content) => content,
        Err(err) => {
            error!("Failed to decode ap portal data: {}", err);
            return None;
        }
    };
    match AP_INFO_PATTERN.captures(ap_portal_content.as_str()) {
        Some(captures) => {
            let ip = captures.get(1).map_or("", |m| m.as_str());
            Some(ApInfo {
                user_ip: ip.to_owned(),
            })
        }
        None => None,
    }
}

pub async fn send_login_request(
    credential: &Credential,
    ap_info: &ApInfo,
) -> Result<(), WifiLoginError> {
    let url = "https://p.njupt.edu.cn:802/eportal/portal/login";
    let ddddd = format!(",0,{}", credential.derive_account());
    let upass = credential.password().get()?;
    let params = [
        ("callback", "dr1003"),
        ("login_method", "1"),
        ("user_account", ddddd.as_ref()),
        ("user_password", upass.as_ref()),
        ("wlan_user_ip", ap_info.user_ip.as_ref()),
        ("wlan_user_ipv6", ""),
        ("wlan_user_mac", "000000000000"),
        ("wlan_ac_ip", ""),
        ("wlan_ac_name", ""),
        ("sVersion", "4.1.3"),
        ("terminal_type", "1"),
        ("lang", "zh-cn"),
        ("v", "3335"),
        ("lang", "zh"),
    ];
    let client = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone())
        .redirect(Policy::none())
        .build()?;
    let resp = client.get(url).query(&params).send().await?;
    if resp.status() == reqwest::StatusCode::OK {
        let content = resp.text().await?;
        if content.len() <= ("dr1003();".len()) {
            error!("Failed to parse authentication result: {}", content);
        } else {
            let json_content = &content[("dr1003(").len()..(content.len() - ");".len())];
            match serde_json::from_str::<NJUPTAuthenticationResult>(json_content) {
                Ok(result) => {
                    if result.result == 1 {
                        return Ok(());
                    }
                    if result.msg == ERROR_MSG_OFF_HOURS {
                        return Err(WifiLoginError::OffHours());
                    } else {
                        return Err(WifiLoginError::ServerRejected(result.msg));
                    }
                }
                Err(err) => {
                    error!(
                        "Failed to parse authentication result: {}, error: {}",
                        content, err
                    );
                }
            }
        }
    }
    if client.get(URL_GENERATE_204).send().await?.status() == reqwest::StatusCode::NO_CONTENT {
        // Fallback
        return Ok(());
    }
    Err(WifiLoginError::AuthenticationFailed())
}
