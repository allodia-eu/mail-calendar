//! Making a message fit a pane narrower than it was written for, by reflowing the sender's own
//! layout rather than scaling a picture of it. Rule 3 of `docs/reading-zoom.md`.
//!
//! **The problem.** Half of real mail has no `@media` rules at all: a table pinned to 600px in the
//! markup and again in its inline style, cells pinned to a third of that each. Nothing in it
//! adapts, so a 400pt phone pane shows two thirds of every line.
//!
//! **The approach is the mail clients' own**, the "munger" that came from AOSP Email through K-9
//! Mail and Thunderbird for Android, and that Infomaniak's iOS client still runs: try to make the
//! content *reflow* before scaling anything, because a reflowed message is readable at the
//! reader's own text size while a scaled one is a small picture of a message.
//!
//! **Where ours differs is that it needs no script and no measurement.** Those clients run the
//! munger as JavaScript inside the message, which `docs/rendering-security.md` gate 1 forbids
//! here, and feed it the pane's width, which the core does not have. Both go away if the rules are
//! written as CSS the engine evaluates itself:
//!
//! - `max-width:100%` does the whole of "shrink this if the pane is narrower", at every width, with
//!   no number in it. A `<table width="600">` under it is 600px in a wide pane and the pane's width
//!   in a narrow one.
//! - The **destructive** half, clearing the widths on a table's cells, cannot be unconditional: it
//!   would flatten a three-column newsletter that fits perfectly well on a desktop. It is gated
//!   behind a media query whose breakpoint is [`natural_width`], the widest fixed width the message
//!   itself asks for. So the message is untouched in a pane that can hold it, and reflows in one
//!   that cannot, and the engine re-decides that on every resize with nothing re-rendered.
//!
//! What this cannot do is shrink a table below the width its content needs, and nothing can: a
//! cell holding a 300px-wide word is 300px wide. `overflow-wrap` in the base stylesheet is what
//! makes that rare.

/// The widest fixed pixel width the message asks for, if any, which is the pane width below which
/// it cannot lay out as written.
///
/// Read off the text rather than the DOM: this runs on the **sanitised** fragment, where the only
/// widths left are the ones a host would honour, and a scan is a fraction of the cost of a second
/// parse of a message that has already been parsed once.
///
/// What counts is `width` and `min-width`, as an HTML attribute (`width="600"`) or a CSS length
/// (`width:600px`), wherever it appears: an inline style, a `<style>` block, a presentational
/// attribute. What does not:
///
/// - **`max-width`**, which is a promise to shrink rather than a demand for room. Counting it would
///   put the breakpoint at a width the message never needed.
/// - **Percentages and every other unit.** A percentage already adapts, which is the thing being
///   asked about.
/// - **Anything outside [`PLAUSIBLE`]**, so a decorative `<td width="1">` spacer cannot lower the
///   breakpoint and a `width="100000"` cannot raise it past every pane there is.
pub(super) fn natural_width(fragment: &str) -> Option<u32> {
    let bytes = fragment.as_bytes();
    let mut widest = None;
    let mut at = 0;
    while let Some(found) = fragment[at..].find("width") {
        let start = at + found;
        at = start + "width".len();
        if !is_width_property(bytes, start) {
            continue;
        }
        if let Some(width) = pixel_value(&fragment[at..]).filter(|w| PLAUSIBLE.contains(w)) {
            widest = Some(widest.map_or(width, |seen: u32| seen.max(width)));
        }
    }
    widest
}

/// Widths worth building a breakpoint from. The floor is below any pane we draw into, so a spacer
/// cell or a tracking pixel's `width="1"` cannot pull the breakpoint under the real layout; the
/// ceiling is past any reading pane, so a message asking for more than that is one no reflow was
/// ever going to rescue and the media query would be permanently on.
const PLAUSIBLE: std::ops::RangeInclusive<u32> = 200..=2000;

/// Whether the `width` starting at `start` is the property/attribute itself rather than the tail
/// of another word.
///
/// The one that matters is **`max-width`**, which ends in `width` and means the opposite. `min-`
/// is kept: a minimum width is a demand for room exactly as `width` is.
fn is_width_property(bytes: &[u8], start: usize) -> bool {
    let before = |n: usize| start.checked_sub(n).map(|i| &bytes[i..start]);
    if before(4) == Some(b"min-") {
        return true;
    }
    // A letter or a hyphen in front makes this the end of some other word (`max-width`,
    // `borderwidth`); anything else, including the start of the fragment, makes it the word.
    !matches!(bytes.get(start.wrapping_sub(1)), Some(c) if c.is_ascii_alphanumeric() || *c == b'-')
}

