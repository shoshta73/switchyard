use std::{env, ffi::OsStr, io, path::PathBuf};

use tracing::{debug, warn};

pub(crate) fn log_debug_info() {
    debug!("Runtime paths are provided by the CLI package");
}

pub(crate) fn state_dir() -> io::Result<PathBuf> {
    user_state_dir().map(|path| path.join("switchyard"))
}

pub(crate) fn salt_file() -> io::Result<PathBuf> {
    state_dir().map(|path| path.join("salt"))
}

fn user_state_dir() -> io::Result<PathBuf> {
    user_state_dir_from_env(
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

fn user_state_dir_from_env(
    xdg_state_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> io::Result<PathBuf> {
    if let Some(path) = xdg_state_home {
        let path = PathBuf::from(path);

        if path.is_absolute() {
            return Ok(path);
        }
    }
    warn!("XDG_STATE_HOME is not set or is not an absolute path");

    home.map(PathBuf::from)
        .map(|path| path.join(".local/state"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "HOME environment variable is not set",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_absolute_xdg_state_home() {
        let path = user_state_dir_from_env(
            Some(OsStr::new("/tmp/state")),
            Some(OsStr::new("/home/user")),
        )
        .unwrap();

        assert_eq!(path, PathBuf::from("/tmp/state"));
    }

    #[test]
    fn ignores_relative_xdg_state_home() {
        let path = user_state_dir_from_env(
            Some(OsStr::new("relative/state")),
            Some(OsStr::new("/home/user")),
        )
        .unwrap();

        assert_eq!(path, PathBuf::from("/home/user/.local/state"));
    }

    #[test]
    fn falls_back_to_home() {
        let path = user_state_dir_from_env(None, Some(OsStr::new("/home/user"))).unwrap();

        assert_eq!(path, PathBuf::from("/home/user/.local/state"));
    }

    #[test]
    fn errors_when_home_is_missing() {
        let error = user_state_dir_from_env(None, None).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn appends_switchyard_to_user_state_dir() {
        let expected = user_state_dir().unwrap().join("switchyard");

        assert_eq!(state_dir().unwrap(), expected);
    }

    #[test]
    fn appends_salt_file_to_state_dir() {
        let expected = state_dir().unwrap().join("salt");

        assert_eq!(salt_file().unwrap(), expected);
    }
}
