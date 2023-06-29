use crate::credential::Credential;
use crate::dns_resolver::CustomTrustDnsResolver;
use log::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use thiserror::Error;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

static DNS_RESOLVER: Lazy<Arc<CustomTrustDnsResolver>> = Lazy::new(|| {
    Arc::new(
        CustomTrustDnsResolver::new(ResolverConfig::google(), ResolverOpts::default()).unwrap(),
    )
});

const URL_GENERATE_204: &str = "http://connect.rom.miui.com/generate_204";

#[derive(Error, Debug)]
pub enum WifiLoginError {
    #[error("network disconnected")]
    Disconnect(),
    #[error("http request failed: {0}")]
    HttpRequestFailed(#[from] reqwest::Error),
    #[error("authentication failed")]
    AuthenticationFailed(),
}

pub async fn login_wifi(credential: &Credential) -> Result<(), WifiLoginError> {
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
                    send_login_request(credential, ip, wlanacip, wlanacname).await?;
                    Ok(())
                }
                None => Err(WifiLoginError::AuthenticationFailed()),
            }
        }
        _ => Err(WifiLoginError::Disconnect()),
    }
}

async fn send_login_request(
    credential: &Credential,
    ip: &str,
    wlanacip: &str,
    wlanacname: &str,
) -> Result<(), WifiLoginError> {
    let url = format!("http://p.njupt.edu.cn:801/eportal/?c=ACSetting&a=Login&protocol=http:&hostname=p.njupt.edu.cn&iTermType=1&wlanuserip={}&wlanacip={}&wlanacname={}&mac=00-00-00-00-00-00&ip={}&enAdvert=0&queryACIP=0&loginMethod=1", ip, wlanacip, wlanacname, ip);
    let ddddd = format!(",0,{}", credential.derive_account());
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
        ("upass", credential.password()),
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