/// The pixel value `rest` opens with, for `rest` taken straight after a `width` token: `="600"`,
/// `: 600px`, `=600`. `None` for any other unit, a percentage, or no number at all.
fn pixel_value(rest: &str) -> Option<u32> {
    let mut chars = rest.char_indices().skip_while(|(_, c)| c.is_whitespace());
    // The separator, `=` for an attribute and `:` for a declaration.
    let (_, separator) = chars.next()?;
    if separator != '=' && separator != ':' {
        return None;
    }
    let rest = &rest[chars.next().map_or(rest.len(), |(i, _)| i)..];
    let rest = rest
        .trim_start()
        .trim_start_matches(['"', '\''])
        .trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let unit = rest[digits.len()..].trim_start();
    // A bare number is an HTML attribute's pixels; `px` is the only CSS length a fixed mail layout
    // is written in, and every other unit (`%`, `em`, `pt`, `vw`) either adapts already or is too
    // rare here to guess at.
    let is_pixels = unit.starts_with("px")
        || unit.starts_with(['"', '\'', ';', '>', '/'])
        || unit.is_empty()
        || unit.starts_with(char::is_whitespace);
    is_pixels.then(|| digits.parse().ok()).flatten()
}

/// The stylesheet that reflows an over-wide message, given what [`natural_width`] found.
///
/// Two halves, and which is which is decided by whether the rule can hurt a message that already
/// fits:
///
/// **Always on, because it can only ever shrink something.** `max-width:100%` caps every box that
/// carries a layout at its container, so an over-wide one narrows and one that fits is untouched.
/// It cascades: the outermost offender is capped to the body, its children to it, down the tree.
///
/// **Behind the breakpoint, because it overrides what the sender wrote.** A table whose cells have
/// fixed widths cannot narrow, however small its own `max-width`, because a table is never
/// narrower than its columns; the widths have to go, and with them `table-layout:fixed` and the
/// `nowrap` that keeps a cell from breaking a line. On a pane that can hold the message none of
/// that fires, so a newsletter with three real columns still has three real columns on a desktop.
///
/// `!important` throughout the gated half: the widths it exists to override are inline attributes
/// and inline styles, which beat any stylesheet rule without it.
pub(super) fn reflow_css(natural_width: Option<u32>) -> String {
    use std::fmt::Write as _;

    let mut css = String::from(ALWAYS);
    if let Some(width) = natural_width {
        let _ = write!(
            css,
            "@media (max-width:{}px){{{NARROW}}}",
            breakpoint(width)
        );
    }
    css
}

/// The pane width at or below which the message stops fitting, which is what it asks for plus the
/// room the document takes from both edges ([`super::BODY_PADDING`]).
///
/// ⚠️ **The padding is the whole point of this function.** A 600px message in a 612pt pane has
/// 584pt to lay out in and is clipped, and a breakpoint at 600 leaves it that way: a 28px band of
/// pane widths where the message does not fit and nothing reflows it. Found on a 13-inch iPad,
/// whose reading pane lands in exactly that band, and invisible on every narrower one.
fn breakpoint(natural_width: u32) -> u32 {
    natural_width + 2 * super::document::BODY_PADDING
}

/// The half that can only shrink. `figure` and `pre` are here because both routinely carry a
/// fixed width in mail and neither is a table.
const ALWAYS: &str = "table,td,th,div,p,blockquote,pre,figure{max-width:100%}";

/// The half that overrides the sender, in a pane too narrow for what they wrote.
///
/// **A table is as wide as its columns**, whatever `max-width` says, and a table inside a table
/// cell is worse than that: the cell's own width is decided by its contents, so a 600px table in a
/// cell makes the cell 600px and `max-width:100%` of that is 600px again. Only clearing the widths
/// breaks the circle, which is what the munger does and why it has to.
///
/// ⚠️ **Clearing them means clearing the right ones.** `width:auto` on a table shrinks it to its
/// contents, and the full-bleed band, `<table width="100%" bgcolor>` wrapping a column, is in half
/// the newsletters ever sent: clear that one and the colour stops reaching the edges while the
/// text inside it still looks right. So the selector asks for a width attribute that is **not** a
/// percentage, which is exactly "a width that cannot adapt on its own":
///
/// - `<table width="600">` → cleared, and the `!important` beats the `style="width:600px"` beside
///   it, because an important rule in a stylesheet outranks an inline declaration.
/// - `<table width="100%">` → left alone.
/// - `<table style="width:100%">` with no attribute → left alone, because `[width]` asks for the
///   attribute's presence first.
///
/// The same clause leaves a **style-only** fixed width (`style="width:600px"` and no attribute)
/// to `max-width` alone, which is a real limit and a small one: a table width in mail is written
/// as the attribute, with or without a style beside it, because Outlook has never honoured
/// anything else.
const NARROW: &str = "table{table-layout:auto!important}\
                      table[width]:not([width$=\"%\"]),\
                      td[width]:not([width$=\"%\"]),\
                      th[width]:not([width$=\"%\"])\
                      {width:auto!important;min-width:0!important}\
                      td,th{white-space:normal!important}";

#[cfg(test)]
mod tests;
