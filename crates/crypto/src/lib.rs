use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use anyhow::{Result, anyhow};
use tracing::error;
use zerocopy::{FromBytes, IntoBytes};

use crate::storage::{HEADER_LEN, VaultHeader};

pub mod meta {
    pub static NAME: &str = env!("CARGO_PKG_NAME");
    pub static VERSION: &str = env!("CARGO_PKG_VERSION");
}

pub mod argon2;
mod storage;

const SALT_LENGTH: usize = 32;
pub type Salt = [u8; 32];

const NONCE_LENGTH: usize = 12;
pub type Nonce = [u8; NONCE_LENGTH];

pub fn random_salt() -> Salt {
    let mut salt: Salt = [0; SALT_LENGTH];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, salt.as_mut_slice());
    salt
}

pub fn random_nonce() -> Nonce {
    let mut nonce: Nonce = [0; NONCE_LENGTH];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    nonce
}

pub fn encrypt(
    key: argon2::Key,
    nonce: Nonce,
    plaintext: &[u8],
) -> Result<Vec<u8>, aes_gcm::Error> {
    Aes256Gcm::new_from_slice(key.as_slice())
        .map_err(|err| {
            error!("Failed to create cipher: {}", err);
            aes_gcm::Error
        })?
        .encrypt(aes_gcm::Nonce::from_slice(&nonce), plaintext)
        .inspect_err(|err| {
            error!("Failed to encrypt: {}", err);
        })
}

pub fn decrypt(
    key: argon2::Key,
    nonce: Nonce,
    ciphertext: &[u8],
) -> Result<Vec<u8>, aes_gcm::Error> {
    Aes256Gcm::new_from_slice(key.as_slice())
        .map_err(|err| {
            error!("Failed to create cipher: {}", err);
            aes_gcm::Error
        })?
        .decrypt(aes_gcm::Nonce::from_slice(&nonce), ciphertext)
        .inspect_err(|err| {
            error!("Failed to decrypt: {}", err);
        })
}

pub struct Vault {
    pub salt: Salt,
    pub nonce: Nonce,
    pub ciphertext: Vec<u8>,
}

impl Vault {
    pub fn serialize(self) -> Vec<u8> {
        let mut result: Vec<u8> = Vec::with_capacity(HEADER_LEN + self.ciphertext.len());
        result.extend_from_slice(VaultHeader::from(&self).as_bytes());
        result.extend(self.ciphertext);
        result
    }

    pub fn deserialize(data: Vec<u8>) -> Result<Self> {
        if data.len() < HEADER_LEN {
            error!("Failed to deserialize vault: data is missing length header");
            return Err(anyhow!("vault data is missing length header"));
        }

        let header = VaultHeader::read_from_bytes(&data[..HEADER_LEN]).map_err(|err| {
            error!("Failed to read vault header from bytes: {}", err);
            anyhow!("failed to read vault header: {err}")
        })?;
        header.validate()?;

        Ok(Self {
            salt: header.salt,
            nonce: header.nonce,
            ciphertext: data[HEADER_LEN..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_header_format() {
        let vault = Vault {
            salt: [1; SALT_LENGTH],
            nonce: [2; NONCE_LENGTH],
            ciphertext: vec![3, 4, 5],
        };

        let serialized = vault.serialize();
        let deserialized = Vault::deserialize(serialized).expect("deserialize should succeed");

        assert_eq!(deserialized.salt, [1; SALT_LENGTH]);
        assert_eq!(deserialized.nonce, [2; NONCE_LENGTH]);
        assert_eq!(deserialized.ciphertext, vec![3, 4, 5]);
    }

    #[test]
    fn deserialize_rejects_overflowing_length_header() {
        let mut data = Vec::new();
        data.extend(usize::MAX.to_ne_bytes());
        data.extend(1usize.to_ne_bytes());
        data.extend(1usize.to_ne_bytes());

        let error = match Vault::deserialize(data) {
            Ok(_) => panic!("expected deserialize to fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("vault data is missing length header")
        );
    }

    #[test]
    fn deserialize_rejects_length_header_larger_than_data() {
        let mut data = Vec::new();
        data.extend(1usize.to_ne_bytes());
        data.extend(12usize.to_ne_bytes());
        data.extend(1usize.to_ne_bytes());
        data.extend([0]);

        let error = match Vault::deserialize(data) {
            Ok(_) => panic!("expected deserialize to fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("vault data is missing length header")
        );
    }

    #[test]
    fn deserialize_rejects_invalid_magic() {
        let vault = Vault {
            salt: [1; SALT_LENGTH],
            nonce: [2; NONCE_LENGTH],
            ciphertext: vec![3],
        };
        let mut data = vault.serialize();
        data[0] = b'X';

        let error = match Vault::deserialize(data) {
            Ok(_) => panic!("expected deserialize to fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("unsupported vault format"));
    }

    #[test]
    fn deserialize_rejects_unsupported_version() {
        let vault = Vault {
            salt: [1; SALT_LENGTH],
            nonce: [2; NONCE_LENGTH],
            ciphertext: vec![3],
        };
        let mut data = vault.serialize();
        data[6..8].copy_from_slice(&2u16.to_le_bytes());

        let error = match Vault::deserialize(data) {
            Ok(_) => panic!("expected deserialize to fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("unsupported vault version: 2"));
    }
}
