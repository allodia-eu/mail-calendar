//! The style library and each account's slot (`docs/ai.md`, "What a style is"), held in memory
//! and written behind each change, as the signature library is.
//!
//! The rules every change keeps: forgetting a style clears every slot holding it; a learned style
//! takes its account's slot when that is empty; a slot given a style that names nothing is
//! cleared; a slot naming a style that has gone reads as none.

use std::{io, path::Path, time::Instant};

use engine_api::Provider;
use mailcal_account::{
    StoredWritingStyle, WritingStyleId, WritingStyles, load_writing_styles, save_writing_styles,
    writing_styles_path,
};

use crate::{
    App, Surface,
    preferences_file::saved_file::{Document, SavedFile},
};

/// A handle on the library's file.
pub(crate) type StylesFile = SavedFile<WritingStyles>;

impl Document for WritingStyles {
    const NAME: &'static str = "writing styles";

    fn read(path: &Path) -> Self {
        load_writing_styles(path)
    }

    fn write(path: &Path, document: &Self) -> io::Result<()> {
        save_writing_styles(path, document)
    }
}

/// The library as the app holds it, and its file.
pub(crate) struct StyleLibrary {
    library: WritingStyles,
    pub(super) file: StylesFile,
}

impl StyleLibrary {
    pub(super) fn new(dir: Option<&Path>, on_failure: impl Fn() + Send + Sync + 'static) -> Self {
        let path = dir.map(writing_styles_path);
        Self {
            library: path.as_ref().map(load_writing_styles).unwrap_or_default(),
            file: StylesFile::at(path, on_failure),
        }
    }

    fn put(&mut self, id: WritingStyleId, style: StoredWritingStyle) {
        self.library.insert(id.clone(), style.clone());
        self.file
            .edit(move |library| library.insert(id.clone(), style.clone()));
    }

    fn remove(&mut self, id: &WritingStyleId) -> bool {
        if !self.library.remove(id) {
            return false;
        }
        let id = id.clone();
        self.file.edit(move |library| {
            library.remove(&id);
        });
        true
    }
}

/// Whether a change to the library, and the slots it moved, reached the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylesWritten {
    /// Written.
    Written,
    /// Held until the app closes: the failed-save notice says so and offers Try again.
    NotWritten,
}

/// Mints an opaque style id from the system CSPRNG: 16 bytes, URL-safe base64, as a signature's.
pub(crate) fn mint_writing_style_id() -> WritingStyleId {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::{SecureRandom, SystemRandom};

    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    WritingStyleId::new(URL_SAFE_NO_PAD.encode(bytes)).expect("base64 is a valid style id")
}

impl<P: Provider> App<P> {
    fn style_library(&self) -> std::sync::MutexGuard<'_, StyleLibrary> {
        self.ai
            .styles
            .lock()
            .expect("writing styles mutex poisoned")
    }

    /// The library in the person's order. Read from memory.
    #[must_use]
    pub fn writing_styles(&self) -> Vec<(WritingStyleId, StoredWritingStyle)> {
        self.style_library()
            .library
            .ordered()
            .into_iter()
            .map(|(id, style)| (id.clone(), style.clone()))
            .collect()
    }

    /// One style, or `None` when the id names nothing.
    #[must_use]
    pub fn writing_style(&self, id: &WritingStyleId) -> Option<StoredWritingStyle> {
        self.style_library().library.get(id).cloned()
    }

    /// The style `account` drafts in: one in the library, or none.
    #[must_use]
    pub fn writing_style_slot(&self, account: &str) -> Option<WritingStyleId> {
        let id = self.ai.settings().writing_styles.get(account).cloned()?;
        self.style_library()
            .library
            .get(&id)
            .is_some()
            .then_some(id)
    }

    /// Adds a newly learned style under an id of its own, giving it `account`'s slot when that
    /// holds none. Written behind the caller; [`App::settle_writing_styles`] waits for it.
    pub fn add_writing_style(
        &self,
        account: Option<&str>,
        style: StoredWritingStyle,
    ) -> WritingStyleId {
        let id = mint_writing_style_id();
        self.style_library().put(id.clone(), style);
        if let Some(account) = account
            && self.writing_style_slot(account).is_none()
        {
            self.assign_writing_style(account, Some(id.clone()));
        }
        log::info!("ai: a style was added to the library");
        self.observer.surface_changed(Surface::WritingStyle);
        id
    }

    /// Replaces the style `id` (a rename, the notes, the reveal shown). Whether it was there: an
    /// edit never makes a style the person thought they were changing.
    pub fn replace_writing_style(&self, id: &WritingStyleId, style: StoredWritingStyle) -> bool {
        {
            let mut library = self.style_library();
            if library.library.get(id).is_none() {
                return false;
            }
            library.put(id.clone(), style);
        }
        self.observer.surface_changed(Surface::WritingStyle);
        true
    }

    /// Forgets the style `id` and clears every slot that held it. Whether it was there.
    pub fn forget_writing_style(&self, id: &WritingStyleId) -> bool {
        if !self.style_library().remove(id) {
            return false;
        }
        let id = id.clone();
        self.ai.edit_settings(move |settings| {
            settings.writing_styles.retain(|_, held| *held != id);
        });
        log::info!("ai: a style was forgotten");
        self.observer.surface_changed(Surface::WritingStyle);
        true
    }

    /// Sets the style `account` drafts in, or none. A style that names nothing clears the slot.
    pub fn assign_writing_style(&self, account: &str, style: Option<WritingStyleId>) {
        let style = style.filter(|id| self.style_library().library.get(id).is_some());
        let account = account.to_owned();
        self.ai.edit_settings(move |settings| match &style {
            Some(style) => {
                settings
                    .writing_styles
                    .insert(account.clone(), style.clone());
            }
            None => {
                settings.writing_styles.remove(&account);
            }
        });
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// Waits until the library and the preferences holding the slots are written, or a write has
    /// failed. Not for a UI thread.
    pub fn settle_writing_styles(&self) -> StylesWritten {
        let file = self.style_library().file.clone();
        file.settle();
        self.prefs.settle();
        if file.failed() || self.prefs.failed() {
            StylesWritten::NotWritten
        } else {
            StylesWritten::Written
        }
    }

    pub(crate) fn writing_styles_file(&self) -> StylesFile {
        self.style_library().file.clone()
    }

    /// Whether one of the assistant's own files could not be written.
    pub(crate) fn ai_files_failed(&self) -> bool {
        self.writing_styles_file().failed() || self.drafted_replies_file().failed()
    }

    /// Tries the assistant's failed writes again, and waits for the attempts.
    pub(crate) fn retry_ai_files(&self) {
        let styles = self.writing_styles_file();
        if styles.failed() {
            styles.retry();
        }
        let drafted = self.drafted_replies_file();
        if drafted.failed() {
            drafted.retry();
        }
    }

    /// Waits until the assistant's files are written, or a write has failed.
    pub(crate) fn settle_ai_files(&self) {
        self.writing_styles_file().settle();
        self.drafted_replies_file().settle();
    }

    /// Waits, until `deadline` at the latest, for the assistant's files to be written. Whether
    /// nothing was left to write.
    pub(crate) fn settle_ai_files_by(&self, deadline: Instant) -> bool {
        let styles = self.writing_styles_file().settle_by(deadline);
        self.drafted_replies_file().settle_by(deadline) && styles
    }
}
