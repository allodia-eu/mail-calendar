//! The files attached to each open composition, held by the core (`docs/composer-layout.md`,
//! rules 3, 4 and 8).
//!
//! A composer draws [`ComposerFiles`] and changes the list only through
//! [`MailcalApp::add_composer_files`] and [`MailcalApp::remove_composer_file`]. Every save and the
//! send read the list from here, so the file on screen and the file that goes out cannot differ.
//! The list starts at `open_composer`, which seeds it with what the composer opens holding, and
//! ends with the composition: a discard, a close, a save-and-close or an accepted send.

use std::{collections::HashMap, sync::Mutex};

use mailcal_app::CompositionId;
use mailcal_composer::{safe_file_name, safe_media_type};

use crate::{
    MailcalApp, MailcalError, composer_files::ComposerFileAttachment,
    records_account_page::ControlState,
};

/// The files a composer shows, as the core holds them.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComposerFiles {
    /// One tile per file, in the order they were added.
    pub tiles: Vec<ComposerFileTile>,
    /// Whether the list differs from what the composer opened holding: the files' half of "was
    /// anything written".
    pub changed: bool,
}

/// One attached file, as its tile shows it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComposerFileTile {
    /// What [`MailcalApp::remove_composer_file`] names it by. Unique within the composition.
    pub id: String,
    /// The name the message will carry.
    pub file_name: String,
    /// Its size in bytes, or `None` when the file could not be read. The client words it.
    pub size: Option<u64>,
    /// The tile's Remove control.
    pub remove: ControlState,
}

/// A file's id within its composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileId(u64);

impl FileId {
    fn label(self) -> String {
        format!("file-{}", self.0)
    }

    fn parse(label: &str) -> Option<Self> {
        label.strip_prefix("file-")?.parse().ok().map(Self)
    }
}

#[derive(Debug, Clone)]
struct HeldFile {
    id: FileId,
    path: String,
    file_name: String,
    media_type: String,
    size: Option<u64>,
}

#[derive(Debug, Default)]
struct FileList {
    files: Vec<HeldFile>,
    opened_with: Vec<u64>,
    next: u64,
}

impl FileList {
    fn add(&mut self, files: Vec<ComposerFileAttachment>) {
        for file in files {
            let file_name = safe_file_name(&file.file_name, &file.path);
            let media_type = safe_media_type(&file.media_type, &file_name);
            let size = std::fs::metadata(&file.path).ok().map(|meta| meta.len());
            self.files.push(HeldFile {
                id: FileId(self.next),
                path: file.path,
                file_name,
                media_type,
                size,
            });
            self.next += 1;
        }
    }

    fn snapshot(&self) -> ComposerFiles {
        ComposerFiles {
            tiles: self
                .files
                .iter()
                .map(|file| ComposerFileTile {
                    id: file.id.label(),
                    file_name: file.file_name.clone(),
                    size: file.size,
                    remove: ControlState {
                        enabled: true,
                        running: false,
                    },
                })
                .collect(),
            changed: !self
                .files
                .iter()
                .map(|file| file.id.0)
                .eq(self.opened_with.iter().copied()),
        }
    }
}

/// Every open composition's files.
#[derive(Debug, Default)]
pub(crate) struct ComposerFileLists(Mutex<HashMap<CompositionId, FileList>>);

impl ComposerFileLists {
    /// Starts `composition`'s list holding `files`, which is what it counts as unchanged.
    pub(crate) fn open(&self, composition: &CompositionId, files: Vec<ComposerFileAttachment>) {
        let mut list = FileList::default();
        list.add(files);
        list.opened_with = list.files.iter().map(|file| file.id.0).collect();
        self.lock().insert(composition.clone(), list);
    }

    pub(crate) fn add(
        &self,
        composition: &CompositionId,
        files: Vec<ComposerFileAttachment>,
    ) -> ComposerFiles {
        let mut lists = self.lock();
        let list = lists.entry(composition.clone()).or_default();
        list.add(files);
        list.snapshot()
    }

    /// Takes the file off the list. An id the list does not hold changes nothing.
    pub(crate) fn remove(&self, composition: &CompositionId, file: &str) -> ComposerFiles {
        let mut lists = self.lock();
        let list = lists.entry(composition.clone()).or_default();
        if let Some(id) = FileId::parse(file) {
            list.files.retain(|held| held.id != id);
        }
        list.snapshot()
    }

    pub(crate) fn read(&self, composition: &CompositionId) -> ComposerFiles {
        self.lock()
            .get(composition)
            .map_or_else(|| FileList::default().snapshot(), FileList::snapshot)
    }

    /// What a save or the send attaches: every file, where it is now.
    pub(crate) fn attachments(&self, composition: &CompositionId) -> Vec<ComposerFileAttachment> {
        self.lock().get(composition).map_or_else(Vec::new, |list| {
            list.files
                .iter()
                .map(|file| ComposerFileAttachment {
                    path: file.path.clone(),
                    file_name: file.file_name.clone(),
                    media_type: file.media_type.clone(),
                })
                .collect()
        })
    }

    pub(crate) fn forget(&self, composition: &CompositionId) {
        self.lock().remove(composition);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CompositionId, FileList>> {
        self.0.lock().expect("composer files mutex poisoned")
    }
}

#[uniffi::export]
impl MailcalApp {
    /// The files `composition`'s composer shows. An unknown composition holds none.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn composer_files(&self, composition: String) -> Result<ComposerFiles, MailcalError> {
        Ok(self.composer_files.read(&composition_id(composition)?))
    }

    /// Attaches `files` (picked, dropped, pasted or shared) to `composition`, after what it
    /// already holds, and answers with the list as it now stands. Each file's name and media type
    /// are settled here, and its size read; a file that cannot be read is still listed, with no
    /// size, and the send says so if it still cannot be read then.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn add_composer_files(
        &self,
        composition: String,
        files: Vec<ComposerFileAttachment>,
    ) -> Result<ComposerFiles, MailcalError> {
        Ok(self
            .composer_files
            .add(&composition_id(composition)?, files))
    }

    /// Takes the file `file` (a tile's `id`) off `composition`, and answers with the list as it
    /// now stands.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn remove_composer_file(
        &self,
        composition: String,
        file: String,
    ) -> Result<ComposerFiles, MailcalError> {
        Ok(self
            .composer_files
            .remove(&composition_id(composition)?, &file))
    }
}

fn composition_id(value: String) -> Result<CompositionId, MailcalError> {
    CompositionId::new(value)
        .ok_or_else(|| MailcalError::Engine("composition id is blank".to_owned()))
}

#[cfg(test)]
#[path = "composer_file_list_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests_composer_files.rs"]
mod app_tests;
