//! The files attached to each open composition, held by the core (`docs/composer-layout.md`,
//! rules 3, 4 and 8).
//!
//! A composer draws [`ComposerFiles`] and changes the list only through
//! [`MailcalApp::add_composer_files`] and [`MailcalApp::remove_composer_file`], or, for files it
//! still has to copy or read, [`MailcalApp::begin_adding_composer_files`] and
//! [`MailcalApp::finish_adding_composer_files`], between which its Attach runs. Every save and the
//! send read the list from here, so the file on screen and the file that goes out cannot differ.
//! The list starts at `open_composer`, which seeds it with what the composer opens holding, and
//! ends with the composition: a discard, a close, a save-and-close or an accepted send.

use std::{collections::HashMap, path::Path, sync::Mutex};

use mailcal_app::{CompositionId, StagedAttachment, is_raster_picture};
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
    /// The sizes of every file that could be read, in bytes, added up: what the summary above the
    /// tiles states beside the count. The client words it.
    #[uniffi(default = 0)]
    pub total_size: u64,
    /// The composer's Attach: running, and so held, from the moment files the person chose are
    /// on their way (a picker has closed, a drop has landed) until they are on the list. A client
    /// copies or reads a file before the core can list it, and a film can take many seconds.
    pub attach: ControlState,
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
    /// The file's own path when its bytes are a picture every client decodes (PNG, JPEG, GIF or
    /// WebP, the avatars' sniff), so the tile may show it shrunk; `None` for anything else, which
    /// the tile draws as a file. Never decoded at full size for this.
    #[uniffi(default = None)]
    pub preview: Option<String>,
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
    picture: bool,
}

#[derive(Debug, Default)]
struct FileList {
    files: Vec<HeldFile>,
    opened_with: Vec<u64>,
    next: u64,
    /// Additions started and not yet ended.
    adding: u32,
}

/// A file as it is read for the list, before it has an id. Read before the lock is taken: its
/// size and first bytes come off a disk that may be a slow network mount, and every composer's
/// list waits on that lock.
#[derive(Debug)]
struct ReadFile {
    path: String,
    file_name: String,
    media_type: String,
    size: Option<u64>,
    picture: bool,
}

fn read_files(files: Vec<ComposerFileAttachment>) -> Vec<ReadFile> {
    files
        .into_iter()
        .map(|file| {
            let file_name = safe_file_name(&file.file_name, &file.path);
            let media_type = safe_media_type(&file.media_type, &file_name);
            let size = std::fs::metadata(&file.path).ok().map(|meta| meta.len());
            let picture = is_raster_picture(Path::new(&file.path));
            ReadFile {
                path: file.path,
                file_name,
                media_type,
                size,
                picture,
            }
        })
        .collect()
}

impl FileList {
    fn add(&mut self, files: Vec<ReadFile>) {
        for file in files {
            self.files.push(HeldFile {
                id: FileId(self.next),
                path: file.path,
                file_name: file.file_name,
                media_type: file.media_type,
                size: file.size,
                picture: file.picture,
            });
            self.next += 1;
        }
    }

    fn added_since(&self, mark: u64) -> usize {
        self.files.iter().filter(|file| file.id.0 >= mark).count()
    }

    fn changed(&self, since: Option<u64>, before: Option<usize>) -> FilesChanged {
        FilesChanged {
            files: self.snapshot(),
            added: since
                .zip(before)
                .map(|(mark, before)| (before, self.added_since(mark))),
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
                    preview: file.picture.then(|| file.path.clone()),
                })
                .collect(),
            changed: !self
                .files
                .iter()
                .map(|file| file.id.0)
                .eq(self.opened_with.iter().copied()),
            total_size: self.files.iter().filter_map(|file| file.size).sum(),
            attach: ControlState {
                enabled: self.adding == 0,
                running: self.adding > 0,
            },
        }
    }
}

/// A list as a change left it, with how many of its files were added since the mark the change
/// was asked to count from, before the change and after it.
#[derive(Debug)]
pub(crate) struct FilesChanged {
    pub(crate) files: ComposerFiles,
    pub(crate) added: Option<(usize, usize)>,
}

/// Every open composition's files.
#[derive(Debug, Default)]
pub(crate) struct ComposerFileLists(Mutex<HashMap<CompositionId, FileList>>);

impl ComposerFileLists {
    /// Starts `composition`'s list holding `files`, which is what it counts as unchanged.
    pub(crate) fn open(&self, composition: &CompositionId, files: Vec<ComposerFileAttachment>) {
        let mut list = FileList::default();
        list.add(read_files(files));
        list.opened_with = list.files.iter().map(|file| file.id.0).collect();
        self.lock().insert(composition.clone(), list);
    }

    /// [`Self::open`] for a composition that has no list yet, and nothing for one that has: an
    /// Outbox edit's opening is read until the host dismisses it, and a later read must not undo a
    /// file the user added or removed in between.
    pub(crate) fn open_if_absent(
        &self,
        composition: &CompositionId,
        files: Vec<ComposerFileAttachment>,
    ) {
        if !self.lock().contains_key(composition) {
            self.open(composition, files);
        }
    }

