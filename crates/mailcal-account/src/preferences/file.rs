//! Where the preferences file lives, and how it is read and written.
//!
//! Its own module because reading and writing the file is a separate responsibility from what
//! the file holds, and `preferences.rs` was over the 500-line limit. The re-exports keep the
//! three functions at `mailcal_account::`, so no caller moves.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::Preferences;

/// The preferences file's name, in the app data directory.
const FILE_NAME: &str = "preferences.toml";

/// The preferences file's path inside the app data directory `base`.
///
/// Derived here so a host that reads the file before the app exists; `Appearance` is wanted
/// before the first frame; cannot end up naming a different file from the one the app writes.
#[must_use]
pub fn preferences_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// Loads preferences from `path`. A missing or unreadable/unparseable file yields
/// defaults (`display_timezone: None`) rather than an error: a preferences file is
/// best-effort state, and a host that cannot read it simply falls back to the
/// device zone on next boot.
#[must_use]
pub fn load_preferences(path: impl AsRef<Path>) -> Preferences {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

/// Writes `prefs` to `path` as TOML, creating parent directories as needed.
///
/// # Errors
///
/// Returns an [`io::Error`] if the parent directory or file cannot be written (a
/// TOML serialization failure is mapped to [`io::ErrorKind::InvalidData`], though a
/// flat preferences struct never triggers it in practice).
pub fn save_preferences(path: impl AsRef<Path>, prefs: &Preferences) -> io::Result<()> {
    let body =
        toml::to_string(prefs).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)
}
