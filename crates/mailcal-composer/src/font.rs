//! A run's typeface and size: what the toolbar offers, and what another client's draft carries.

use core::fmt;

use serde::{Deserialize, Serialize};

/// A text size in CSS pixels.
///
/// The toolbar offers four of them ([`FontSize::SMALL`] to [`FontSize::HUGE`]), but a draft another
/// client wrote can carry any size, and a size rounded to the nearest of four changes the user's
/// draft on its next save. Out-of-range values clamp to [`FontSize::MIN_PX`]..=[`FontSize::MAX_PX`]
/// rather than fail the document: size is presentation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FontSize(u16);

impl FontSize {
    /// The toolbar's huge size.
    pub const HUGE: Self = Self(24);
    /// The toolbar's large size.
    pub const LARGE: Self = Self(18);
    /// The largest size kept: 72pt, the largest size Word and Outlook offer.
    pub const MAX_PX: u16 = 96;
    /// The smallest size kept.
    pub const MIN_PX: u16 = 6;
    /// The toolbar's normal size: the body size the editor draws.
    pub const NORMAL: Self = Self(15);
    /// The toolbar's small size.
    pub const SMALL: Self = Self(13);

    /// A size of `px` pixels, clamped to the range kept.
    #[must_use]
    pub fn from_px(px: u16) -> Self {
        Self(px.clamp(Self::MIN_PX, Self::MAX_PX))
    }

    /// Pixel value used in outgoing HTML.
    #[must_use]
    pub const fn css_px(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for FontSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FontSize({}px)", self.0)
    }
}

/// Reads a size from pixels, or from one of the names the toolbar's four sizes had before a size
/// was a number, which an Outbox entry queued by an earlier version still carries.
impl<'de> Deserialize<'de> for FontSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Px(u64),
            Named(String),
        }
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Px(px) => Self::from_px(u16::try_from(px).unwrap_or(u16::MAX)),
            Raw::Named(name) => match name.as_str() {
                "Small" => Self::SMALL,
                "Large" => Self::LARGE,
                "Huge" => Self::HUGE,
                _ => Self::NORMAL,
            },
        })
    }
}

/// The generic families CSS names without quotes.
const GENERIC_FAMILIES: [&str; 6] = [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
];

/// The most names one family list keeps.
const MAX_NAMES: usize = 8;

/// The longest single font name kept.
const MAX_NAME_LEN: usize = 64;

/// A CSS font-family list: one or more font names, most preferred first.
///
/// Held in one canonical spelling, the names unquoted and joined by `", "`, so a family read back
/// from the HTML it was rendered into compares equal to the one that was rendered. A name may hold
/// letters, digits, spaces, `-`, `_` and `.`, which is every font name in use and leaves nothing
/// that could end a declaration or the `style` attribute it is written into.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FontFamily(String);

impl FontFamily {
    /// Parses a family list in any spelling CSS or a `<font face>` allows, or `None` when it names
    /// nothing, or names something no font is called.
    #[must_use]
    pub fn new(value: &str) -> Option<Self> {
        let mut names = Vec::new();
        for raw in value.split(',') {
            let name = raw.trim().trim_matches(['"', '\'']).trim();
            if name.is_empty() {
                continue;
            }
            if name.chars().count() > MAX_NAME_LEN
                || !name
                    .chars()
                    .all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
            {
                return None;
            }
            names.push(name.split_whitespace().collect::<Vec<_>>().join(" "));
        }
        if names.is_empty() || names.len() > MAX_NAMES {
            return None;
        }
        Some(Self(names.join(", ")))
    }

    /// The canonical list, as stored in the document.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The list as a CSS `font-family` value: each name quoted unless it is a generic family.
    /// Single quotes, because the renderer writes it inside a double-quoted `style` attribute.
    #[must_use]
    pub fn css(&self) -> String {
        self.0
            .split(", ")
            .map(|name| {
                if GENERIC_FAMILIES.contains(&name.to_ascii_lowercase().as_str()) {
                    name.to_owned()
                } else {
                    format!("'{name}'")
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Debug for FontFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FontFamily").field(&self.0).finish()
    }
}

/// Deserializes a family list, mapping one [`FontFamily::new`] refuses to `None` rather than
/// failing the whole document: a typeface is presentation, like a colour.
pub(crate) fn deserialize_family<'de, D>(deserializer: D) -> Result<Option<FontFamily>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(FontFamily::new))
}

#[cfg(test)]
mod tests {
    use super::{FontFamily, FontSize};

    #[test]
    fn a_family_has_one_spelling_however_it_was_written() {
        for spelling in [
            "\"Comic Sans MS\"",
            "'Comic Sans MS'",
            "Comic Sans MS",
            "  Comic   Sans MS ",
        ] {
            assert_eq!(
                FontFamily::new(spelling).map(|family| family.as_str().to_owned()),
                Some("Comic Sans MS".to_owned()),
                "{spelling}"
            );
        }
        let list = FontFamily::new("\"Segoe UI Emoji\",sans-serif").expect("a list");
        assert_eq!(list.as_str(), "Segoe UI Emoji, sans-serif");
        assert_eq!(list.css(), "'Segoe UI Emoji', sans-serif");
    }

    #[test]
    fn a_family_cannot_break_out_of_its_declaration() {
        for hostile in [
            "Arial;color:red",
            "Arial\" onload=\"x",
            "url(http://x)",
            "Arial}",
            "",
            " , ",
        ] {
            assert!(FontFamily::new(hostile).is_none(), "{hostile:?}");
        }
    }

    #[test]
    fn a_size_is_pixels_and_reads_the_names_it_had() {
        let size: FontSize = serde_json::from_str("21").expect("pixels");
        assert_eq!(size.css_px(), 21);
        let named: FontSize = serde_json::from_str("\"Huge\"").expect("a name");
        assert_eq!(named, FontSize::HUGE);
        let huge: FontSize = serde_json::from_str("100000").expect("clamped");
        assert_eq!(huge.css_px(), FontSize::MAX_PX);
        assert_eq!(serde_json::to_string(&FontSize::SMALL).expect("json"), "13");
    }
}
