#![cfg(not(target_os = "windows"))]
use chacha20poly1305::aead::{Aead, Generate, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use hex::{FromHex, ToHex};
use machine_uid::machine_id::get_machine_id;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug)]
pub struct LocalMachineDataProtection {
    nonce: Vec<u8>,
    secret: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum LocalMachineDataProtectionError {
    #[error("machine id error: {message}")]
    MachineIdError { message: String },
    #[error("aead error")]
    AeadError,
}

impl LocalMachineDataProtection {
    pub fn protect(s: &[u8]) -> Result<Self, LocalMachineDataProtectionError> {
        let machine_id = Sha256::digest(
            get_machine_id()
                .map_err(|e| LocalMachineDataProtectionError::MachineIdError {
                    message: e.to_string(),
                })?
                .into_bytes(),
        );
        let cipher = ChaCha20Poly1305::new(
            machine_id
                .as_slice()
                .try_into()
                .map_err(|_| LocalMachineDataProtectionError::AeadError)?,
        );
        let nonce = Nonce::generate();
        let ciphertext = cipher
            .encrypt(&nonce, s)
            .map_err(|_| LocalMachineDataProtectionError::AeadError)?;
        Ok(Self {
            nonce: nonce.to_vec(),
            secret: ciphertext.to_vec(),
        })
    }

    pub fn unprotect(&self) -> Result<Vec<u8>, LocalMachineDataProtectionError> {
        let machine_id = Sha256::digest(
            get_machine_id()
                .map_err(|e| LocalMachineDataProtectionError::MachineIdError {
                    message: e.to_string(),
                })?
                .into_bytes(),
        );
        let cipher = ChaCha20Poly1305::new(
            machine_id
                .as_slice()
                .try_into()
                .map_err(|_| LocalMachineDataProtectionError::AeadError)?,
        );
        cipher
            .decrypt(
                self.nonce
                    .as_slice()
                    .try_into()
                    .map_err(|_| LocalMachineDataProtectionError::AeadError)?,
                Payload::from(self.secret.as_ref()),
            )
            .map_err(|_| LocalMachineDataProtectionError::AeadError)
    }
}

impl Serialize for LocalMachineDataProtection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut data = "v1$m$".to_string();
        data.push_str(self.nonce.encode_hex::<String>().as_str());
        data.push('$');
        data.push_str(self.secret.encode_hex::<String>().as_str());
        serializer.serialize_str(data.as_str())
    }
}

impl<'de> Deserialize<'de> for LocalMachineDataProtection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = String::deserialize(deserializer)?;
        let parts: Vec<&str> = bytes.split('$').collect();
        if parts.len() != 4 || parts[0] != "v1" || parts[1] != "m" {
            return Err(serde::de::Error::custom("invalid format"));
        }
        let nonce = Vec::from_hex(parts[2]).map_err(serde::de::Error::custom)?;
        let secret = Vec::from_hex(parts[3]).map_err(serde::de::Error::custom)?;
        Ok(Self { nonce, secret })
    }
}
