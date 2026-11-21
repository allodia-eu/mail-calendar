//! The preferences file, the signature library and the writing assistant's files, each written
//! behind whoever changed it ([`saved_file`]).
//!
//! Every setting lives in `preferences.toml`, the signature library in `signatures.toml` beside it
//! (`docs/signatures.md`, "Storage"), and the style library and the drafted replies beside those
//! (`docs/ai.md`, "Storage"). A save of any that fails is the one notice, and Try again and
//! closing the app reach them all.

use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

use engine_api::Provider;
use mailcal_account::{
    Preferences, Signatures, load_preferences, load_signatures, save_preferences, save_signatures,
};

#[path = "saved_file.rs"]
pub(crate) mod saved_file;

use saved_file::{Document, SavedFile};

use crate::App;

/// A handle on the preferences file.
pub(crate) type PreferencesFile = SavedFile<Preferences>;

/// A handle on the signature library's file.
pub(crate) type SignaturesFile = SavedFile<Signatures>;

impl Document for Preferences {
    const NAME: &'static str = "preferences";

    fn read(path: &Path) -> Self {
        load_preferences(path)
    }

    fn write(path: &Path, document: &Self) -> io::Result<()> {
        save_preferences(path, document)
    }
}

impl Document for Signatures {
    const NAME: &'static str = "signatures";

    fn read(path: &Path) -> Self {
        load_signatures(path)
    }

    fn write(path: &Path, document: &Self) -> io::Result<()> {
        save_signatures(path, document)
    }
}

impl<P: Provider> App<P> {
    /// Whether a change to the settings or the signature library could not be saved, so it holds
    /// only until the app closes. The notices say so;
    /// [`Surface::Notices`](crate::Surface::Notices) is signalled when this changes.
    #[must_use]
    pub fn preferences_not_saved(&self) -> bool {
        self.prefs.failed() || self.signature_library_file().failed() || self.ai_files_failed()
    }

    /// Tries the failed saves again, and waits until the attempts have ended. Not for a UI thread.
    pub fn retry_preferences_save(&self) {
        let library = self.signature_library_file();
        if self.prefs.failed() {
            self.prefs.retry();
        }
        if library.failed() {
            library.retry();
        }
        self.retry_ai_files();
    }

    /// Waits until every change to the settings and the signature library has been written, or a
    /// write has failed. For a process about to end, and for tests. Not for a UI thread.
    pub fn settle_preferences(&self) {
        self.prefs.settle();
        self.signature_library_file().settle();
        self.settle_ai_files();
    }

    /// Waits, for no longer than `limit`, until every change to the settings and the signature
    /// library has been written or a write has failed. For a host whose process may end once it
    /// returns. Whether nothing was left to write.
    pub fn settle_preferences_within(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let settings = self.prefs.settle_by(deadline);
        let assistant = self.settle_ai_files_by(deadline);
        self.signature_library_file().settle_by(deadline) && settings && assistant
    }
}

#[cfg(test)]
#[path = "preferences_file_scratch.rs"]
mod scratch;
#[cfg(test)]
pub(crate) use scratch::ScratchPreferences;

#[cfg(test)]
#[path = "preferences_file_tests.rs"]
mod tests;
