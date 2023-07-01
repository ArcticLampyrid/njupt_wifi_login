use crate::password::Password;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IspType {
    EDU,
    CMCC,
    CT,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Credential {
    userid: String,
    password: Password,
    isp: IspType,
}

#[allow(dead_code)]
impl Credential {
    pub fn new(userid: String, password: Password, isp: IspType) -> Self {
        Credential {
            userid,
            password,
            isp,
        }
    }
    pub fn derive_account(&self) -> String {
        match self.isp {
            IspType::EDU => self.userid.clone(),
            IspType::CMCC => format!("{}@cmcc", self.userid),
            IspType::CT => format!("{}@njxy", self.userid),
        }
    }
    pub fn into_password(self) -> Password {
        return self.password;
    }
    pub fn into_userid(self) -> String {
        return self.userid;
    }
    pub fn userid(&self) -> &str {
        return &self.userid;
    }
    pub fn password(&self) -> &Password {
        return &self.password;
    }
    pub fn isp(&self) -> IspType {
        return self.isp;
    }
}
