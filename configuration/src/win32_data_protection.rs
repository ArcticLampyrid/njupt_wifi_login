#![cfg(target_os = "windows")]
use serde::Deserialize;
use serde::Serialize;
use std::mem::MaybeUninit;
use std::ptr;
use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Security::Cryptography::CryptProtectData;
use windows::Win32::Security::Cryptography::CryptUnprotectData;
use windows::Win32::Security::Cryptography::CRYPTPROTECT_LOCAL_MACHINE;
use windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;
use windows::Win32::System::Memory::LocalFree;
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Win32ProtectedData {
    data: Vec<u8>,
}
impl From<Vec<u8>> for Win32ProtectedData {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}
impl AsRef<[u8]> for Win32ProtectedData {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl Win32ProtectedData {
    pub fn protect_for_local_machine(s: &[u8]) -> windows::core::Result<Self> {
        Self::protect_with_flag(s, CRYPTPROTECT_LOCAL_MACHINE)
    }
    pub fn protect_for_current_user(s: &[u8]) -> windows::core::Result<Self> {
        Self::protect_with_flag(s, 0)
    }
    fn protect_with_flag(s: &[u8], flags: u32) -> windows::core::Result<Self> {
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
            )
            .ok()?;
            let result = result.assume_init();
            let result_bytes =
                std::slice::from_raw_parts(result.pbData, result.cbData as usize).to_vec();
            let _ = LocalFree(HLOCAL(result.pbData as isize));
            Ok(Self { data: result_bytes })
        }
    }
    pub fn unprotect(&self) -> windows::core::Result<Vec<u8>> {
        unsafe {
            let source = CRYPT_INTEGER_BLOB {
                cbData: self.data.len() as u32,
                pbData: self.data.as_ptr() as *mut u8,
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
            )
            .ok()?;
            let result = result.assume_init();
            let result_bytes =
                std::slice::from_raw_parts(result.pbData, result.cbData as usize).to_owned();
            let _ = LocalFree(HLOCAL(result.pbData as isize));
            Ok(result_bytes)
        }
    }
}
