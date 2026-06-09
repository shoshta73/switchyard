use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use tracing::error;

use switchyard_crypto::{Vault, argon2, decrypt, encrypt, random_nonce, random_salt};

mod application;
mod command;
mod log;
mod provider;
mod runtime;
mod terminal;
mod user;

fn get_initialized_encryption_key(salt_file: &Path) -> Result<argon2::Key> {
    let vault = Vault::deserialize(fs::read(salt_file).context("failed to read salt file")?)
        .context("failed to deserialize vault file")?;
    for attempt in 1..=3 {
        let key = argon2::derive_key(
            vault.salt,
            user::password().context("Failed to get password")?.as_str(),
        )
        .map_err(|err| anyhow!(err.to_string()))
        .context("failed to derive key from password")?;
        let decrypted = match decrypt(key, vault.nonce, vault.ciphertext.clone().as_slice()) {
            Ok(decrypted) => decrypted,
            Err(err) => {
                error!("Failed to decrypt cipher text:{err}");
                if attempt == 3 {
                    return Err(anyhow!(err.to_string()))
                        .context("failed to unlock vault after 3 attempts");
                }
                continue;
            }
        };

        if str::from_utf8(decrypted.as_slice())
            .context("failed to convert decrypted bytes to string")?
            == "switchyard"
        {
            return Ok(key);
        }
    }

    Err(anyhow!("failed to unlock vault after 3 attempts"))
}

fn setup_encryption(salt_file: &Path) -> Result<argon2::Key> {
    let salt = random_salt();
    let password = user::initial_password().context("Failed to get initial password")?;
    let key = argon2::derive_key(salt, password.as_str())
        .map_err(|err| anyhow!(err.to_string()))
        .context("failed to derive key from password")?;
    let nonce = random_nonce();
    fs::write(
        salt_file,
        Vault {
            salt,
            nonce,
            ciphertext: encrypt(key, nonce, b"switchyard")
                .map_err(|err| anyhow!(err.to_string()))
                .context("failed to encrypt initialization value")?,
        }
        .serialize(),
    )
    .context("failed to serialize vault file")?;
    Ok(key)
}

fn get_encryption_key(data: &mut application::Data) -> Result<()> {
    let state_dir = runtime::state_dir().context("failed to get state directory")?;
    let salt_file = runtime::salt_file().context("failed to get salt file path")?;
    if !fs::exists(state_dir.as_path()).context("failed to check if state_dir exists")? {
        fs::create_dir_all(state_dir).context("failed to create state directory")?;
        data.encryption_key =
            setup_encryption(salt_file.as_path()).context("Failed to setup encryption")?;
        return Ok(());
    }

    if !fs::exists(salt_file.as_path()).context("failed to check if salt_file_exists")? {
        data.encryption_key =
            setup_encryption(salt_file.as_path()).context("Failed to setup encryption")?;
        return Ok(());
    }

    data.encryption_key = get_initialized_encryption_key(salt_file.as_path())
        .context("Failed to get initialized encryption key")?;
    Ok(())
}

fn main() -> Result<()> {
    let mut application_data = application::Data::new();
    get_encryption_key(&mut application_data).context("Failed to get encryption key")?;
    let (terminal, guard) = terminal::init().context("failed to initialize terminal")?;
    application_data.terminal = Some(terminal);
    application_data.terminal_guard = guard;
    let mut application_state = application::State::default();
    application::run(&mut application_data, &mut application_state)
        .context("failed to run application")?;
    let terminal = application_data
        .terminal
        .as_mut()
        .context("terminal is not initialized")?;
    terminal.show_cursor().context("Failed to show cursor")?;
    application_data
        .terminal_guard
        .restore(terminal.backend_mut())
        .context("Failed to restore terminal")?;
    Ok(())
}
