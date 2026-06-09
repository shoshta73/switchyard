use std::{
    fmt::Write as _,
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use keyring_core::{Entry, Error as KeyringError};
use switchyard_crypto::argon2;
use tracing::{debug, warn};

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const KEY_HEX_LENGTH: usize = 64;
const SERVICE: &str = "switchyard";
const USER: &str = "encryption-key-cache";

pub(crate) fn load() -> Result<Option<argon2::Key>> {
    let entry = entry()?;
    let secret = match entry.get_password() {
        Ok(secret) => secret,
        Err(KeyringError::NoEntry) => return Ok(None),
        Err(err) => return Err(err).context("failed to read key cache from keyring"),
    };

    let cached = match CachedKey::parse(secret.trim()) {
        Ok(cached) => cached,
        Err(err) => {
            warn!("Ignoring invalid SwitchYard key cache: {}", err);
            remove()?;
            return Ok(None);
        }
    };
    if cached.is_expired(SystemTime::now()) {
        debug!("SwitchYard key cache expired");
        remove()?;
        return Ok(None);
    }

    Ok(Some(cached.key))
}

pub(crate) fn store(key: argon2::Key) -> Result<()> {
    let expires_at = SystemTime::now()
        .checked_add(CACHE_TTL)
        .context("failed to calculate key cache expiry")?;
    let cached = CachedKey { key, expires_at };

    entry()?
        .set_password(cached.serialize().as_str())
        .context("failed to write key cache to keyring")?;
    Ok(())
}

pub(crate) fn remove() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(KeyringError::NoEntry) => Ok(()),
        Err(err) => {
            warn!(
                "Failed to remove SwitchYard key cache from keyring: {}",
                err
            );
            Err(err).context("failed to remove key cache from keyring")
        }
    }
}

fn entry() -> Result<Entry> {
    static STORE: OnceLock<()> = OnceLock::new();

    if STORE.get().is_none() {
        keyring::use_native_store(false).context("failed to initialize native keyring store")?;
        let _ = STORE.set(());
    }

    Entry::new(SERVICE, USER).context("failed to open keyring entry")
}

struct CachedKey {
    key: argon2::Key,
    expires_at: SystemTime,
}

impl CachedKey {
    fn parse(input: &str) -> Result<Self> {
        let (expires_at, key) = input
            .split_once('\n')
            .ok_or_else(|| anyhow!("key cache is missing expiry"))?;
        let expires_at = expires_at
            .strip_prefix("expires_at=")
            .ok_or_else(|| anyhow!("key cache expiry is malformed"))?
            .parse::<u64>()
            .context("key cache expiry is not a unix timestamp")?;
        let key = key
            .strip_prefix("key=")
            .ok_or_else(|| anyhow!("key cache key is malformed"))?;

        Ok(Self {
            key: decode_key(key)?,
            expires_at: UNIX_EPOCH + Duration::from_secs(expires_at),
        })
    }

    fn serialize(&self) -> String {
        let expires_at = self
            .expires_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        format!("expires_at={expires_at}\nkey={}\n", encode_key(self.key))
    }

    fn is_expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }
}

fn encode_key(key: argon2::Key) -> String {
    let mut encoded = String::with_capacity(KEY_HEX_LENGTH);
    for byte in key {
        write!(&mut encoded, "{byte:02x}").expect("writing to string cannot fail");
    }
    encoded
}

fn decode_key(encoded: &str) -> Result<argon2::Key> {
    if encoded.len() != KEY_HEX_LENGTH {
        return Err(anyhow!("key cache key has invalid length"));
    }

    let mut key = [0; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&encoded[start..start + 2], 16)
            .context("key cache key is not hex")?;
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_and_parses_cached_key() {
        let cached = CachedKey {
            key: [7; 32],
            expires_at: UNIX_EPOCH + Duration::from_secs(42),
        };

        let parsed = CachedKey::parse(cached.serialize().trim()).unwrap();

        assert_eq!(parsed.key, [7; 32]);
        assert_eq!(parsed.expires_at, UNIX_EPOCH + Duration::from_secs(42));
    }

    #[test]
    fn detects_expired_cached_key() {
        let cached = CachedKey {
            key: [7; 32],
            expires_at: UNIX_EPOCH + Duration::from_secs(42),
        };

        assert!(cached.is_expired(UNIX_EPOCH + Duration::from_secs(43)));
        assert!(!cached.is_expired(UNIX_EPOCH + Duration::from_secs(41)));
    }

    #[test]
    fn rejects_invalid_key_length() {
        let error = decode_key("abc").unwrap_err();

        assert!(error.to_string().contains("invalid length"));
    }
}
