use anyhow::{Context, Result};
use inquire::{Password, PasswordDisplayMode, validator::Validation};

use crate::log;

pub(crate) fn initial_password() -> Result<String> {
    log::disable_direct_terminal_output();

    Password::new("SwitchYard encryption password:")
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_validator(|s: &str| Ok(
            if s.len() >= 8
                && s.chars().any(char::is_uppercase)
                && s.chars().any(char::is_lowercase)
                && s.chars().any(|c| c.is_ascii_digit())
                && s.chars().any(|c| !c.is_alphanumeric())
            {
                Validation::Valid
            } else {
                Validation::Invalid(
                    "Password must contain at least 8 characters, 1 uppercase letter, 1 lowercase letter, 1 number, and 1 special character".into()
                )
            }
        ))
        .with_custom_confirmation_error_message("Passwords do not match")
        .prompt()
        .context("failed to get user password input")
}

pub(crate) fn password() -> Result<String> {
    log::disable_direct_terminal_output();

    Password::new("SwitchYard encryption password:")
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()
        .context("failed to get user password input")
}
