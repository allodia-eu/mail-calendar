// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Keeping a person's writing styles the same on every device.
//!
//! **What travels, and what does not.** A style's name and its guide: what was learned about how
//! the person writes, and the notes they added. Never the passages of their mail that a draft
//! imitates: those stay on each device and are picked again there from its own sent mail.
//! [`SyncedStyle`] has no field to put one in, and the service refuses a payload with a key
//! starting `exemplar` anywhere in it.
//!
//! The rules are the account list's: an opaque id minted on the first store, a version every
//! write names, a tombstone for a removal ([`SyncedCollection`](crate::SyncedCollection)).

use std::fmt;

use mailcal_ai::StyleGuide;
use serde::{Deserialize, Serialize};

use crate::{
    collection::{ConflictWith, SyncedRecord, Tombstone},
    reconcile::{Fingerprint, Mine, SyncState, Verdict, decide},
};

/// One style, as it travels: the person's name for it and the guide.
///
/// The guide keeps every field it does not model, so a device that edits a newer device's style
/// writes the newer fields back untouched.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedStyle {
    /// The person's name for it ("Work", "Personal").
    pub name: String,
    /// What was learned, and the notes.
    pub guide: StyleGuide,
}

impl fmt::Debug for SyncedStyle {
    /// The name is the person's own words, so a log or a panic sees its length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncedStyle")
            .field("name_len", &self.name.len())
            .field("guide", &self.guide)
            .finish()
    }
}

impl Fingerprint for SyncedStyle {
    /// Only an identical style is the same one. Two devices that each learned a style called
    /// "Work" hold two styles, and adopting one as the other would overwrite it at the next push.
    fn is_same_as(&self, other: &Self) -> bool {
        self == other
    }
}

/// A stored style, as the service hands it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleRecord {
    /// The sync id. Opaque, and the device keeps it beside the style.
    pub id: String,
    /// Bumped by every write, deletions included. The next write has to name it.
    pub version: u64,
    /// The style itself.
    pub style: SyncedStyle,
    /// When the record last changed, as the server wrote it (RFC 3339). Display only.
    pub updated_at: String,
}

impl SyncedRecord for StyleRecord {
    type List = StyleList;
    type Payload = SyncedStyle;

    const PATH: &'static str = "writing-styles";
    const PAYLOAD: &'static str = "style";

    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn payload(&self) -> &SyncedStyle {
        &self.style
    }

    fn in_conflict(with: &ConflictWith) -> Option<&Self> {
        match with {
            ConflictWith::Style(current) => Some(current),
            _ => None,
        }
    }
}

/// Every style the service holds for this person, or everything that changed since a moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleList {
    /// The styles themselves.
    pub styles: Vec<StyleRecord>,
    /// What has been forgotten.
    pub deleted: Vec<Tombstone>,
    /// The moment this answer describes, to pass back as `since`.
    pub synced_at: String,
}

/// One of this device's styles, as the reconciler sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalStyle {
    /// The core's own id for the style.
    pub style_id: String,
    /// What would travel.
    pub style: SyncedStyle,
    /// What this device remembers about syncing it. `None` for one the service has never seen.
    pub sync: Option<SyncState>,
}

/// Work out what to do about the difference between this device's styles and the service's.
#[must_use]
pub fn reconcile_styles(local: &[LocalStyle], remote: &StyleList) -> Vec<Verdict<StyleRecord>> {
    let mine = local.iter().map(|style| Mine {
        local_id: &style.style_id,
        payload: &style.style,
        sync: style.sync.as_ref(),
    });
    decide(mine, &remote.styles, &remote.deleted)
}

#[cfg(test)]
#[path = "styles_tests.rs"]
mod styles_tests;

#[cfg(test)]
#[path = "styles_reconcile_tests.rs"]
mod styles_reconcile_tests;
