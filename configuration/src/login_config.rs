use crate::credential::Credential;
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
}

const fn default_check_interval() -> u64 {
    20 * 60
}
