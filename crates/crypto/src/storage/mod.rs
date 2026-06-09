mod v1;

const MAGIC: &[u8; 6] = b"SWYVLT";
const VERSION: u16 = 1_u16;

pub(crate) const HEADER_LEN: usize = v1::VAULT_HEADER_LEN;

pub(crate) type VaultHeader = v1::VaultHeader;
