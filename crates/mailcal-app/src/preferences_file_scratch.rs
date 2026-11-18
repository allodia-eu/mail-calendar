//! A preferences file of one test's own.

use std::path::PathBuf;

use mailcal_account::{Preferences, save_preferences};
use tempfile::TempDir;

use super::PreferencesFile;

/// A preferences file in a directory of the test's own, removed when the test ends. No two
/// tests share one, in this checkout or another.
pub(crate) struct ScratchPreferences {
    dir: TempDir,
}

impl ScratchPreferences {
    /// No file yet: what a first launch finds.
    pub(crate) fn empty() -> Self {
        Self {
            dir: TempDir::new().expect("the test's directory could not be made"),
        }
    }

    /// A file holding the defaults.
    pub(crate) fn new() -> Self {
        Self::seeded(&Preferences::default())
    }

    /// A file holding `seed`.
    pub(crate) fn seeded(seed: &Preferences) -> Self {
        let scratch = Self::empty();
        save_preferences(scratch.path(), seed)
            .expect("the test's preferences could not be written");
        scratch
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.dir.path().join("preferences.toml")
    }

    /// A handle on the file whose every change is written before it returns, so a test can read
    /// the disk straight after one.
    pub(crate) fn open(&self) -> PreferencesFile {
        PreferencesFile::start(self.path(), Box::new(|| {}), true)
    }
}
