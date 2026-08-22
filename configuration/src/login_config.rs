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
    #[serde(default)]
    pub login_failure_backoff: LoginFailureBackoffConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginFailureBackoffConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "LoginFailureBackoffConfig::default_max_interval")]
    pub max_interval: u64,
    #[serde(default = "LoginFailureBackoffConfig::default_jitter_percent")]
    pub jitter_percent: u64,
}

impl Default for LoginFailureBackoffConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_interval: Self::default_max_interval(),
            jitter_percent: Self::default_jitter_percent(),
        }
    }
}

impl LoginFailureBackoffConfig {
    const fn default_max_interval() -> u64 {
        3600
    }

    const fn default_jitter_percent() -> u64 {
        50
    }
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
