use crate::{
    NONCE_LENGTH, SALT_LENGTH, Vault,
    storage::{MAGIC, VERSION},
};

use anyhow::{Result, anyhow};

/// Length in bytes of the version 1 vault header.
pub(crate) const VAULT_HEADER_LEN: usize = 64;
const MAGIC_LEN: usize = 6;
const VERSION_LEN: usize = 2;

const RESERVED_LEN: usize = VAULT_HEADER_LEN - MAGIC_LEN - VERSION_LEN - SALT_LENGTH - NONCE_LENGTH;

/// Version 1 vault header stored before ciphertext bytes.
#[derive(
    Clone,
    Copy,
    zerocopy::IntoBytes,
    zerocopy::FromBytes,
    zerocopy::Immutable,
    zerocopy::KnownLayout,
)]
#[repr(C)]
pub(crate) struct VaultHeader {
    magic: [u8; MAGIC_LEN],
    version: [u8; VERSION_LEN],
    pub(crate) salt: [u8; SALT_LENGTH],
    pub(crate) nonce: [u8; NONCE_LENGTH],
    unused: [u8; RESERVED_LEN],
}

impl From<&Vault> for VaultHeader {
    fn from(value: &Vault) -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION.to_le_bytes(),
            salt: value.salt,
            nonce: value.nonce,
            unused: [0; RESERVED_LEN],
        }
    }
}

impl VaultHeader {
    /// Validates that the header belongs to a supported vault format.
    pub(crate) fn validate(&self) -> Result<()> {
        if &self.magic != MAGIC {
            return Err(anyhow!("unsupported vault format"));
        }

        let version = u16::from_le_bytes(self.version);
        if version != VERSION {
            return Err(anyhow!("unsupported vault version: {version}"));
        }

        Ok(())
    }
}
