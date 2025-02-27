use crate::{
    dns::resolver::CustomTrustDnsResolver, smart_bind_to_interface_ext::SmartBindToInterfaceExt,
};
use display_error_chain::ErrorChainExt;
use hickory_resolver::config::{
    NameServerConfig, Protocol, ResolverConfig, ResolverOpts, ServerOrderingStrategy,
};
use log::*;
use njupt_wifi_login_configuration::{credential::Credential, password::PasswordError};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{
    dns::{Addrs, Name, Resolve},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};
use thiserror::Error;

// Use multiple URLs here, so any of them won't be overloaded easily.
const URLS_CONNECTIVITY_CHECK_204: [&str; 3] = [
    "http://connect.rom.miui.com/generate_204",
    "http://connectivitycheck.platform.hicloud.com/generate_204",
    "http://wifi.vivo.com.cn/generate_204",
];
const URL_AP_PORTAL: &str = "https://p.njupt.edu.cn/a79.htm";
const AP_PORTAL_FALLBACK_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 10, 244, 11));
const POSSIBLE_MSGS_OFF_HOURS: [&str; 2] = [
    // Confirmed on 2023-07-24
    "Authentication Fail ErrCode=16",
    // Confirmed on 2025-02-26
    "当前时间禁止上网",
];

static NJUPT_AUTHENTICATION_PATTERN: Lazy<regex::Regex> = Lazy::new(|| {
    Regex::new("Authentication is required\\. Click <a href=\"(.*?)\">here</a> to open the authentication page\\.").unwrap()
});

static AP_INFO_PATTERN: Lazy<regex::Regex> = Lazy::new(|| Regex::new("v46ip='(.*?)'").unwrap());

static CONNECTIVITY_CHECK_204_LOAD_BALANCE: AtomicUsize = AtomicUsize::new(0);

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
    #[error("http request failed")]
    HttpRequestFailed(#[from] reqwest::Error),
    #[error("authentication failed")]
    AuthenticationFailed(),
    #[error("off hours")]
    OffHours(),
    #[error("authentication server rejected: {0}")]
    ServerRejected(String),
    #[error("failed to get password")]
    PasswordError(#[from] PasswordError),
    #[error("failed to bind to interface")]
    BindToInterfaceError(#[from] crate::smart_bind_to_interface_ext::SmartBindToInterfaceError),
}

#[derive(Serialize, Deserialize)]
struct NJUPTAuthenticationResult {
    result: i32,
    msg: String,
    ret_code: Option<i32>,
}

pub fn new_dns_resolver(interface: Option<String>) -> Arc<CustomTrustDnsResolver> {
    let mut config = ResolverConfig::new();

    let ns = NameServerConfig::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
        Protocol::Udp,
    );
    config.add_name_server(ns);

    let ns = NameServerConfig::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(114, 114, 114, 114)), 53),
        Protocol::Udp,
    );
    config.add_name_server(ns);

    let mut opts = ResolverOpts::default();
    opts.server_ordering_strategy = ServerOrderingStrategy::QueryStatistics;
    Arc::new(
        CustomTrustDnsResolver::new(interface, config, opts, |name: &Name| -> Option<Addrs> {
            if name.as_str() == "p.njupt.edu.cn" {
                return Some(Box::new(
                    vec![SocketAddr::new(AP_PORTAL_FALLBACK_IP, 0)].into_iter(),
                ));
            }
            None
        })
        .unwrap(),
    )
}

pub fn random_url_for_connectivity_check_204() -> &'static str {
    let index = CONNECTIVITY_CHECK_204_LOAD_BALANCE
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        % URLS_CONNECTIVITY_CHECK_204.len();
    URLS_CONNECTIVITY_CHECK_204[index]
}

pub async fn get_network_status(
    interface: Option<&str>,
    dns_resolver: Arc<impl Resolve + 'static>,
) -> Result<NetworkStatus, WifiLoginError> {
    // Use public connectivity check page to determine network status,
    // which prevents exposing the school if not in the campus network.
    // What's more, it will minimize the network traffic to campus portal,
    // which is fragile and slow.
    let client_builder = reqwest::Client::builder()
        .optional_smart_bind_to_interface(interface)?
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .dns_resolver(dns_resolver);
    let client = client_builder.build()?;
    let generate_204_page = match client
        .get(random_url_for_connectivity_check_204())
        .send()
        .await
    {
        Ok(generate_204_page) => generate_204_page,
        Err(_) => return Ok(NetworkStatus::Disconnected),
    };
    match generate_204_page.status() {
        reqwest::StatusCode::NO_CONTENT => {
            // Network has been available
            Ok(NetworkStatus::Connected)
        }
        reqwest::StatusCode::OK => {
            let content = match generate_204_page.text().await {
                Ok(content) => content,
                Err(_) => return Ok(NetworkStatus::Disconnected),
            };
            if NJUPT_AUTHENTICATION_PATTERN.is_match(content.as_str()) {
                match get_ap_info(client).await {
                    Some(value) => Ok(NetworkStatus::AuthenticationNJUPT(value)),
                    None => Ok(NetworkStatus::AuthenticationUnknown),
                }
            } else {
                Ok(NetworkStatus::AuthenticationUnknown)
            }
        }
        reqwest::StatusCode::FOUND | reqwest::StatusCode::TEMPORARY_REDIRECT => {
            Ok(NetworkStatus::AuthenticationUnknown)
        }
        _ => Ok(NetworkStatus::Disconnected),
    }
}

async fn get_ap_info(client: reqwest::Client) -> Option<ApInfo> {
    let ap_portal = match client.get(URL_AP_PORTAL).send().await {
        Ok(ap_portal) => ap_portal,
        Err(err) => {
            error!("Failed to get ap info: {}", err.chain());
            return None;
        }
    };
    if ap_portal.status() != reqwest::StatusCode::OK {
        return None;
    }
    let ap_portal_content = match ap_portal.text().await {
        Ok(content) => content,
        Err(err) => {
            error!("Failed to decode ap portal data: {}", err.chain());
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
    interface: Option<&str>,
    dns_resolver: Arc<impl Resolve + 'static>,
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
        .optional_smart_bind_to_interface(interface)?
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .dns_resolver(dns_resolver)
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
                    return if result.result == 1 {
                        Ok(())
                    } else if POSSIBLE_MSGS_OFF_HOURS.contains(&result.msg.as_str()) {
                        Err(WifiLoginError::OffHours())
                    } else {
                        Err(WifiLoginError::ServerRejected(result.msg))
                    }
                }
                Err(err) => {
                    error!(
                        "Failed to parse authentication result: {}, error: {}",
                        content,
                        err.chain()
                    );
                }
            }
        }
    }
    if client
        .get(random_url_for_connectivity_check_204())
        .send()
        .await?
        .status()
        == reqwest::StatusCode::NO_CONTENT
    {
        // Fallback
        return Ok(());
    }
    Err(WifiLoginError::AuthenticationFailed())
}
