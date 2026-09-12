//! The data model shared by the codegen stages, plus the small string helpers they all
//! need: placeholder extraction, identifier validation, parameter typing, and name casing.
//!
//! A message template carries `{name}` placeholders (the inlang message-format convention,
//! the same one the Allodia web app uses). The base locale's template defines the canonical
//! placeholder order; every locale must agree on the placeholder *set* (checked in
//! `crate::validate`).

use std::collections::BTreeMap;

/// The raw, per-locale message maps loaded from disk, before validation.
#[derive(Debug, Clone)]
pub struct Raw {
    /// The base locale (e.g. `en`); its key set is canonical.
    pub base: String,
    /// Every locale in catalog order (the base locale included).
    pub locales: Vec<String>,
    /// `locale -> (key -> template)`, with `$`-prefixed meta keys (e.g. `$schema`) stripped.
    pub maps: BTreeMap<String, BTreeMap<String, String>>,
}

/// One message: its key, the placeholder names (in base-template order), and the per-locale
/// templates.
#[derive(Debug, Clone)]
pub struct Message {
    /// The message key (a valid identifier; see [`is_identifier`]).
    pub key: String,
    /// Placeholder names in first-appearance order in the base template.
    pub placeholders: Vec<String>,
    /// `locale -> template` (raw, still carrying `{name}` placeholders).
    pub values: BTreeMap<String, String>,
}

/// The suffix marking a message as the **singular** partner of the key it is appended to:
/// `mailbox_count_conversations_one` beside `mailbox_count_conversations`.
///
/// The unsuffixed key carries the plural, so a message that never needs a singular is written
/// exactly as before and a singular is purely additive. Only the emitters read this: a key with
/// a partner gets an accessor that picks between the two, and the partner itself gets none, so
/// no caller ever names a grammatical form.
///
/// **A partner keeps the `{count}`.** `"{count} conversation"`, never `"1 conversation"`, and
/// that is also what separates this from the older pairs beside it in the catalog
/// (`invitation_attendees_one`, `"1 attendee"`): those spell the numeral into the sentence, so
/// they are separate messages a caller chooses between itself, and they keep their own
/// accessors. See [`Catalog::is_singular`].
pub const ONE_SUFFIX: &str = "_one";

/// A validated catalog: the base-locale key set, each key carrying its per-locale values.
#[derive(Debug, Clone)]
pub struct Catalog {
    /// The base locale.
    pub base: String,
    /// Every locale in catalog order.
    pub locales: Vec<String>,
    /// The messages, sorted by key (stable output ordering).
    pub messages: Vec<Message>,
}

impl Catalog {
    /// Builds the catalog from validated raw maps; the base locale's keys are canonical and
    /// each message's placeholder order is taken from the base template.
    #[must_use]
    pub fn from_raw(raw: &Raw) -> Self {
        let base_map = &raw.maps[&raw.base];
        let mut messages: Vec<Message> = base_map
            .iter()
            .map(|(key, base_value)| {
                let values = raw
                    .locales
                    .iter()
                    .filter_map(|loc| {
                        raw.maps
                            .get(loc)
                            .and_then(|m| m.get(key))
                            .map(|v| (loc.clone(), v.clone()))
                    })
                    .collect();
                Message {
                    key: key.clone(),
                    placeholders: placeholders(base_value),
                    values,
                }
            })
            .collect();
        messages.sort_by(|a, b| a.key.cmp(&b.key));
        Self {
            base: raw.base.clone(),
            locales: raw.locales.clone(),
            messages,
        }
    }
}

impl Catalog {
    /// Whether `key` has a singular partner, and so needs an accessor that chooses by count.
    #[must_use]
    pub fn has_singular(&self, key: &str) -> bool {
        self.singular_of(key).is_some()
    }

    /// The singular partner of `key`, when it has one.
    #[must_use]
    pub fn singular_of(&self, key: &str) -> Option<&Message> {
        let partner = format!("{key}{ONE_SUFFIX}");
        self.is_singular(&partner)
            .then(|| self.message(&partner))
            .flatten()
    }

    /// The messages an emitter turns into accessors: every message except the singular
    /// partners, which are reached through their plural's accessor rather than named directly.
    /// They stay in the emitted **tables**, which is where the accessor looks them up.
    pub fn accessors(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(|m| !self.is_singular(&m.key))
    }

    /// Whether `key` is a singular partner an accessor should choose for the caller.
    ///
    /// Three conditions, and each rules out a real message in this catalog. The stem must exist
    /// (`setup_allodia_have_one` is a sentence, not a plural). Both halves must carry `{count}`,
    /// which is what the choice is made on and what tells a *plural form* from the older pairs
    /// that spell the numeral in (`invitation_attendees_one`, `"1 attendee"`): those stay
    /// ordinary messages with their own accessors, chosen between at the call site. And the two
    /// placeholder sets must match, so either branch formats with the arguments the accessor
    /// declares.
    #[must_use]
    pub fn is_singular(&self, key: &str) -> bool {
        let Some(stem) = key.strip_suffix(ONE_SUFFIX) else {
            return false;
        };
        let (Some(singular), Some(plural)) = (self.message(key), self.message(stem)) else {
            return false;
        };
        let carries_count = |m: &Message| m.placeholders.iter().any(|p| p == "count");
        carries_count(singular)
            && carries_count(plural)
            && sorted(&singular.placeholders) == sorted(&plural.placeholders)
    }

    /// One message by key.
    #[must_use]
    pub fn message(&self, key: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.key == key)
    }
}

/// A sorted copy, for comparing two placeholder sets regardless of the order they appear in.
fn sorted(names: &[String]) -> Vec<&str> {
    let mut out: Vec<&str> = names.iter().map(String::as_str).collect();
    out.sort_unstable();
    out
}

/// The accessor parameter type for a placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    /// A string-valued placeholder (the default).
    Str,
    /// An integer-valued placeholder: a placeholder named `count` or ending in `_count`.
    Int,
}

/// Classifies a placeholder name into its accessor parameter type. A `count` / `*_count`
/// placeholder is an integer (so a caller passes a number, not a pre-formatted string);
/// everything else is a string. This mirrors Paraglide's typed `m.x({ count })`.
#[must_use]
pub fn param_type(name: &str) -> ParamType {
    if name == "count" || name.ends_with("_count") {
        ParamType::Int
    } else {
        ParamType::Str
    }
}

/// Whether `s` is a valid identifier (`[A-Za-z_][A-Za-z0-9_]*`): the inlang message-id rule,
/// which also guarantees a legal Swift/Kotlin/C# symbol and Android resource name.
#[must_use]
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Extracts `{name}` placeholders from a template in first-appearance order, de-duplicated.
/// Only well-formed identifier placeholders are recognised; a stray `{` or non-identifier
/// content between braces is ignored (so literal text with braces does not become a param).
#[must_use]
pub fn placeholders(template: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        if let Some(close) = after.find('}') {
            let name = &after[..close];
            if is_identifier(name) && !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
            rest = &after[close + 1..];
        } else {
            break;
        }
    }
    out
}

/// Converts a snake/camel key to PascalCase for C# accessor names
/// (`auth_login_emailPlaceholder` -> `AuthLoginEmailPlaceholder`).
#[must_use]
pub fn pascal_case(key: &str) -> String {
    key.split('_')
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}
