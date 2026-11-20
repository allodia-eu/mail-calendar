//! The `[ai]` table (`docs/ai.md`, "Storage"): which writing style each account drafts in, the own
//! endpoint's address, model and declaration, and the jurisdiction mode. The endpoint's key is in
//! the platform keystore, never here.

use std::collections::BTreeMap;

use mailcal_jurisdiction::{Class, Mode};
use serde::{Deserialize, Serialize};

use super::Preferences;
use crate::writing_styles::WritingStyleId;

/// What the writing assistant keeps in the preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPreferences {
    /// The style each account drafts in, by account id; an account absent here drafts in none.
    #[serde(default)]
    pub writing_styles: BTreeMap<String, WritingStyleId>,
    /// The person's own endpoint, when they set one up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<AiEndpoint>,
    /// How far a request may reach: `eu-native` unless the file says otherwise. No client offers
    /// to change it.
    #[serde(default)]
    pub jurisdiction: Mode,
    /// Whether the assistant has been offered on this install, so its Gmail accounts name the
    /// drafted-reply keyword (`crate::GmailKeywordNaming`). Never turned off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gmail_keyword_named: bool,
}

/// An own endpoint's settings, as saved: the address and model trimmed, and where the person
/// declared it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiEndpoint {
    /// The API root, such as `https://api.example.eu/v1`.
    pub base_url: String,
    /// The model asked for.
    pub model: String,
    /// Where the person declared it runs; `None` for a file that says nothing, which the gate
    /// classes as unknown.
    #[serde(default)]
    pub declared: Option<Class>,
}

impl Preferences {
    /// The style `account` drafts in, if the preferences name one.
    #[must_use]
    pub fn writing_style_of(&self, account: &str) -> Option<&WritingStyleId> {
        self.ai.writing_styles.get(account)
    }

    /// Sets the style `account` drafts in, or none.
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

    /// Clears `style` from every account that drafts in it. Whether any did.
    pub fn forget_writing_style(&mut self, style: &WritingStyleId) -> bool {
        let before = self.ai.writing_styles.len();
        self.ai
            .writing_styles
            .retain(|_, assigned| assigned != style);
        self.ai.writing_styles.len() != before
    }
}

#[cfg(test)]
mod tests {
    use mailcal_jurisdiction::{Class, Mode};

    use super::{AiEndpoint, AiPreferences};
    use crate::{Preferences, WritingStyleId};

    #[test]
    fn the_table_round_trips_and_defaults_to_eu_native_with_nothing_set_up() {
        assert_eq!(AiPreferences::default().jurisdiction, Mode::EuNative);
        let mut prefs = Preferences::default();
        let style = WritingStyleId::new("s1").unwrap();
        prefs.set_account_writing_style("acct", Some(style.clone()));
        prefs.ai.endpoint = Some(AiEndpoint {
            base_url: "https://api.example.eu/v1".to_owned(),
            model: "m".to_owned(),
            declared: Some(Class::EuHosted),
        });
        prefs.ai.jurisdiction = Mode::EuHosted;
        let body = toml::to_string(&prefs).unwrap();
        assert!(body.contains("[ai]"), "{body}");
        assert!(body.contains("jurisdiction = \"eu-hosted\""), "{body}");
        let back: Preferences = toml::from_str(&body).unwrap();
        assert_eq!(back.ai, prefs.ai);
        assert_eq!(back.writing_style_of("acct"), Some(&style));
    }

    #[test]
    fn forgetting_a_style_clears_every_account_that_drafts_in_it() {
        let mut prefs = Preferences::default();
        let kept = WritingStyleId::new("kept").unwrap();
        let gone = WritingStyleId::new("gone").unwrap();
        prefs.set_account_writing_style("a", Some(gone.clone()));
        prefs.set_account_writing_style("b", Some(gone.clone()));
        prefs.set_account_writing_style("c", Some(kept.clone()));
        assert!(prefs.forget_writing_style(&gone));
        assert!(!prefs.forget_writing_style(&gone));
        assert_eq!(prefs.writing_style_of("a"), None);
        assert_eq!(prefs.writing_style_of("c"), Some(&kept));
        prefs.set_account_writing_style("c", None);
        assert_eq!(prefs.ai.writing_styles.len(), 0);
    }
}
