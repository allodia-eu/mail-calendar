//! The half of a writing style that travels between a person's devices, and what arrives from
//! them (`docs/ai.md`). The binding layer carries it to the Allodia account service; this is what
//! it reads and writes here.
//!
//! **No passage leaves, and none arrives.** [`SyncableStyle`] is a style's name and guide and
//! nothing else. A style that arrives is given passages here, from this device's own sent mail,
//! without a request.

use std::fmt;

use engine_api::Provider;
use mailcal_account::StoredWritingStyle;
use mailcal_ai::{
    Exemplars, StyleGuide,
    corpus::{self, CorpusOptions},
    pick_passages,
};

use super::{LearnRange, mint_style_id};
use crate::{App, Surface};

/// A style's name and guide: all of it that is synced.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncableStyle {
    /// The style's id on this device.
    pub id: String,
    /// The person's name for it.
    pub name: String,
    /// What was learned, and the notes.
    pub guide: StyleGuide,
}

impl fmt::Debug for SyncableStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncableStyle")
            .field("id", &self.id)
            .field("name_len", &self.name.len())
            .field("guide", &self.guide)
            .finish()
    }
}

impl<P: Provider> App<P> {
    /// Every style's name and guide, in library order.
    ///
    /// A style whose stored guide does not read is left out: sent as an empty guide, it would
    /// empty the style on every other device.
    #[must_use]
    pub fn syncable_writing_styles(&self) -> Vec<SyncableStyle> {
        self.writing_style
            .library()
            .ordered()
            .into_iter()
            .filter_map(|(id, style)| {
                Some(SyncableStyle {
                    id: id.as_str().to_owned(),
                    name: style.name.clone(),
                    guide: serde_json::from_str(&style.guide_json).ok()?,
                })
            })
            .collect()
    }

    /// Replaces a style's name and guide with what another device synced, keeping this device's
    /// passages and every account's slot. Returns whether the id named one.
    pub fn apply_synced_writing_style(&self, id: &str, name: String, guide: &StyleGuide) -> bool {
        let Ok(guide_json) = serde_json::to_string(guide) else {
            return false;
        };
        self.edit_writing_style(id, |style| {
            style.name = name;
            style.guide_json = guide_json;
        })
    }

    /// Adds a style that arrived from another device, with passages picked from this device's own
    /// sent mail in the languages its guide covers, and returns its id.
    ///
    /// No account drafts in it until the person says so: the slots name this device's accounts,
    /// which the other device never knew.
    pub async fn add_synced_writing_style(&self, name: String, guide: &StyleGuide) -> String {
        let passages = self.passages_for(guide).await;
        let id = mint_style_id();
        let state = &self.writing_style;
        let mut library = state.library();
        library.insert(
            id.clone(),
            StoredWritingStyle {
                name,
                source: String::new(),
                guide_json: serde_json::to_string(guide).unwrap_or_default(),
                exemplars_json: serde_json::to_string(&passages).unwrap_or_default(),
            },
        );
        state.persist_library(&library);
        drop(library);
        log::info!(
            "ai: a writing style arrived from another device, with passages in {} of its {} \
             language(s)",
            passages.languages.len(),
            guide.languages.len(),
        );
        self.observer.surface_changed(Surface::WritingStyle);
        id.as_str().to_owned()
    }

    /// Passages for `guide` from every account's sent mail on this device.
    async fn passages_for(&self, guide: &StyleGuide) -> Exemplars {
        let mut messages = Vec::new();
        for account in self.account_ids().await {
            if let Ok((sent, _)) = self.sent_mail(&account, LearnRange::default()).await {
                messages.extend(sent);
            }
        }
        let signatures = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .plain_bodies();
        let corpus = corpus::build(
            messages,
            &CorpusOptions {
                signatures,
                horizon: None,
            },
        );
        pick_passages(guide, &corpus)
    }
}
