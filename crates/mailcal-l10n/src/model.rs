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
