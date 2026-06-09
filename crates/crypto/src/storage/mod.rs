mod v1;

/// Magic bytes that identify Switchyard vault files.
const MAGIC: &[u8; 6] = b"SWYVLT";
/// Current vault storage format version.
const VERSION: u16 = 1_u16;

/// Length in bytes of the current vault header.
pub(crate) const HEADER_LEN: usize = v1::VAULT_HEADER_LEN;

/// Current vault header representation.
pub(crate) type VaultHeader = v1::VaultHeader;
