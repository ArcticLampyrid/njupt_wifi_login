use crate::dns_resolver::CustomTrustDnsResolver;
use base64::Engine;
use log::*;
use njupt_wifi_login_configuration::credential::Credential;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{redirect::Policy, Url};
use std::{sync::Arc, vec};
use thiserror::Error;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

static DNS_RESOLVER: Lazy<Arc<CustomTrustDnsResolver>> = Lazy::new(|| {
    Arc::new(
        CustomTrustDnsResolver::new(ResolverConfig::google(), ResolverOpts::default()).unwrap(),
    )
});

const URL_GENERATE_204: &str = "http://connect.rom.miui.com/generate_204";
const ERROR_MSG_OFF_HOURS: &str = "QXV0aGVudGljYXRpb24gRmFpbCBFcnJDb2RlPTE2";

const AP_INFO_PATTERNS: Lazy<Vec<regex::Regex>> = Lazy::new(|| {
    vec![
        Regex::new("ip=(.*?)&wlanacip=(.*?)&wlanacname=(.*?)\"").unwrap(),
        Regex::new("UserIP=(.*?)&wlanacname=(.*?)&(.*?)=").unwrap(),
    ]
});

#[derive(Debug)]
pub struct ApInfo {
    pub user_ip: String,
    pub ac_ip: String,
    pub ac_name: String,
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
}

pub async fn get_network_status() -> NetworkStatus {
    let client_builder = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone());
    let client = match client_builder.build() {
        Ok(client) => client,
        Err(_) => return NetworkStatus::Disconnected,
    };
    let login_page = match client.get(URL_GENERATE_204).send().await {
        Ok(login_page) => login_page,
        Err(_) => return NetworkStatus::Disconnected,
    };
    match login_page.status() {
        reqwest::StatusCode::NO_CONTENT => {
            // Network has been available
            NetworkStatus::Connected
        }
        reqwest::StatusCode::OK => {
            let content = match login_page.text().await {
                Ok(content) => content,
                Err(_) => return NetworkStatus::Disconnected,
            };
            let captures_box = AP_INFO_PATTERNS
                .iter()
                .find_map(|pattern| pattern.captures(content.as_str()));
            match captures_box {
                Some(captures) => {
                    let ip = captures.get(1).map_or("", |m| m.as_str());
                    let wlanacip = captures.get(2).map_or("", |m| m.as_str());
                    let wlanacname = captures.get(3).map_or("", |m| m.as_str());
                    NetworkStatus::AuthenticationNJUPT(ApInfo {
                        user_ip: ip.to_owned(),
                        ac_ip: wlanacip.to_owned(),
                        ac_name: wlanacname.to_owned(),
                    })
                }
                None => NetworkStatus::AuthenticationUnknown,
            }
        }
        reqwest::StatusCode::FOUND | reqwest::StatusCode::TEMPORARY_REDIRECT => {
            NetworkStatus::AuthenticationUnknown
        }
        _ => NetworkStatus::Disconnected,
    }
}

pub async fn send_login_request(
    credential: &Credential,
    ap_info: &ApInfo,
) -> Result<(), WifiLoginError> {
    let url = format!("http://p.njupt.edu.cn:801/eportal/?c=ACSetting&a=Login&protocol=http:&hostname=p.njupt.edu.cn&iTermType=1&wlanuserip={}&wlanacip={}&wlanacname={}&mac=00-00-00-00-00-00&ip={}&enAdvert=0&queryACIP=0&loginMethod=1", ap_info.user_ip, ap_info.ac_ip, ap_info.ac_name, ap_info.user_ip);
    let ddddd = format!(",0,{}", credential.derive_account());
    let upass = credential.password().get();
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
        ("DDDDD", ddddd.as_ref()),
        ("upass", upass.as_ref()),
    ];
    let client = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(DNS_RESOLVER.clone())
        .redirect(Policy::none())
        .build()?;
    let resp = client.post(url).form(&params).send().await?;
    match resp.status() {
        reqwest::StatusCode::FOUND => {
            let url = resp
                .headers()
                .get("Location")
                .and_then(|location| location.to_str().ok())
                .and_then(|location| Url::parse(location).ok());
            let url = match url {
                Some(url) => url,
                None => return Err(WifiLoginError::AuthenticationFailed()),
            };
            let error_msg = url
                .query_pairs()
                .find(|(key, value)| key == "ErrorMsg" && !value.is_empty())
                .map(|(_, value)| value);
            match error_msg {
                Some(error_msg) => {
                    if error_msg == ERROR_MSG_OFF_HOURS {
                        return Err(WifiLoginError::OffHours());
                    } else {
                        let decoded_error_msg = base64::engine::general_purpose::STANDARD
                            .decode(error_msg.as_ref())
                            .ok()
                            .and_then(|x| String::from_utf8(x).ok())
                            .unwrap_or_else(|| error_msg.as_ref().to_owned());
                        return Err(WifiLoginError::ServerRejected(decoded_error_msg));
                    }
                }
                None => return Ok(()),
            }
        }
        _ => {
            if client.get(URL_GENERATE_204).send().await?.status()
                == reqwest::StatusCode::NO_CONTENT
            {
                // Fallback
                return Ok(());
            }
        }
    }
    Err(WifiLoginError::AuthenticationFailed())
}
