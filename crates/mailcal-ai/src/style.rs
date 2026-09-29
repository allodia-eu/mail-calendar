//! A writing style's two halves: the [`StyleGuide`] that may be synced, and the [`Exemplars`] that
//! never leave the device except inside a prompt.
//!
//! **Forward compatible on write-back.** Both carry a `schema_version` and keep every field they
//! do not model in `unknown`, so an older device that edits a guide a newer one wrote (the notes,
//! a rename) writes the newer fields back untouched.
//!
//! **No passage of mail in the guide.** The guide describes; the exemplars quote. What a model
//! returns for the guide is bounded on the way in ([`LanguageStyle::bounded`]) so a description
//! cannot carry a paragraph of somebody's message into the synced record.

use std::{collections::BTreeMap, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The schema version this crate writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The synced half of a writing style: what was learned about how one person writes, per
/// language, and the notes they added.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleGuide {
    /// The schema version the guide was written under.
    pub schema_version: u32,
    /// One entry per language the person writes in, keyed by its ISO 639-1 code.
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageStyle>,
    /// What the person added or corrected, in their own words. Sent with every draft.
    #[serde(default)]
    pub notes: String,
    /// What it was learned from.
    #[serde(default)]
    pub learned: Option<Provenance>,
    /// Fields a newer version wrote.
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

impl StyleGuide {
    /// An empty guide at the current schema version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            languages: BTreeMap::new(),
            notes: String::new(),
            learned: None,
            unknown: Map::new(),
        }
    }

    /// The language a reply is written in when nothing better is known: the one most messages
    /// were learned from.
    #[must_use]
    pub fn main_language(&self) -> Option<&str> {
        self.learned
            .as_ref()
            .and_then(|learned| {
                learned
                    .messages_per_language
                    .iter()
                    .max_by_key(|(_, count)| **count)
                    .map(|(language, _)| language.as_str())
            })
            .or_else(|| self.languages.keys().next().map(String::as_str))
    }
}

impl Default for StyleGuide {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for StyleGuide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StyleGuide")
            .field("schema_version", &self.schema_version)
            .field("languages", &self.languages.keys().collect::<Vec<_>>())
            .field("notes_len", &self.notes.len())
            .finish_non_exhaustive()
    }
}

/// Where a guide was learned from: the date range and how many messages. No account, no
/// address: the guide may be synced, and an account id means nothing on another device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The oldest message used, as seconds since the Unix epoch.
    #[serde(default)]
    pub oldest: Option<i64>,
    /// The newest message used, as seconds since the Unix epoch.
    #[serde(default)]
    pub newest: Option<i64>,
    /// How many messages each language was learned from.
    #[serde(default)]
    pub messages_per_language: BTreeMap<String, u32>,
    /// When it was learned, as seconds since the Unix epoch.
    #[serde(default)]
    pub learned_at: i64,
    /// Fields a newer version wrote.
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

/// How a person writes in one language.
///
/// The descriptive fields are written in the language of the person's interface, because the
/// person reads them back on the reveal screen; the habits and phrases are quoted in the language
/// of the mail, because a draft copies them.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageStyle {
    /// How they open a message, most frequent first, with the exact wording.
    #[serde(default)]
    pub greetings: Vec<Habit>,
    /// How they close a message, the words before their name, most frequent first.
    #[serde(default)]
    pub sign_offs: Vec<Habit>,
    /// The name they sign with, exactly as they write it. Empty when they do not sign.
    #[serde(default)]
    pub signs_as: String,
    /// Their register and how it shifts with the recipient: formal or informal pronouns, first
    /// names or titles.
    #[serde(default)]
    pub register: String,
    /// The register in at most five words, as a heading.
    #[serde(default)]
    pub register_headline: String,
    /// The typical length of a reply, in words. Zero when unknown.
    #[serde(default)]
    pub typical_words: u32,
    /// How many paragraphs a typical reply has between the greeting and the sign-off. Zero when
    /// unknown.
    #[serde(default)]
    pub typical_paragraphs: u32,
    /// Paragraphing and sentence length.
    #[serde(default)]
    pub shape: String,
    /// Punctuation, capitalisation, emoji and exclamation habits.
    #[serde(default)]
    pub punctuation: String,
    /// Structural habits: opening with thanks, closing with a next step, using lists.
    #[serde(default)]
    pub structure: String,
    /// The structural habits in at most five words, as a heading.
    #[serde(default)]
    pub structure_headline: String,
    /// How they decline, chase, apologise and confirm.
    #[serde(default)]
    pub moves: String,
    /// How they decline, chase, apologise and confirm, in at most five words, as a heading.
    #[serde(default)]
    pub moves_headline: String,
    /// Short words and constructions characteristic of them.
    #[serde(default)]
    pub phrases: Vec<String>,
    /// What they avoid, and what a draft in their name must never do.
    #[serde(default)]
    pub avoid: Vec<String>,
    /// Fields a newer version wrote.
    #[serde(flatten)]
    #[schemars(skip)]
    pub unknown: Map<String, Value>,
}

