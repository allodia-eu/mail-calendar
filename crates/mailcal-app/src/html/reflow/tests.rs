//! What the reflow stylesheet is built from: which widths in a message count as "the width this
//! message was written for", and which of the two halves of the sheet each one reaches.
//!
//! The rules themselves are a question about an engine, so what a real message does with them is
//! checked on each client rather than here; `docs/reading-zoom.md` says where.

use super::{natural_width, reflow_css};

#[test]
fn reads_the_width_off_a_presentational_attribute() {
    // The commonest fixed-width newsletter there is: a table pinned in the markup.
    assert_eq!(
        natural_width(r#"<table width="600"><tr><td>x</td></tr></table>"#),
        Some(600)
    );
    // Unquoted and single-quoted are both legal HTML and both appear in real mail.
    assert_eq!(natural_width("<table width=600>"), Some(600));
    assert_eq!(natural_width("<table width='600'>"), Some(600));
}

#[test]
fn reads_the_width_off_a_css_length() {
    assert_eq!(
        natural_width(r#"<div style="width:600px">x</div>"#),
        Some(600)
    );
    // Whitespace is legal everywhere a declaration can carry it.
    assert_eq!(
        natural_width(r#"<div style="width : 600px ;">x</div>"#),
        Some(600)
    );
    // A <style> block is the same text to a scan, which is the point of scanning rather than
    // walking a DOM: a newsletter's container width is as often in a rule as in an attribute.
    assert_eq!(
        natural_width("<style>.wrap{width:600px}</style><div class=wrap>x</div>"),
        Some(600)
    );
}

#[test]
fn min_width_counts_and_max_width_does_not() {
    // `min-width` is a demand for room exactly as `width` is.
    assert_eq!(
        natural_width(r#"<div style="min-width:600px">x</div>"#),
        Some(600)
    );
    // `max-width` is a promise to shrink. Counting it would put the breakpoint at a width the
    // message never needed, so a message that already adapts would be reflowed anyway.
    assert_eq!(
        natural_width(r#"<div style="max-width:600px">x</div>"#),
        None
    );
    // And the two together are the standard responsive idiom, which needs nothing from us.
    assert_eq!(
        natural_width(r#"<img style="max-width:600px;width:100%">"#),
        None
    );
}

#[test]
fn a_width_that_already_adapts_is_not_a_width() {
    for adapting in [
        r#"<table width="100%">"#,
        r#"<div style="width:100%">"#,
        r#"<div style="width:40em">"#,
        r#"<div style="width:80vw">"#,
        r#"<div style="width:auto">"#,
        r#"<div style="width:calc(100% - 20px)">"#,
    ] {
        assert_eq!(natural_width(adapting), None, "{adapting}");
    }
}

#[test]
fn the_widest_width_is_the_one_that_decides() {
    // A message is only as adaptable as its widest fixed box, so the breakpoint is that box.
    let out = natural_width(
        r#"<table width="600"><tr><td width="184">a</td><td style="width:900px">b</td></tr></table>"#,
    );
    assert_eq!(out, Some(900));
}

#[test]
fn implausible_widths_cannot_move_the_breakpoint() {
    // A spacer cell and a tracking pixel are in half the newsletters ever sent. Counted, they
    // would put the breakpoint under every pane and the gated rules would never fire.
    assert_eq!(
        natural_width(r#"<td width="1"><img width="1" height="1"></td>"#),
        None
    );
    // And nothing rescues a message asking for more than any pane; a breakpoint there would just
    // be permanently on.
    assert_eq!(
        natural_width(r#"<div style="width:100000px">x</div>"#),
        None
    );
    // The plausible range is inclusive at both ends.
    assert_eq!(
        natural_width(r#"<div style="width:200px">x</div>"#),
        Some(200)
    );
    assert_eq!(
        natural_width(r#"<div style="width:2000px">x</div>"#),
        Some(2000)
    );
    assert_eq!(natural_width(r#"<div style="width:199px">x</div>"#), None);
    assert_eq!(natural_width(r#"<div style="width:2001px">x</div>"#), None);
}

#[test]
fn a_word_that_merely_ends_in_width_is_not_a_width() {
    // `max-width` is the one that matters and has its own test; these are the rest of the family.
    // A scan that took them would read a border's 3px as the message's natural width.
    for other in [
        r#"<div style="border-width:600px">x</div>"#,
        r#"<div style="outline-width:600px">x</div>"#,
        r#"<div style="column-width:600px">x</div>"#,
        r#"<div data-fullwidth="600">x</div>"#,
    ] {
        assert_eq!(natural_width(other), None, "{other}");
    }
}

#[test]
fn a_message_with_nothing_fixed_has_no_natural_width() {
    assert_eq!(
        natural_width("<p>Just a note, with no layout at all.</p>"),
        None
    );
    assert_eq!(natural_width(""), None);
}

#[test]
fn the_scan_is_byte_safe_on_hostile_input() {
    // The fragment is a stranger's mail. Every slice the scan takes has to land on a character
    // boundary, or reading a message is a panic in the core.
    for hostile in [
        "width",
        "width:",
        "width :",
        "width=",
        "width:é",
        "width:600é",
        "…width:600px…",
        "min-width",
        "<div style=\"width:600px\">日本語のメール</div>",
    ] {
        let _ = natural_width(hostile);
    }
}

#[test]
fn the_sheet_shrinks_always_and_overrides_only_below_the_breakpoint() {
    // Without a fixed width there is nothing to reflow, so the message gets the shrinking half
    // and no media query at all: a rule that cannot fire is a rule not worth sending.
    let open = reflow_css(None);
    assert!(open.contains("max-width:100%"), "{open}");
    assert!(!open.contains("@media"), "{open}");

    // With one, the destructive half arrives behind a breakpoint at the width the message needs,
    // which is what it asks for plus the room the document takes from both edges.
    let gated = reflow_css(Some(600));
    assert!(gated.contains("max-width:100%"), "{gated}");
    assert!(gated.contains("@media (max-width:628px)"), "{gated}");
    // The widths a table's own cells pin are what stops it narrowing, so they are what the gated
    // half clears, and `!important` because every one of them is written inline.
    assert!(gated.contains("width:auto!important"), "{gated}");
    assert!(gated.contains("white-space:normal!important"), "{gated}");

    // The overrides belong to the query rather than sitting loose before it, or a three-column
    // newsletter would lose its columns on a desktop.
    let (before, inside) = gated.split_once("@media").expect("a breakpoint");
    assert!(!before.contains("!important"), "{before}");
    assert!(inside.contains("!important"), "{inside}");
}

#[test]
fn the_gated_half_clears_a_fixed_width_and_leaves_a_relative_one() {
    // A table is as wide as its columns whatever `max-width` says, and a table inside a cell makes
    // that circular, so the fixed widths have to go or the column never narrows.
    //
    // But `width:auto` shrinks a table to its contents, and the full-bleed band,
    // `<table width="100%" bgcolor>` wrapping a column, is in half the newsletters ever sent.
    // Clearing that one stops the colour reaching the edges, and it would do it because of a fixed
    // width somewhere else entirely in the message. So the selector asks for a width that cannot
    // adapt on its own: an attribute, and not a percentage.
    let narrow = reflow_css(Some(600))
        .split_once("@media")
        .expect("a breakpoint")
        .1
        .to_owned();
    assert!(
        narrow.contains(r#"table[width]:not([width$="%"])"#),
        "{narrow}"
    );
    assert!(
        narrow.contains(r#"td[width]:not([width$="%"])"#),
        "{narrow}"
    );
    assert!(narrow.contains("width:auto!important"), "{narrow}");
    // A bare `table{...width:auto}` would take the bands with it.
    assert!(
        !narrow.contains("table{table-layout:auto!important;width:auto"),
        "{narrow}"
    );
    // The cell rule that lets a line break is not width-conditional: a `nowrap` cell holds a
    // column open however its width was written.
    assert!(
        narrow.contains("td,th{white-space:normal!important}"),
        "{narrow}"
    );
}

#[test]
fn the_breakpoint_leaves_no_band_where_a_message_is_clipped_and_nothing_reflows() {
    // A 600px message in a 612pt pane has 584pt to lay out in, because the document insets itself
    // by 14px on each side, so it is clipped. A breakpoint at 600 leaves it clipped: a 28px band
    // of pane widths where nothing fits and nothing fires. Found on a 13-inch iPad, whose reading
    // pane lands in exactly that band, and invisible on every narrower device.
    let gated = reflow_css(Some(600));
    let breakpoint = gated
        .split_once("@media (max-width:")
        .expect("a breakpoint")
        .1
        .split_once("px)")
        .expect("a length")
        .0
        .parse::<u32>()
        .expect("a number");
    assert_eq!(breakpoint, 600 + 2 * super::super::document::BODY_PADDING);
    assert!(
        breakpoint > 612,
        "the 13-inch iPad's pane is inside the query"
    );
}

#[test]
fn the_seeded_newsletters_are_read_the_way_they_are_written() {
    // Both halves of how real newsletters are built, taken from the harness fixtures
    // (`docker/stalwart/seed/mail/11-*.eml` and `12-*.eml`) rather than invented here, because a
    // scanner that only ever meets its own test cases is a scanner that agrees with itself.

    // A column pinned in the markup and again inline, with its cells pinned too.
    let pinned = r#"<table width="600" cellpadding="0" cellspacing="0" border="0"
             style="width:600px;background:#ffffff;font-family:Georgia,serif;">
             <tr><td style="padding:24px;">
             <table width="552" style="width:552px;border-collapse:collapse;">
             <tr><td style="border:1px solid #d9d9d9;padding:8px;width:184px;">First</td></tr>
             </table></td></tr></table>"#;
    assert_eq!(natural_width(pinned), Some(600));

    // Full-bleed bands wrapping the same column. The bands are relative and must not decide the
    // breakpoint; the column is fixed and must.
    let banded = r#"<table width="100%" cellpadding="0" cellspacing="0" border="0"
             style="background:#16598d;"><tr><td align="center" style="padding:18px;">
             <table width="600" style="width:600px;"><tr>
             <td width="200">First of three</td></tr></table></td></tr></table>"#;
    assert_eq!(natural_width(banded), Some(600));
}

#[test]
fn a_width_in_a_url_is_not_the_message_its_width() {
    // An image CDN's query parameter carries the token, the separator and a plausible number, and
    // it is in a great deal of real mail. Read as a layout width it would put the breakpoint past
    // every desktop pane, so the destructive half would fire on a three-column newsletter that fits
    // perfectly well.
    for url in [
        r#"<img src="https://cdn.example/p.jpg?width=1200">"#,
        r#"<img src="https://cdn.example/p.jpg?h=200&width=1200">"#,
        r#"<a href="https://example.com/width=1200/x">x</a>"#,
    ] {
        assert_eq!(natural_width(url), None, "{url}");
    }
    // And the attribute beside one is still read.
    assert_eq!(
        natural_width(r#"<img src="https://cdn.example/p.jpg?width=1200" width="600">"#),
        Some(600)
    );
}

#[test]
fn a_media_feature_is_a_question_about_the_pane_not_a_demand_for_room() {
    // `min-width` in a declaration is a demand for room. In a query condition it is the opposite:
    // the message is saying what it does *when* the pane is that wide, and it already adapts.
    for query in [
        "<style>@media (min-width:700px){.c{float:left}}</style>",
        "<style>@media screen and (min-width:700px){.c{float:left}}</style>",
    ] {
        assert_eq!(natural_width(query), None, "{query}");
    }
}

#[test]
fn prose_about_css_is_not_layout() {
    // The fragment is a stranger's mail, and a newsletter about web design, or a quoted reply
    // carrying one, writes lengths in its own text. Only a tag's attributes and a `<style>` body
    // are layout.
    assert_eq!(
        natural_width("<p>Set width: 1200px on the container.</p>"),
        None
    );
    // A `<style>` block's body is layout, and still read.
    assert_eq!(
        natural_width("<style>.wrap{width:600px}</style><p>width: 1200px in prose</p>"),
        Some(600)
    );
}

#[test]
fn prose_after_a_style_block_is_still_prose() {
    // `</style>` names the same element as `<style>`, so a region split that asks only for the
    // name hands the whole rest of the message back as stylesheet, and the sender's own words
    // decide the breakpoint after all. Only an opening tag opens a body.
    assert_eq!(
        natural_width("<style>.a{color:red}</style><p>Set width: 1200px on the container.</p>"),
        None
    );
    // And the block's own body is still read, before and after that closing tag exists.
    assert_eq!(
        natural_width("<style>.a{width:600px}</style><p>Set width: 1200px on it.</p>"),
        Some(600)
    );
}

#[test]
fn a_tag_broken_by_a_raw_angle_bracket_does_not_lose_the_rest() {
    // `>` is legal unescaped inside an attribute value, so a split on the first one can cut a tag
    // in half. What matters is that the scan picks up again at the next `<` rather than giving up
    // on the message: the widths after the broken tag still decide the breakpoint.
    assert_eq!(
        natural_width(r#"<img alt="a > b"><table width="600"><tr><td>x</td></tr></table>"#),
        Some(600)
    );
}
