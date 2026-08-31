//! A sender's avatar: the monogram, the colour it sits on, and the photo when one is known.
//!
//! Fifty rows of identical envelope glyphs give the eye nothing to find a person by. Every
//! mainstream client draws a small circle at the leading edge of a row instead, and almost
//! never a photograph, because for most correspondents no provider has one. The monogram *is*
//! the feature, and the colour is what does the work: a wall of identical grey circles would
//! be no better than the envelopes.
//!
//! # Why this is decided in the core
//!
//! [`docs/avatars.md`](../../../docs/avatars.md) is the contract. Colour lives here for the
//! same reason calendar contrast does ([`crate::color`]): resolved per client, four clients
//! disagree about whether a white letter is legible on a mid-green fill. What a client keeps
//! is the *shape* (circle, rounded square, size) which is genuinely platform-native.

use crate::color::{self, PALETTE, Swatch};

/// A sender's avatar, resolved for both themes.
///
/// Preference order is photo, then monogram. Never blank and never a silhouette: a row with
/// neither a name nor an address gets empty [`initials`](Self::initials), and the client draws
/// its own platform person glyph rather than the core inventing English placeholder text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avatar {
    /// One or two letters, uppercased. Empty when the row names nobody.
    pub initials: String,
    /// How to draw the monogram in a light theme.
    pub light: Swatch,
    /// How to draw it in a dark theme.
    pub dark: Swatch,
    /// A raster image on disk to draw instead of the monogram, when one is known.
    pub image_path: Option<String>,
}

impl Default for Avatar {
    /// The avatar for nobody: no letters, no photo, and a real colour rather than empty
    /// strings.
    ///
    /// A snapshot exists before any message is opened, and its fields have to be *drawable*
    /// : a swatch of `""` would reach a client as a colour and render as whatever that
    /// platform makes of nonsense. Empty initials already tell a client to draw its own
    /// person glyph.
    fn default() -> Self {
        resolve("", "", None)
    }
}

/// Resolves the avatar for a person named by `name` and `address`.
///
/// `address` decides the colour and `name` the letters, which is deliberate: two people share
/// a name, so colouring by name would give them one identity. Colouring by *person id* would
/// be worse still; it is unknown before contacts sync, and a later merge would recolour a
/// sender under the user.
#[must_use]
pub fn resolve(name: &str, address: &str, image_path: Option<String>) -> Avatar {
    let color = color::resolve(None, None, palette_slot(address));
    Avatar {
        initials: initials(if name.trim().is_empty() {
            address
        } else {
            name
        }),
        light: color.light,
        dark: color.dark,
        image_path,
    }
}

/// One or two letters for the monogram.
///
/// The first character of the first and last whitespace-separated words, so "Ada Lovelace" is
/// `AL` and "Ada" is `A`. `char`s, not bytes: a name can begin with any scalar value, and
/// slicing one by byte index would panic mid-codepoint.
///
/// This is the same derivation the contacts list uses, and reusing it is worth more than
/// matching anyone else's rule; Outlook renders "The Google Workspace Team" as *TG* where
/// this gives *TT*, and two surfaces of our own disagreeing would be the worse bug.
#[must_use]
pub fn initials(display_name: &str) -> String {
    let words: Vec<&str> = display_name.split_whitespace().collect();
    let first = words.first().and_then(|word| word.chars().next());
    let last = (words.len() > 1)
        .then(|| words.last().and_then(|word| word.chars().next()))
        .flatten();
    first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect()
}

/// Picks a palette slot from an address, stably.
///
/// **FNV-1a, never [`std::collections::hash_map::DefaultHasher`].** `DefaultHasher`'s output
/// is explicitly not guaranteed across Rust releases, so a toolchain bump would silently
/// recolour every sender in the user's mailbox: a change nobody could explain and no test
/// would catch, because within one build it is perfectly consistent.
///
/// The address is lowercased first. [`engine_api::CanonicalEmail`] deliberately case-folds
/// only the *domain*, since two mailboxes differing in local-part case may be two people;
/// correct for identity, wrong for colour, where `Ada@example.com` and `ada@example.com`
/// appearing in different colours would just look broken.
fn palette_slot(address: &str) -> usize {
    let digest = address
        .trim()
        .bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            hash.wrapping_mul(0x0100_0000_01b3) ^ u64::from(byte)
        });
    usize::try_from(digest % PALETTE.len() as u64).unwrap_or(0)
}

#[cfg(test)]
#[path = "avatar_tests.rs"]
mod tests;
