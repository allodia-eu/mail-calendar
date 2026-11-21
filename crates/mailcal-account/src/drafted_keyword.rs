//! The keyword a drafted reply carries (`docs/ai.md` rule 25), and the names it goes by where a
//! provider keeps no free-form keyword.
//!
//! IMAP and JMAP store the keyword itself. Gmail keeps it as a label and Outlook as a category,
//! both of which the person sees, so each needs a name: created in English whatever the device's
//! language, so two devices never create two, and recognised under every catalog locale's
//! translation, so one made by hand or by a later build in another language is reused. This table
//! is the one place the names are written; they are protocol data every device has to agree on,
//! not copy that follows the interface's language, which is why they are not in the catalog.
//!
//! A Gmail label under one of the names leaves the folder pane once the names are registered, so
//! Gmail registers them only on an install the assistant has reached ([`GmailKeywordNaming`]).
//! Graph keeps no category in the folder pane, and registers them always.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use engine_api::{Keyword, KeywordName};

/// The keyword, fixed and untranslated so every device recognises it.
pub const DRAFTED_REPLY_KEYWORD: &str = "writing-assistant";

/// The name a label or category is created under.
const CREATE_AS: &str = "Writing assistant";

/// The same name in every other catalog locale, by locale.
const ALSO_KNOWN_AS: [(&str, &str); 6] = [
    ("nl", "Schrijfassistent"),
    ("de", "Schreibassistent"),
    ("fr", "Assistant de rédaction"),
    ("es", "Asistente de redacción"),
    ("it", "Assistente di scrittura"),
    ("pt", "Assistente de escrita"),
];

/// The keyword a drafted reply's Sent copy and stored draft carry.
#[must_use]
pub fn drafted_reply_keyword() -> Keyword {
    Keyword::new(DRAFTED_REPLY_KEYWORD).expect("the drafted-reply keyword is a valid keyword")
}

/// The names Gmail and Graph keep [`drafted_reply_keyword`] under, for their providers'
/// `with_keyword_names`.
#[must_use]
pub fn drafted_reply_keyword_names() -> Vec<KeywordName> {
    let named = KeywordName::new(drafted_reply_keyword(), CREATE_AS)
        .expect("the drafted-reply name is not blank");
    vec![
        ALSO_KNOWN_AS
            .iter()
            .fold(named, |named, (_, name)| named.also_known_as(*name)),
    ]
}

/// Whether the Gmail accounts of this install read and write the keyword under its names,
/// shared by every Gmail provider built here (`docs/ai.md` rule 25). Off until the writing
/// assistant has been offered on the install, then on for good: a label the person made under one
/// of the names stays theirs for everyone the feature never reached, and a device that once had
/// the offer keeps reading what every device marked. A provider built after it turns on uses the
/// names; mail synced before then has to be read again to carry the keyword.
#[derive(Debug, Clone, Default)]
pub struct GmailKeywordNaming(Arc<AtomicBool>);

impl GmailKeywordNaming {
    /// On or off from the start, as the install last recorded it.
    #[must_use]
    pub fn new(on: bool) -> Self {
        Self(Arc::new(AtomicBool::new(on)))
    }

    /// Whether the names are registered now.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Registers the names from now on; whether they were not before.
    pub fn turn_on(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ALSO_KNOWN_AS, CREATE_AS, GmailKeywordNaming, drafted_reply_keyword,
        drafted_reply_keyword_names,
    };

    #[test]
    fn gmail_names_nothing_until_turned_on_and_then_for_good() {
        let naming = GmailKeywordNaming::default();
        let shared = naming.clone();
        assert!(!naming.is_on());
        assert!(shared.turn_on(), "it was off");
        assert!(naming.is_on(), "every provider shares it");
        assert!(!naming.turn_on(), "already on");
        assert!(GmailKeywordNaming::new(true).is_on());
    }

    #[test]
    fn the_keyword_is_named_in_english_and_recognised_in_every_catalog_locale() {
        let [named] = drafted_reply_keyword_names().try_into().expect("one name");
        assert_eq!(named.keyword(), &drafted_reply_keyword());
        assert_eq!(named.create_as(), CREATE_AS);
        for (_, name) in ALSO_KNOWN_AS {
            assert!(named.is_named(name), "{name}");
        }
        assert!(
            named.is_named("writing ASSISTANT"),
            "matched without regard to case"
        );
    }

    /// Every locale the catalog ships has its name here, and no other.
    #[test]
    fn the_table_covers_the_catalog() {
        let settings = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../project.inlang/settings.json"
        ))
        .expect("the inlang settings");
        let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
        let mut catalog: Vec<&str> = settings["locales"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|locale| *locale != "en")
            .collect();
        catalog.sort_unstable();
        let mut here: Vec<&str> = ALSO_KNOWN_AS.iter().map(|(locale, _)| *locale).collect();
        here.sort_unstable();
        assert_eq!(here, catalog);
    }
}