/// One recurring form and roughly how often it is used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Habit {
    /// The exact wording.
    pub text: String,
    /// Roughly how often, as a percentage of messages.
    #[serde(default)]
    pub share: u8,
}

/// The longest description kept, in characters.
const DESCRIPTION_CAP: usize = 600;
/// The longest habit, phrase or name kept, in characters.
const PHRASE_CAP: usize = 80;
/// The most entries a list keeps.
const LIST_CAP: usize = 12;

/// The longest a headline is kept, in words and in characters.
const HEADLINE_WORDS: usize = 5;
const HEADLINE_CAP: usize = 60;

/// More paragraphs than this is a model miscounting, not a habit.
const PARAGRAPH_CAP: u32 = 12;

impl LanguageStyle {
    /// Bounds what a model returned: descriptions to a paragraph, headlines to five words, habits
    /// and phrases to a short line, lists to a dozen entries. An entry over the line cap is dropped
    /// rather than cut, because a cut quotation is still a quotation.
    #[must_use]
    pub fn bounded(mut self) -> Self {
        for text in [
            &mut self.register,
            &mut self.shape,
            &mut self.punctuation,
            &mut self.structure,
            &mut self.moves,
        ] {
            *text = truncate(text.trim(), DESCRIPTION_CAP);
        }
        for headline in [
            &mut self.register_headline,
            &mut self.structure_headline,
            &mut self.moves_headline,
        ] {
            let words = headline.split_whitespace().take(HEADLINE_WORDS);
            *headline = truncate(&words.collect::<Vec<_>>().join(" "), HEADLINE_CAP);
        }
        self.typical_paragraphs = self.typical_paragraphs.min(PARAGRAPH_CAP);
        self.signs_as = short(&self.signs_as).unwrap_or_default();
        for habits in [&mut self.greetings, &mut self.sign_offs] {
            habits.retain_mut(|habit| match short(&habit.text) {
                Some(text) => {
                    habit.text = text;
                    habit.share = habit.share.min(100);
                    true
                }
                None => false,
            });
            habits.truncate(LIST_CAP);
        }
        for list in [&mut self.phrases, &mut self.avoid] {
            *list = list.iter().filter_map(|entry| short(entry)).collect();
            list.truncate(LIST_CAP);
        }
        self
    }
}

impl fmt::Debug for LanguageStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageStyle")
            .field("greetings", &self.greetings.len())
            .field("sign_offs", &self.sign_offs.len())
            .field("typical_words", &self.typical_words)
            .field("typical_paragraphs", &self.typical_paragraphs)
            .field("phrases", &self.phrases.len())
            .finish_non_exhaustive()
    }
}

/// `text` trimmed, or `None` when it is empty or longer than a phrase.
fn short(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && text.chars().count() <= PHRASE_CAP).then(|| text.to_owned())
}

fn truncate(text: &str, cap: usize) -> String {
    text.chars().take(cap).collect()
}

/// The local half of a writing style: short passages the person wrote, per language, which a
/// draft imitates. Never synced; re-picked from the Sent folder on each device.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Exemplars {
    /// The schema version they were written under.
    pub schema_version: u32,
    /// The passages, keyed by ISO 639-1 code.
    #[serde(default)]
    pub languages: BTreeMap<String, Vec<String>>,
    /// Fields a newer version wrote.
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

impl Exemplars {
    /// No passages, at the current schema version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            languages: BTreeMap::new(),
            unknown: Map::new(),
        }
    }
}

impl Default for Exemplars {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Exemplars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts: BTreeMap<&str, usize> = self
            .languages
            .iter()
            .map(|(language, passages)| (language.as_str(), passages.len()))
            .collect();
        f.debug_struct("Exemplars")
            .field("schema_version", &self.schema_version)
            .field("passages", &counts)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
