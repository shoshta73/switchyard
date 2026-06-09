use argon2::{Argon2, Params};
use tracing::error;

use crate::Salt;

const KEY_LENGTH: usize = 32;
/// AES-256 key derived from a password and salt.
pub type Key = [u8; KEY_LENGTH];

type Result<T> = std::result::Result<T, argon2::Error>;

/// Derives a fixed-length encryption key from a password and salt using Argon2id.
pub fn derive_key(salt: Salt, password: &str) -> Result<Key> {
    let params = Params::new(
        64 * 1024,         // memory cost in KiB = 64 MB
        3,                 // iterations
        4,                 // parallelism
        KEY_LENGTH.into(), // output length
    )
    .inspect_err(|err| error!("Failed to create Argon2 params: {}", err))?;
    let ctx = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key: Key = [0; KEY_LENGTH];
    ctx.hash_password_into(password.as_bytes(), salt.as_ref(), key.as_mut_slice())
        .inspect_err(|err| error!("Failed to hash password: {}", err))?;
    Ok(key)
}
