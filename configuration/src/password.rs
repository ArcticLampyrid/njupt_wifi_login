use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::borrow::Cow;
use thiserror::Error;

#[cfg(target_os = "windows")]
use crate::win32_data_protection::Win32ProtectedData;

#[derive(Serialize, Deserialize, Debug)]
pub enum PasswordScope {
    Anywhere,
    LocalMachine,
    CurrentUser,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Password {
    Basic(String),
    #[cfg(target_os = "windows")]
    DataProtection {
        #[serde_as(as = "serde_with::base64::Base64")]
        data_protection: Win32ProtectedData,
    },
}

#[derive(Error, Debug)]
pub enum PasswordError {
    #[cfg(target_os = "windows")]
    #[error("win32 cryptography error: {0}")]
    Win32CryptographyError(#[from] windows::core::Error),
    #[error("scope not supported")]
    ScopeNotSupported(PasswordScope),
}

impl ToString for Password {
    fn to_string(&self) -> String {
        match self {
            Password::Basic(s) => s.clone(),
            #[cfg(target_os = "windows")]
            Password::DataProtection { data_protection } => {
                String::from_utf8(data_protection.unprotect()).unwrap_or_default()
            }
        }
    }
}

impl Password {
    pub fn new_basic(s: String) -> Self {
        Password::Basic(s)
    }

    pub fn try_new(s: String, scope: PasswordScope) -> Result<Self, PasswordError> {
        match scope {
            PasswordScope::Anywhere => Ok(Password::Basic(s)),
            #[cfg(target_os = "windows")]
            PasswordScope::LocalMachine => Ok(Password::DataProtection {
                data_protection: Win32ProtectedData::protect_for_local_machine(s.as_bytes())?,
            }),
            #[cfg(target_os = "windows")]
            PasswordScope::CurrentUser => Ok(Password::DataProtection {
                data_protection: Win32ProtectedData::protect_for_current_user(s.as_bytes())?,
            }),
            #[allow(unreachable_patterns)]
            _ => Err(PasswordError::ScopeNotSupported(scope)),
        }
    }

    pub fn get(&self) -> Cow<'_, str> {
        match &self {
            Password::Basic(s) => Cow::Borrowed(s),
            #[allow(unreachable_patterns)]
            _ => Cow::Owned(self.to_string()),
        }
    }
}
