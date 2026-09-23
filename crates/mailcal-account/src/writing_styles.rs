//! The writing-style library: the person's learned styles and where they are stored.
//!
//! A writing style is a **standalone entity**, like a signature: learned once, named, and assigned
//! to any number of accounts (`docs/ai.md`). Which style an account drafts in is a small pointer
//! in the preferences ([`crate::Preferences::writing_style_assignments`]); the styles themselves
//! live in their own file, because each carries a few kilobytes of passages and every preference
//! write rewrites the whole preferences file.
//!
//! **Two halves, one of which never leaves the device.** `guide_json` is what was learned, and
//! may be synced; `exemplars_json` is short passages of the person's own mail and is local by
//! construction (`docs/ai.md`, "What leaves the device"). Both are stored as JSON strings rather
//! than modelled in TOML, so this file never has to agree with the schema `mailcal-ai` owns.
//!
//! **Privacy.** Both halves are user content and never reach a log, which is why
//! [`StoredWritingStyle`]'s `Debug` prints lengths.

use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The stable identity of one writing style.
///
/// Opaque and random, minted by the product core, never derived from the name. A newtype so it
/// cannot be passed where an account id or a signature id belongs.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WritingStyleId(String);

impl WritingStyleId {
    /// A writing-style id when the value is non-empty after trimming and carries no control
    /// characters. The store is a file a person can edit, so a value read back is validated.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The id as stored.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WritingStyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WritingStyleId").field(&self.0).finish()
    }
}

/// One learned style.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredWritingStyle {
    /// The person's name for it ("Work", "Personal").
    pub name: String,
    /// The account it was learned from, by id; empty for a style that arrived from another
    /// device, where that account's id means nothing.
    #[serde(default)]
    pub source: String,
    /// The style guide, as `mailcal-ai` serialises it. The half that may be synced.
    pub guide_json: String,
    /// The exemplars, as `mailcal-ai` serialises them. Never synced.
    #[serde(default)]
    pub exemplars_json: String,
}

impl fmt::Debug for StoredWritingStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredWritingStyle")
            .field("name_len", &self.name.len())
            .field("guide_len", &self.guide_json.len())
            .field("exemplars_len", &self.exemplars_json.len())
            .finish_non_exhaustive()
    }
}

/// The persisted library: the styles, and the order the person arranged them in.
///
/// The order is kept apart from the map for the reason [`crate::Signatures`] keeps it apart: a
/// [`BTreeMap`] sorts by the random id, which is no order at all to a reader.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritingStyles {
    /// Every style, keyed by id.
    #[serde(default)]
    pub entries: BTreeMap<WritingStyleId, StoredWritingStyle>,
    /// The display order. An id here with no entry is ignored, and an entry missing from it
    /// still appears, so a hand-edited file cannot hide a style the person can no longer delete.
    #[serde(default)]
    pub order: Vec<WritingStyleId>,
}

impl WritingStyles {
    /// The styles in display order: those named in `order` first, then any `order` forgot.
    #[must_use]
    pub fn ordered(&self) -> Vec<(&WritingStyleId, &StoredWritingStyle)> {
        let mut listed: Vec<_> = self
            .order
            .iter()
            .filter_map(|id| self.entries.get_key_value(id))
            .collect();
        listed.extend(
            self.entries
                .iter()
                .filter(|(id, _)| !self.order.contains(id)),
        );
        listed
    }

    /// One style by id.
    #[must_use]
    pub fn get(&self, id: &WritingStyleId) -> Option<&StoredWritingStyle> {
        self.entries.get(id)
    }

    /// One style by id, to change.
    pub fn get_mut(&mut self, id: &WritingStyleId) -> Option<&mut StoredWritingStyle> {
        self.entries.get_mut(id)
    }

    /// Adds a style at the end of the order, or replaces it in place under a known id.
    pub fn insert(&mut self, id: WritingStyleId, style: StoredWritingStyle) {
        if !self.order.contains(&id) {
            self.order.push(id.clone());
        }
        self.entries.insert(id, style);
    }

    /// Removes a style and its place in the order. Returns whether it existed.
    pub fn remove(&mut self, id: &WritingStyleId) -> bool {
        self.order.retain(|entry| entry != id);
        self.entries.remove(id).is_some()
    }
}

/// The library file's name, beside `preferences.toml`.
const FILE_NAME: &str = "writing_styles.toml";

/// The library's path inside the app data directory `base`.
#[must_use]
pub fn writing_styles_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// Loads the library from `path`; empty when the file is absent or unreadable, as the signature
/// library is.
#[must_use]
pub fn load_writing_styles(path: impl AsRef<Path>) -> WritingStyles {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

/// Writes the library to `path`, creating its directory when needed.
///
/// # Errors
///
/// Returns an [`io::Error`] when the directory or the file cannot be written, or
/// [`io::ErrorKind::InvalidData`] when the library does not serialise.
pub fn save_writing_styles(path: impl AsRef<Path>, styles: &WritingStyles) -> io::Result<()> {
    let body =
        toml::to_string(styles).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)
}

#[cfg(test)]
#[path = "writing_styles_tests.rs"]
mod tests;
