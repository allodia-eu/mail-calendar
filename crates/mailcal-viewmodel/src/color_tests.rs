//! Color cases, and the contrast invariant that must hold for every input.

use super::*;

/// WCAG AA for normal text.
const AA: f64 = 4.5;

fn rgb(hex: &str) -> Rgb {
    parse(hex).expect("valid hex")
}

#[test]
fn the_palette_is_valid_and_holds_no_orange() {
    // Allodia Orange is reserved for actions. A calendar tinted into that band would read
    // as a button sitting in the grid.
    for hex in PALETTE {
        let color = parse(hex).expect("palette entries are valid hex");
        assert_eq!(color.to_hex(), hex);
        assert!(
            distance(color, rgb("#F6A24A")) > 5_000.0,
            "{hex} sits too close to Allodia Orange"
        );
    }
}

#[test]
fn a_server_color_snaps_to_the_nearest_palette_hue_not_a_random_one() {
    // A user migrating from Google keeps their coding: their red calendar stays red-ish,
    // their green stays green-ish. Snapping a crimson to a navy would silently destroy
    // years of muscle memory, which is the whole point of honouring the server colour.
    assert_eq!(resolve(None, Some("#ff0000"), 0).hex, "#a85046"); // red -> red
    assert_eq!(resolve(None, Some("#00ff00"), 0).hex, "#3f8f55"); // green -> green
    assert_eq!(resolve(None, Some("#0000ff"), 0).hex, "#4f5ba6"); // blue -> indigo
    assert_eq!(resolve(None, Some("teal"), 0).hex, "#2c8c82"); // a CSS name, too
}

#[test]
fn a_user_override_beats_the_server_color() {
    let color = resolve(Some("#2183A0"), Some("#ff0000"), 0);
    assert_eq!(color.hex, "#2183a0");
}

#[test]
fn nearest_index_reports_which_palette_slot_a_color_claims() {
    // How the calendar cache learns which hues are already spoken for, so a colourless calendar can
    // be given one that is not. It must agree with `resolve`'s own snapping.
    assert_eq!(nearest_index("#ff0000"), Some(5)); // red -> the red slot
    assert_eq!(nearest_index("#0000ff"), Some(1)); // blue -> indigo
    assert_eq!(nearest_index("teal"), Some(8)); // a CSS name resolves too
    // Every palette entry reports its own index.
    for (index, hex) in PALETTE.iter().enumerate() {
        assert_eq!(
            nearest_index(hex),
            Some(index),
            "{hex} should map to slot {index}"
        );
    }
    // Unparseable input lays no claim on a hue.
    assert_eq!(nearest_index("not-a-color"), None);
}

#[test]
fn a_calendar_with_no_color_gets_a_stable_distinct_one_from_its_position() {
    // A server that sends no colours must not give every calendar the same blue.
    let first = resolve(None, None, 0).hex;
    let second = resolve(None, None, 1).hex;
    assert_ne!(first, second);
    // Stable across calls: the colour must not move when the app restarts.
    assert_eq!(resolve(None, None, 1).hex, second);
    // And it wraps rather than panicking past the end of the palette.
    assert_eq!(resolve(None, None, PALETTE.len()).hex, first);
}

#[test]
fn junk_falls_back_to_the_palette_rather_than_rendering_a_guess() {
    for junk in ["", "not-a-color", "#12", "#zzzzzz", "rgb(1,2,3)"] {
        assert_eq!(resolve(None, Some(junk), 3).hex, PALETTE[3]);
    }
}

#[test]
fn shorthand_and_named_colors_parse() {
    assert_eq!(rgb("#abc"), rgb("#aabbcc"));
    assert_eq!(parse("Red").unwrap(), rgb("#ff0000"));
    assert_eq!(parse("  #2F6FA8  ").unwrap(), rgb("#2f6fa8"));
    assert!(parse("chartreuse").is_none());
}

#[test]
fn every_palette_color_is_legible_in_both_themes() {
    for hex in PALETTE {
        let color = resolve(Some(hex), None, 0);
        for (theme, swatch) in [("light", &color.light), ("dark", &color.dark)] {
            let ratio = contrast(rgb(&swatch.background), rgb(&swatch.text));
            assert!(
                ratio >= AA,
                "{hex} in {theme}: label contrast is only {ratio:.2}:1"
            );
        }
    }
}

/// **The invariant.** For *any* background; palette, server-supplied, or a colour a future
/// provider invents: the chosen label clears WCAG AA.
///
/// It holds because the text is pure black or pure white, never an off-black: the
/// worst-case background (the one equidistant from both) still clears 4.58:1 against the
/// better choice. Soften either end and the guarantee is gone, silently, on exactly the
/// mid-tone colours people pick for calendars.
#[test]
fn any_color_at_all_resolves_to_a_legible_label() {
    let mut worst = f64::MAX;
    for r in (0..=255).step_by(15) {
        for g in (0..=255).step_by(15) {
            for b in (0..=255).step_by(15) {
                let background = Rgb { r, g, b };
                let ratio = contrast(background, readable_on(background));
                assert!(
                    ratio >= AA,
                    "{} labels at only {ratio:.2}:1",
                    background.to_hex()
                );
                worst = worst.min(ratio);
            }
        }
    }
    // Pin the worst case: it should sit just above the threshold, not miles clear. If a
    // change to `readable_on` pushed this under 4.5 the assertion above would fire, but
    // recording it here makes the margin visible rather than a matter of luck.
    assert!(
        (4.5..4.7).contains(&worst),
        "the worst-case contrast moved to {worst:.2}:1"
    );
}

#[test]
fn a_dark_swatch_is_sunk_rather_than_left_to_glow() {
    let color = resolve(Some("#2F6FA8"), None, 0);
    // The `hex` field keeps the bright palette colour a picker shows as selected…
    assert_eq!(color.hex, "#2f6fa8");
    // …while the rendered light fill is sunk from it, so white text wins (see below).
    assert!(rgb(&color.light.background).luminance() < rgb("#2f6fa8").luminance());
    // Darker still than the light fill, so a saturated chip does not blaze out of a dark grid.
    assert!(rgb(&color.dark.background).luminance() < rgb(&color.light.background).luminance());
    // And the border stays visible against its own fill in each theme.
    assert_ne!(color.light.border, color.light.background);
    assert_ne!(color.dark.border, color.dark.background);
}

#[test]
fn every_palette_hue_carries_a_white_label_in_light_mode() {
    // The aesthetic decision behind sinking the light fill: a calendar chip is saturated
    // colour under a *white* label, like every mainstream calendar: not the muddy dark-on-
    // green a raw mid-tone hue would fall back to. `every_palette_color_is_legible_in_both_themes`
    // already proves AA; this pins *which* legible choice, so a future palette hue too light
    // for white text trips here instead of silently reintroducing dark labels.
    for hex in PALETTE {
        let color = resolve(Some(hex), None, 0);
        assert_eq!(
            color.light.text, "#ffffff",
            "{hex} should render a white label on its (sunk) light fill"
        );
    }
}
