//! The AI features' preferences: which writing style each account drafts in, and the person's own
//! endpoint. The jurisdiction mode is a field of its own on [`Preferences`], because it binds
//! every external dispatch and not only these.

use std::collections::BTreeMap;

use mailcal_jurisdiction::Class;
use serde::{Deserialize, Serialize};

use super::Preferences;
use crate::writing_styles::WritingStyleId;

/// What the AI features remember, stored as the `[ai]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPreferences {
    /// Which writing style each account drafts replies in, keyed by account id. The styles live
    /// in their own store ([`crate::WritingStyles`]); an account absent here has none. Read it
    /// through [`Preferences::writing_style_of`].
    #[serde(default)]
    pub writing_styles: BTreeMap<String, WritingStyleId>,
    /// The person's own endpoint, when they configured one. Its key is in the keystore.
    #[serde(default)]
    pub endpoint: Option<AiEndpoint>,
}

/// An endpoint the person runs or rents themselves: where it is, the model to ask for, and where
/// they say it runs. The key is in the platform keystore, never in this file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiEndpoint {
    /// The API root, such as `https://api.example.eu/v1`.
    pub base_url: String,
    /// The model name to ask for.
    pub model: String,
    /// Where the person says it runs; `None` until they say.
    #[serde(default)]
    pub declared: Option<Class>,
}

impl Preferences {
    /// The style `account` drafts in, if it has one.
    #[must_use]
    pub fn writing_style_of(&self, account: &str) -> Option<&WritingStyleId> {
        self.ai.writing_styles.get(account)
    }

    /// Assigns a style to `account`, or clears its assignment with `None`.
    pub fn set_account_writing_style(&mut self, account: &str, style: Option<WritingStyleId>) {
        match style {
            Some(style) => {
                self.ai.writing_styles.insert(account.to_owned(), style);
            }
            None => {
                self.ai.writing_styles.remove(account);
            }
        }
    }

    /// Drops `account`'s assignment; called when the account is removed. Returns whether it had
    /// one.
    pub fn remove_account_writing_style(&mut self, account: &str) -> bool {
        self.ai.writing_styles.remove(account).is_some()
    }

    /// Clears `style` from every account that drafts in it; called when the style is forgotten.
    /// Returns whether any did.
    pub fn forget_writing_style(&mut self, style: &WritingStyleId) -> bool {
        let before = self.ai.writing_styles.len();
        self.ai
            .writing_styles
            .retain(|_, assigned| assigned != style);
        self.ai.writing_styles.len() != before
    }
}
