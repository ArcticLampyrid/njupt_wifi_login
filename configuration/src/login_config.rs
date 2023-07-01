use crate::credential::Credential;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
pub struct LoginConfig {
    #[serde(flatten)]
    pub credential: Credential,
}
