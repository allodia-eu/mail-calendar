//! Avatar resolution: the letters, the colour, and the stability the colour depends on.

use super::{Avatar, initials, palette_slot, resolve};
use crate::color::PALETTE;

/// The shapes real mail actually carries: a person, a bot, a team, an address with no name,
/// and nothing at all. The last is the one worth stating: the core invents no placeholder,
/// because any word it chose would be untranslatable English baked into a provider-neutral
/// layer and shown verbatim by every client.
#[test]
fn initials_cover_the_shapes_a_mailbox_really_contains() {
    assert_eq!(initials("Ada Lovelace"), "AL");
    assert_eq!(initials("GitHub"), "G");
    assert_eq!(initials("renovate[bot]"), "R");
    assert_eq!(initials("The Google Workspace Team"), "TT");
    assert_eq!(
        initials("  Ada   Lovelace  "),
        "AL",
        "padding is not a word"
    );
    assert_eq!(initials(""), "");
    assert_eq!(initials("   "), "");
}

/// A name can begin with any scalar value, and a two-letter monogram is built from the first
/// *characters* of two words, byte slicing would panic mid-codepoint on most of the world's
/// names. A character whose uppercase form is two characters (`ß` → `SS`) must not be
/// truncated into half a letter either.
#[test]
fn initials_are_built_from_characters_not_bytes() {
    assert_eq!(initials("Édith Piaf"), "ÉP");
    assert_eq!(initials("Ада Лавлейс"), "АЛ");
    assert_eq!(initials("张 伟"), "张伟");
    assert_eq!(initials("😀 Smiley"), "😀S");
    assert_eq!(initials("ßeta"), "SS");
}

/// With no name, the address carries the letter: a row still gets a monogram rather than a
/// blank circle.
#[test]
fn an_address_supplies_the_letter_when_no_name_does() {
    assert_eq!(resolve("", "ada@example.test", None).initials, "A");
    assert_eq!(resolve("   ", "ada@example.test", None).initials, "A");
    assert_eq!(
        resolve("", "", None).initials,
        "",
        "naming nobody yields no letters, and the client draws its own glyph"
    );
}

/// **The test that catches a non-deterministic hasher.** The palette slot is baked into what
/// the user sees, so it has to be a property of the address and nothing else: not of a
/// process, a build, or a toolchain. `DefaultHasher` would pass every same-process assertion
/// and silently recolour the whole mailbox on a Rust upgrade, so the expected slots are
/// written out literally: if the hash ever changes, these fail rather than quietly agreeing
/// with themselves.
#[test]
fn a_palette_slot_is_pinned_to_the_address_and_nothing_else() {
    for (address, slot) in [
        ("ada@example.test", 5),
        ("grace@example.test", 3),
        ("renovate[bot]@example.test", 8),
        ("noreply@example.com", 1),
    ] {
        assert_eq!(palette_slot(address), slot, "slot for {address}");
    }
    // Same input, same answer, however many times it is asked.
    assert_eq!(
        palette_slot("ada@example.test"),
        palette_slot("ada@example.test")
    );
}

/// Case and padding are presentation noise, not identity. `CanonicalEmail` keeps local-part
/// case on purpose (two mailboxes differing only in case may be two people) but a *colour*
/// that flipped when a correspondent capitalised their own address would just look broken.
#[test]
fn the_colour_ignores_case_and_padding_even_though_identity_does_not() {
    let canonical = palette_slot("Ada@Example.test");
    assert_eq!(canonical, palette_slot("ada@example.test"));
    assert_eq!(canonical, palette_slot("  ADA@EXAMPLE.TEST  "));
}

/// Different senders must actually land on different colours often enough to be useful; a
/// hash that collapsed everything into one slot would pass every test above.
#[test]
fn addresses_spread_across_the_palette() {
    let used: std::collections::BTreeSet<usize> = (0..200)
        .map(|n| palette_slot(&format!("person{n}@example.test")))
        .collect();
    assert_eq!(
        used.len(),
        PALETTE.len(),
        "every palette slot should be reachable: {used:?}"
    );
}

/// The letters come from the name and the colour from the address, so two people who share a
/// name are still told apart, and one person keeps their colour whatever a header calls them.
#[test]
fn letters_follow_the_name_and_colour_follows_the_address() {
    let one = resolve("Ada Lovelace", "ada@example.test", None);
    let other = resolve("Ada Lovelace", "ada.l@example.test", None);
    assert_eq!(one.initials, other.initials);
    assert_ne!(
        one.light, other.light,
        "a shared name is not a shared identity"
    );

    let renamed = resolve("A. Lovelace", "ada@example.test", None);
    assert_eq!(
        one.light, renamed.light,
        "the same person keeps their colour whatever the header calls them"
    );
}

/// A photo wins over the monogram, and the monogram survives beside it: a client that cannot
/// decode the file still has letters to fall back on rather than an empty circle.
#[test]
fn a_photo_is_carried_without_discarding_the_monogram() {
    let Avatar {
        initials,
        image_path,
        light,
        ..
    } = resolve(
        "Ada Lovelace",
        "ada@example.test",
        Some("/blobs/ada.blob".into()),
    );
    assert_eq!(image_path.as_deref(), Some("/blobs/ada.blob"));
    assert_eq!(initials, "AL");
    assert!(!light.background.is_empty());
}

/// Every avatar's letters must be legible on their own fill, in both themes. The contrast
/// guarantee lives in `color`, and this is what binds avatars to it rather than to a
/// separately-chosen colour that happens to look fine today.
#[test]
fn every_palette_slot_is_legible_in_both_themes() {
    use crate::color::{Rgb, contrast};

    /// A resolved swatch is always `#rrggbb`, so the test reads it back without needing the
    /// module's own parser to be public.
    fn rgb(hex: &str) -> Rgb {
        let byte = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&hex[range], 16).expect("a resolved swatch is #rrggbb")
        };
        Rgb {
            r: byte(1..3),
            g: byte(3..5),
            b: byte(5..7),
        }
    }

    for n in 0..PALETTE.len() {
        let avatar = resolve("Ada Lovelace", &format!("person{n}@example.test"), None);
        for swatch in [&avatar.light, &avatar.dark] {
            let (bg, fg) = (rgb(&swatch.background), rgb(&swatch.text));
            assert!(
                contrast(bg, fg) >= 4.5,
                "{:?} on {:?} is {:.2}:1",
                swatch.text,
                swatch.background,
                contrast(bg, fg)
            );
        }
    }
}
