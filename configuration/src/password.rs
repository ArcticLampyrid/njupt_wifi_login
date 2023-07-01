use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::serde_as;
use std::borrow::Cow;
use std::mem::MaybeUninit;
use std::ptr;
use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Security::Cryptography::CryptProtectData;
use windows::Win32::Security::Cryptography::CryptUnprotectData;
use windows::Win32::Security::Cryptography::CRYPTPROTECT_LOCAL_MACHINE;
use windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;
use windows::Win32::System::Memory::LocalFree;

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
    DataProtection {
        #[serde_as(as = "Base64")]
        data_protection: Vec<u8>,
    },
}

impl ToString for Password {
    fn to_string(&self) -> String {
        match self {
            Password::Basic(s) => s.clone(),
            Password::DataProtection { data_protection } => unsafe {
                let source = CRYPT_INTEGER_BLOB {
                    cbData: data_protection.len() as u32,
                    pbData: data_protection.as_ptr() as *mut u8,
                };
                let mut result = MaybeUninit::<CRYPT_INTEGER_BLOB>::uninit();
                CryptUnprotectData(
                    ptr::addr_of!(source),
                    None,
                    None,
                    None,
                    None,
                    0,
                    result.as_mut_ptr(),
                );
                let result = result.assume_init();
                let result_str = String::from_utf8_lossy(&*ptr::slice_from_raw_parts(
                    result.pbData,
                    result.cbData as usize,
                ))
                .to_string();
                let _ = LocalFree(HLOCAL(result.pbData as isize));
                return result_str;
            },
        }
    }
}

impl Password {
    pub fn new(s: String, scope: PasswordScope) -> Self {
        match scope {
            PasswordScope::Anywhere => Password::Basic(s),
            PasswordScope::LocalMachine => {
                Password::new_with_data_protection(s, CRYPTPROTECT_LOCAL_MACHINE)
            }
            PasswordScope::CurrentUser => Password::new_with_data_protection(s, 0),
        }
    }

    fn new_with_data_protection(s: String, flags: u32) -> Self {
        unsafe {
            let source = CRYPT_INTEGER_BLOB {
                cbData: s.len() as u32,
                pbData: s.as_ptr() as *mut u8,
            };
            let mut result = MaybeUninit::<CRYPT_INTEGER_BLOB>::uninit();
            CryptProtectData(
                ptr::addr_of!(source),
                None,
                None,
                None,
                None,
                flags,
                result.as_mut_ptr(),
            );
            let result = result.assume_init();
            let result_bytes =
                (&*ptr::slice_from_raw_parts(result.pbData, result.cbData as usize)).to_vec();
            let _ = LocalFree(HLOCAL(result.pbData as isize));
            Password::DataProtection {
                data_protection: result_bytes,
            }
        }
    }

    pub fn get(&self) -> Cow<'_, str> {
        match &self {
            Password::Basic(s) => Cow::Borrowed(s),
            _ => Cow::Owned(self.to_string()),
        }
    }
}
