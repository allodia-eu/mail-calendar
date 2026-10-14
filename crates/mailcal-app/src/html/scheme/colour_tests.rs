//! Where a colour lands on a dark page, and the properties a reader depends on: the page is the
//! canvas, ink is legible on every fill a dark render produces, and a hue survives the move.

use super::colour::{Rgba, Role, darken, hue, lightness, luminance};

fn hex(value: &str) -> Rgba {
    Rgba::from_hex(value.trim_start_matches('#')).expect("a hex colour")
}

/// The colour a dark render gives `value` in `role`; the colour itself when it is left alone.
fn dark(value: &str, role: Role) -> Rgba {
    let colour = hex(value);
    darken(colour, role).unwrap_or(colour)
}

fn contrast(a: Rgba, b: Rgba) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn a_white_page_becomes_the_canvas_and_black_ink_its_ink() {
    // The two that decide whether a plain message looks like part of the pane: its white page is
    // the canvas around it, so there is no seam, and its black text is the canvas's own ink.
    assert_eq!(dark("#ffffff", Role::Fill).to_css(), "#1e1e1e");
    assert_eq!(dark("#000000", Role::Ink).to_css(), "#e3e3e3");
}

#[test]
fn colours_already_right_for_a_dark_page_stay() {
    // A sender's own dark band with white text on it, or their dark-mode sheet, is already what a
    // dark page needs. Moving it would undo their design.
    assert!(darken(hex("#202124"), Role::Fill).is_none());
    assert!(darken(hex("#1a1a1a"), Role::Fill).is_none());
    assert!(darken(hex("#ffffff"), Role::Ink).is_none());
    assert!(darken(hex("#e8eaed"), Role::Ink).is_none());
}

#[test]
fn a_pair_that_read_on_white_still_reads_on_the_dark_page() {
    // The light fills newsletters use, against the inks written on them. A pair the sender made
    // legible must stay legible after the move; a pair that never was (grey on grey) owes nothing.
    // That is the contract `docs/reading-dark-mode.md` makes.
    let fills = [
        "#ffffff", "#f8f9fa", "#f5f5f5", "#eeeeee", "#fff8e1", "#e8f0fe", "#fce8e6", "#c5c8cd",
        "#ffeb3b", "#a8dab5", "#999999",
    ];
    let inks = [
        "#000000", "#202124", "#3c4043", "#5f6368", "#1a1a1a", "#333333", "#666666", "#c5221f",
        "#1967d2", "#188038",
    ];
    for fill in fills {
        for ink in inks {
            if contrast(hex(fill), hex(ink)) < 4.5 {
                continue;
            }
            let (f, i) = (dark(fill, Role::Fill), dark(ink, Role::Ink));
            let ratio = contrast(f, i);
            assert!(
                ratio >= 4.5,
                "{ink} on {fill} became {} on {}: {ratio:.2}",
                i.to_css(),
                f.to_css()
            );
        }
    }
}

#[test]
fn ink_the_sender_left_light_is_raised_to_a_legible_floor() {
    // Google's link blue reads on white and on a dark page alike, but at the edge; anything
    // dimmer than the floor is lifted to it rather than left grey on grey.
    let canvas = dark("#ffffff", Role::Fill);
    for ink in ["#1a73e8", "#4285f4", "#808080", "#5f6368", "#d93025"] {
        let moved = dark(ink, Role::Ink);
        assert!(
            contrast(moved, canvas) >= 4.5,
            "{ink} became {}",
            moved.to_css()
        );
    }
}

#[test]
fn a_colour_keeps_its_hue() {
    // The point of moving lightness alone: a brand's blue band stays blue, a red warning stays red.
    for (value, role) in [
        ("#e8f0fe", Role::Fill),
        ("#fce8e6", Role::Fill),
        ("#1967d2", Role::Ink),
        ("#c5221f", Role::Ink),
        ("#188038", Role::Ink),
    ] {
        let (before, after) = (hex(value), dark(value, role));
        let drift = (hue(before) - hue(after)).abs();
        assert!(
            drift.min(360.0 - drift) < 8.0,
            "{value} drifted {drift:.1}° to {}",
            after.to_css()
        );
    }
}

#[test]
fn a_saturated_fill_is_kept_displayable() {
    // Bright yellow at a dark lightness is outside sRGB; the move reduces its chroma rather than
    // clipping a channel, which would turn it green or brown.
    let moved = dark("#ffeb3b", Role::Fill);
    assert!(lightness(moved) < 0.45, "{}", moved.to_css());
    let drift = (hue(hex("#ffeb3b")) - hue(moved)).abs();
    assert!(drift < 15.0, "{}", moved.to_css());
}

#[test]
fn a_line_swaps_its_lightness_end_for_end() {
    // A pale divider stays subtle against the dark page, rather than becoming a bright rule; a
    // black frame becomes a light one rather than vanishing.
    let canvas = dark("#ffffff", Role::Fill);
    let divider = dark("#dddddd", Role::Line);
    assert!(contrast(divider, canvas) < 2.0, "{}", divider.to_css());
    let frame = dark("#000000", Role::Line);
    assert!(contrast(frame, canvas) > 7.0, "{}", frame.to_css());
}

#[test]
fn alpha_survives_the_move() {
    let tint = Rgba {
        a: 0.5,
        ..hex("#ffffff")
    };
    let moved = darken(tint, Role::Fill).expect("a light tint moves");
    assert_eq!(moved.to_css(), "rgba(30,30,30,0.5)");
}

#[test]
fn every_css_colour_form_parses() {
    let white = Some(hex("#ffffff"));
    assert_eq!(Rgba::from_hex("fff"), white);
    assert_eq!(Rgba::from_hex("ffffffff"), white);
    assert_eq!(Rgba::from_name("White"), white);
    assert_eq!(Rgba::from_name("transparent"), None);
    assert_eq!(Rgba::from_name("solid"), None);
    assert_eq!(Rgba::from_hex("ggg"), None);
    assert_eq!(Rgba::from_hex("ffff1"), None);
}