    /// Adds `files` after what the list holds, counting the files added since `since`
    /// ([`Self::mark`]) on either side of the change, under the one lock the change takes.
    pub(crate) fn add(
        &self,
        composition: &CompositionId,
        files: Vec<ComposerFileAttachment>,
        since: Option<u64>,
    ) -> FilesChanged {
        let files = read_files(files);
        let mut lists = self.lock();
        let list = lists.entry(composition.clone()).or_default();
        let before = since.map(|mark| list.added_since(mark));
        list.add(files);
        list.changed(since, before)
    }

    /// Starts one addition: Attach runs until [`Self::finish_adding`] ends it.
    pub(crate) fn begin_adding(&self, composition: &CompositionId) -> ComposerFiles {
        let mut lists = self.lock();
        let list = lists.entry(composition.clone()).or_default();
        list.adding += 1;
        list.snapshot()
    }

    /// Ends one addition with the files that arrived, none when nothing did, counting as
    /// [`Self::add`] does. An end with no addition under way ends nothing, and one for a
    /// composition that has gone (the message was discarded or sent meanwhile) starts no list.
    pub(crate) fn finish_adding(
        &self,
        composition: &CompositionId,
        files: Vec<ComposerFileAttachment>,
        since: Option<u64>,
    ) -> FilesChanged {
        let files = read_files(files);
        let mut lists = self.lock();
        let Some(list) = lists.get_mut(composition) else {
            return FilesChanged {
                files: FileList::default().snapshot(),
                added: None,
            };
        };
        let before = since.map(|mark| list.added_since(mark));
        list.add(files);
        list.adding = list.adding.saturating_sub(1);
        list.changed(since, before)
    }

    /// Ends every addition under way in the list's count, for a composer that moved: the one
    /// that started them is the one that sees them end, so the composer it moved to would
    /// otherwise draw Attach running with nothing to stop it. Their files still land.
    pub(crate) fn settle_adding(&self, composition: &CompositionId) {
        if let Some(list) = self.lock().get_mut(composition) {
            list.adding = 0;
        }
    }

    /// Takes the file off the list, counting as [`Self::add`] does. An id the list does not hold
    /// changes nothing, and a composition that has gone (a Remove pressed as it was sent or
    /// discarded) is not started again with an empty list.
    pub(crate) fn remove(
        &self,
        composition: &CompositionId,
        file: &str,
        since: Option<u64>,
    ) -> FilesChanged {
        let mut lists = self.lock();
        let Some(list) = lists.get_mut(composition) else {
            return FilesChanged {
                files: FileList::default().snapshot(),
                added: None,
            };
        };
        let before = since.map(|mark| list.added_since(mark));
        if let Some(id) = FileId::parse(file) {
            list.files.retain(|held| held.id != id);
        }
        list.changed(since, before)
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

    /// Where `composition`'s list stands: the number its next file will be given.
    pub(crate) fn mark(&self, composition: &CompositionId) -> u64 {
        self.lock().get(composition).map_or(0, |list| list.next)
    }

    /// How many files on `composition`'s list were added since `mark` ([`Self::mark`]).
    pub(crate) fn added_since(&self, composition: &CompositionId, mark: u64) -> usize {
        self.lock()
            .get(composition)
            .map_or(0, |list| list.added_since(mark))
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
        let composition = composition_id(composition)?;
        let since = self.app.reply_draft_files_mark(&composition);
        let changed = self.composer_files.add(&composition, files, since);
        Ok(self.files_changed(&composition, changed))
    }

    /// Starts adding files the person chose that are not ready yet: a picker has closed or a drop
    /// has landed, and the client still copies or reads them. Attach runs from here until
    /// [`MailcalApp::finish_adding_composer_files`] ends it, which a client calls exactly once for
    /// each start, with nothing when nothing arrived. Answers with the list as it now stands.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn begin_adding_composer_files(
        &self,
        composition: String,
    ) -> Result<ComposerFiles, MailcalError> {
        Ok(self
            .composer_files
            .begin_adding(&composition_id(composition)?))
    }

    /// Ends one addition started with [`MailcalApp::begin_adding_composer_files`]: attaches
    /// `files` as [`MailcalApp::add_composer_files`] does, and answers with the list as it now
    /// stands. Off the UI thread, for the same reason as that call.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn finish_adding_composer_files(
        &self,
        composition: String,
        files: Vec<ComposerFileAttachment>,
    ) -> Result<ComposerFiles, MailcalError> {
        let composition = composition_id(composition)?;
        let since = self.app.reply_draft_files_mark(&composition);
        let changed = self
            .composer_files
            .finish_adding(&composition, files, since);
        Ok(self.files_changed(&composition, changed))
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
        let composition = composition_id(composition)?;
        let since = self.app.reply_draft_files_mark(&composition);
        let changed = self.composer_files.remove(&composition, &file, since);
        Ok(self.files_changed(&composition, changed))
    }
}

impl MailcalApp {
    /// Hands the drafted reply's record what a change to the list did to the files it counts
    /// (`docs/ai.md` rule 23), and answers with the list.
    fn files_changed(&self, composition: &CompositionId, changed: FilesChanged) -> ComposerFiles {
        if let Some((before, after)) = changed.added {
            self.app
                .reply_draft_files_changed(composition, before, after);
        }
        changed.files
    }
}

/// What the core staged for a composer to open holding, as the list's entries.
pub(crate) fn staged_files(files: Vec<StagedAttachment>) -> Vec<ComposerFileAttachment> {
    files
        .into_iter()
        .map(|file| ComposerFileAttachment {
            path: file.path,
            file_name: file.file_name,
            media_type: file.media_type,
        })
        .collect()
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
