use crate::credential::Credential;
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
pub struct LoginConfig {
    #[serde(flatten)]
    pub credential: Credential,
    #[serde(default = "default_check_interval")]
    pub check_interval: u64,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
    #[serde(default)]
    pub log_policy: LogFileConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct LogFileConfig {
    #[serde(default)]
    pub size_limit: Option<Byte>,
    #[serde(default)]
    pub file_count: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]

pub struct SecurityConfig {
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,
}

const fn default_check_interval() -> u64 {
    20 * 60
}
