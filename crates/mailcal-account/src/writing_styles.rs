//! The writing-style library (`docs/ai.md`, "What a style is"): the person's learned styles,
//! stored in their own file beside `preferences.toml`, as the signature library is.
//!
//! A style is standalone, like a signature: learned once, named, and chosen by any number of
//! accounts. Which style an account drafts in is a small pointer in the preferences
//! ([`crate::AiPreferences::writing_styles`]); the styles live here, because each carries a few
//! kilobytes of passages and every preference write rewrites the whole preferences file.
//!
//! The description and the passages are stored as the JSON `mailcal-ai` writes, so this file never
//! has to agree with that crate's schema.
//!
//! **Privacy.** Both halves are the person's own writing and never reach a log, which is why
//! [`StoredWritingStyle`]'s `Debug` prints lengths.

use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The identity of one writing style: opaque CSPRNG output minted by the core, never derived from
/// the name, so a rename breaks no account that drafts in it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WritingStyleId(String);

impl WritingStyleId {
    /// An id, when `value` is not blank and carries no control character. The file is one a
    /// person can edit, so an id read back is judged again.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty() && !value.chars().any(char::is_control)).then_some(Self(value))
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
    /// The person's name for it.
    pub name: String,
    /// The address it was learned from. Kept as the address rather than an account id, because
    /// the style outlives the account.
    #[serde(default)]
    pub learned_from: String,
    /// The description, as `mailcal-ai` serialises a `StyleGuide`. The person's notes are in it.
    pub guide_json: String,
    /// The passages, as `mailcal-ai` serialises `Exemplars`.
    #[serde(default)]
    pub exemplars_json: String,
    /// Whether the reveal has been shown, so a row opens the detail from then on.
    #[serde(default)]
    pub revealed: bool,
}

impl fmt::Debug for StoredWritingStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredWritingStyle")
            .field("name_len", &self.name.len())
            .field("guide_len", &self.guide_json.len())
            .field("exemplars_len", &self.exemplars_json.len())
            .field("revealed", &self.revealed)
            .finish_non_exhaustive()
    }
}

/// The library: the styles, and the order the person sees them in.
///
/// The order is kept apart from the map for the reason [`crate::Signatures`] keeps it apart: a
/// [`BTreeMap`] sorts by the random id, which is no order at all to a reader.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritingStyles {
    /// Every style, by id.
    #[serde(default)]
    pub entries: BTreeMap<WritingStyleId, StoredWritingStyle>,
    /// The display order. An id here with no entry is ignored, and an entry missing from it still
    /// appears, so a hand-edited file cannot hide a style the person can no longer forget.
    #[serde(default)]
    pub order: Vec<WritingStyleId>,
}

impl WritingStyles {
    /// The styles in display order: those `order` names first, then any it left out.
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

    /// One style.
    #[must_use]
    pub fn get(&self, id: &WritingStyleId) -> Option<&StoredWritingStyle> {
        self.entries.get(id)
    }

    /// Adds a style at the end of the order, or replaces the one under `id` where it stands.
    pub fn insert(&mut self, id: WritingStyleId, style: StoredWritingStyle) {
        if !self.order.contains(&id) {
            self.order.push(id.clone());
        }
        self.entries.insert(id, style);
    }

    /// Removes a style and its place in the order. Whether there was one.
    pub fn remove(&mut self, id: &WritingStyleId) -> bool {
        self.order.retain(|entry| entry != id);
        self.entries.remove(id).is_some()
    }

    /// Drops the entries whose id [`WritingStyleId::new`] would refuse.
    fn keep_valid_ids(mut self) -> Self {
        let valid = |id: &WritingStyleId| WritingStyleId::new(id.as_str()).is_some();
        let before = self.entries.len();
        self.entries.retain(|id, _| valid(id));
        self.order.retain(valid);
        let dropped = before - self.entries.len();
        if dropped > 0 {
            log::warn!("ai: {dropped} writing style(s) with an unreadable id were left out");
        }
        self
    }
}

/// The library's file name, beside `preferences.toml`.
const FILE_NAME: &str = "writing_styles.toml";

/// The library's path inside the app data directory `base`.
#[must_use]
pub fn writing_styles_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// The library at `path`; empty when the file is missing or does not parse, as the signature
/// library is.
#[must_use]
pub fn load_writing_styles(path: impl AsRef<Path>) -> WritingStyles {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str::<WritingStyles>(&body).ok())
        .map(WritingStyles::keep_valid_ids)
        .unwrap_or_default()
}

/// Writes the library to `path`, beside it and renamed over it, so a reader sees the old file or
/// the new one and never one cut short.
///
/// # Errors
///
/// Returns an [`io::Error`] when the directory or the file cannot be written, or
/// [`io::ErrorKind::InvalidData`] when the library does not serialise.
pub fn save_writing_styles(path: impl AsRef<Path>, styles: &WritingStyles) -> io::Result<()> {
    let body =
        toml::to_string(styles).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    crate::preferences::replace_file(path.as_ref(), &body)
}

#[cfg(test)]
#[path = "writing_styles_tests.rs"]
mod tests;
