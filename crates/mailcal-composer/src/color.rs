//! The colour a text run is painted in, and the one it is highlighted with.
//!
//! A single spelling (`#rrggbb`, lowercase) reaches the outgoing HTML, because the editor is not
//! the only thing that can put a colour in a document: a pasted or imported run may carry any of
//! the half-dozen forms CSS accepts, and an outgoing `style="color:…"` is read by mail clients
//! whose CSS support stops well short of the full grammar. Normalising at the boundary means the
//! renderer has one case to emit and no message ever ships a colour a reader cannot parse.

use core::fmt;

use serde::{Deserialize, Serialize};

/// A validated `#rrggbb` colour.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct TextColor(String);

impl TextColor {
    /// Creates a colour from `#rgb` or `#rrggbb`, in either case, normalised to lowercase
    /// `#rrggbb`. Anything else is `None`.
    ///
    /// Deliberately narrow: no named colours, no `rgb()`, no alpha. The set of colour keywords a
    /// mail client recognises is not the set a browser does, and an alpha channel has no meaning
    /// once the message is composited onto whatever background the reader chose. The editor
    /// normalises what it can and drops the rest before it ever gets here.
    #[must_use]
    pub fn new(value: &str) -> Option<Self> {
        let digits = value.trim().strip_prefix('#')?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let expanded = match digits.len() {
            3 => digits.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => digits.to_owned(),
            _ => return None,
        };
        Some(Self(format!("#{}", expanded.to_ascii_lowercase())))
    }

    /// The colour as `#rrggbb`, ready to place in a CSS declaration.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TextColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TextColor").field(&self.0).finish()
    }
}

/// Deserializes an optional colour, mapping anything unparseable to `None`.
///
/// Lenient on purpose, matching `deserialize_clamped_width`: a colour is presentation, and refusing
/// to send a message because one run carries a spelling we do not accept trades a cosmetic loss for
/// a functional one. The run renders in the inherited colour instead.
pub(crate) fn deserialize_color<'de, D>(deserializer: D) -> Result<Option<TextColor>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(TextColor::new))
}

#[cfg(test)]
mod tests {
    use super::TextColor;

    #[test]
    fn accepts_both_hex_lengths_and_normalizes_case() {
        assert_eq!(TextColor::new("#FF0000").expect("hex").as_str(), "#ff0000");
        assert_eq!(
            TextColor::new("#f00").expect("short hex").as_str(),
            "#ff0000"
        );
        assert_eq!(
            TextColor::new("  #AbCdEf  ").expect("padded").as_str(),
            "#abcdef"
        );
    }

    #[test]
    fn rejects_everything_that_is_not_hex() {
        for bad in [
            "red",
            "rgb(255,0,0)",
            "#ff00",
            "#gggggg",
            "ff0000",
            "#",
            "",
            "#ff0000;color:red",
        ] {
            assert!(
                TextColor::new(bad).is_none(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn a_rejected_colour_cannot_break_out_of_the_style_attribute() {
        // The renderer places the value straight into `style="…"`, so the only thing standing
        // between a hostile document and an injected declaration is this validator.
        assert!(TextColor::new("#fff\" onload=\"x").is_none());
        assert!(TextColor::new("#fff;background:url(http://x)").is_none());
    }
}
