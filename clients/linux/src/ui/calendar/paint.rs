//! Colours, rectangles, and how an unanswered invitation looks on a calendar surface.
//!
//! The core decides *which* records are holds (`participation == NeedsAction`,
//! `docs/invitations.md`); this file is only the drawing, kept in one place so the grid block, the
//! all-day bar, the month chip and the invitation card's preview cannot drift apart. The
//! deliberate twin of Windows' `Calendar/CalendarHold.cs` and Android's
//! `CalendarParticipation.kt`, constant for constant.
//!
//! Every piece here is a no-op on an answered record, so nothing about a confirmed commitment's
//! appearance changes: a hold is told apart by shape, not by a restyle of everything around it.
//!
//! **Two renderers, one set of numbers.** The time grid, the all-day banner and the card's preview
//! are Cairo; the month chips are GTK buttons styled by CSS, because a `DrawingArea` per chip would
//! cost far more than a hatch is worth. So the hatch exists twice, immediately below each other,
//! over one set of constants; change the pitch and both move by construction.
//!
//! The visual is never the whole disclosure. A dashed border is invisible to a screen reader, so
//! every surface that draws a hold also says it: [`spoken_with_hold`] appends "Awaiting your
//! response" (`docs/calendar.md` §4, the spoken-grid rule).

use gtk::cairo;
use mailcal_bindings::ResponseStatus;

use crate::l10n;

/// The width of the hatched gutter down a hold's leading edge, and the diagonals' pitch.
const GUTTER: f64 = 4.0;
const HATCH_STEP: f64 = 4.0;

/// The hairline every surface strokes with; solid on a commitment, dashed on a hold.
const EDGE_THICKNESS: f64 = 1.0;

/// The dash a hold's border is stroked with: on, then off.
const DASH: [f64; 2] = [3.0, 2.0];

/// How much of a hold's colour survives; enough to keep its calendar identifiable, little enough
/// that it does not read as a confirmed commitment beside one.
pub(in crate::ui) const HOLD_FILL_ALPHA: f64 = 0.4;

/// Whether a calendar record is an invitation this account has not answered; the one condition
/// that turns on the provisional drawing.
///
/// `Declined` never reaches a client: the core hides those from every calendar surface. If one
/// ever did, it is not a hold either.
pub(in crate::ui) fn is_awaiting(participation: ResponseStatus) -> bool {
    matches!(participation, ResponseStatus::NeedsAction)
}

/// A calendar record's spoken label, with the hold said out loud when there is one.
///
/// Shared by the grid block, the all-day bar, the month chip and the agenda row, so one rule
/// covers every surface that can show a hold.
pub(in crate::ui) fn spoken_with_hold(label: &str, participation: ResponseStatus) -> String {
    if is_awaiting(participation) {
        format!("{label}, {}", l10n::a11y_invitation_awaiting_response())
    } else {
        label.to_owned()
    }
}

/// A fill's alpha after the hold treatment: faded on an unanswered invitation, untouched on a
/// commitment.
pub(in crate::ui) fn hold_alpha(awaiting: bool) -> f64 {
    if awaiting { HOLD_FILL_ALPHA } else { 1.0 }
}

/// The dash a record's edge is stroked with: dashed for an unanswered hold, the grid's own
/// hairline otherwise.
///
/// A record that already draws a border strokes it with this rather than gaining a second; which
/// is why the grid block calls this and [`hatch`], while a surface with no border of its own calls
/// [`hatch_and_dash`] instead. Always sets the dash, including the empty one, so a hold does not
/// leak its dashes onto the next block drawn with the same context.
pub(in crate::ui) fn set_dash(context: &cairo::Context, awaiting: bool) {
    if awaiting {
        context.set_dash(&DASH, 0.0);
    } else {
        context.set_dash(&[], 0.0);
    }
}

/// The diagonal hatching down a hold's leading edge; the part of the treatment that survives being
/// looked at quickly, when a dashed border at a zoomed-out hour height does not.
///
/// Draws nothing unless `awaiting`, so a commitment costs one test.
pub(in crate::ui) fn hatch(context: &cairo::Context, rect: Rect, color: Rgb, awaiting: bool) {
    if !awaiting {
        return;
    }
    let width = GUTTER.min(rect.width);
    let height = rect.height;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let _ = context.save();
    context.rectangle(rect.x, rect.y, width, height);
    context.clip();
    set_source(context, color);
    context.set_dash(&[], 0.0);
    context.set_line_width(EDGE_THICKNESS);
    // Start a full height to the left, so the first stripe already crosses the strip rather than
    // leaving its top corner bare.
    let mut x = rect.x - height;
    while x < rect.x + width + height {
        context.move_to(x, rect.y + height);
        context.line_to(x + height, rect.y);
        x += HATCH_STEP;
    }
    let _ = context.stroke();
    let _ = context.restore();
}

/// The whole treatment for a surface that has **no** border of its own; the hatched gutter and the
/// dashed edge in one call. A no-op on a commitment.
pub(in crate::ui) fn hatch_and_dash(
    context: &cairo::Context,
    rect: Rect,
    color: Rgb,
    awaiting: bool,
) {
    if !awaiting {
        return;
    }
    hatch(context, rect, color, true);
    // Inset by half the stroke, which straddles the path it is given: drawn on the boundary its
    // outer half falls outside the block and is clipped away, leaving a half-weight dash that reads
    // as a rendering artefact rather than as a deliberate edge.
    let half = EDGE_THICKNESS / 2.0;
    if rect.width <= half * 2.0 || rect.height <= half * 2.0 {
        return;
    }
    let _ = context.save();
    set_source(context, color);
    set_dash(context, true);
    context.set_line_width(EDGE_THICKNESS);
    context.rectangle(
        rect.x + half,
        rect.y + half,
        rect.width - half * 2.0,
        rect.height - half * 2.0,
    );
    let _ = context.stroke();
    let _ = context.restore();
}

/// The same treatment for the **composed** surface: the month grid's chips, which are GTK buttons
/// rather than draw calls.
///
/// Returns nothing on a commitment, so a month cell of ordinary appointments is
/// declaration-for-declaration what it was. A chip is a few points tall, so the hatch is what
/// survives and the dashes are the decoration that rides beside it; the leading gutter is a
/// `repeating-linear-gradient` sized to [`GUTTER`], which is the only way to clip a hatch to a
/// strip in GTK CSS.
pub(in crate::ui) fn hold_css(background: &str, border: &str) -> String {
    format!(
        "background-color: {background}; \
         background-image: repeating-linear-gradient(135deg, {border} 0px, {border} \
         {EDGE_THICKNESS}px, transparent {EDGE_THICKNESS}px, transparent {HATCH_STEP}px); \
         background-size: {GUTTER}px 100%; background-repeat: no-repeat; \
         background-position: left top; border: {EDGE_THICKNESS}px dashed {border};"
    )
}

/// A rectangle in widget coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct Rect {
    pub(in crate::ui) x: f64,
    pub(in crate::ui) y: f64,
    pub(in crate::ui) width: f64,
    pub(in crate::ui) height: f64,
}

/// An opaque colour, as the core's `#rrggbb` swatches resolve to.
#[derive(Clone, Copy, Debug)]
pub(in crate::ui) struct Rgb {
    pub(in crate::ui) red: f64,
    pub(in crate::ui) green: f64,
    pub(in crate::ui) blue: f64,
}

impl Rgb {
    pub(in crate::ui) const fn new(red: f64, green: f64, blue: f64) -> Self {
        Self { red, green, blue }
    }

    pub(in crate::ui) fn from_hex(value: &str) -> Self {
        let value = value.strip_prefix('#').unwrap_or(value);
        if value.len() != 6 {
            return Self::new(0.086, 0.349, 0.553);
        }
        let component = |range| {
            u8::from_str_radix(&value[range], 16).map_or(0.0, |number| f64::from(number) / 255.0)
        };
        Self::new(component(0..2), component(2..4), component(4..6))
    }
}

/// The colour a block draws in when no calendar owns it: Allodia Blue, white text, a darker edge.
///
/// The full grid falls back to it for a row whose calendar is not in the page's list; the
/// invitation card's preview uses it for **every** block, because that preview carries no calendar
/// list at all; it is one day, not a page, and none of its blocks is tappable.
pub(in crate::ui) fn neutral_swatch() -> (Rgb, Rgb, Rgb) {
    (
        Rgb::from_hex("#16598D"),
        Rgb::new(1.0, 1.0, 1.0),
        Rgb::from_hex("#0f3e63"),
    )
}

pub(in crate::ui) fn set_source(context: &cairo::Context, color: Rgb) {
    context.set_source_rgb(color.red, color.green, color.blue);
}

/// Fills `rect`, faded to [`HOLD_FILL_ALPHA`] when the record is an unanswered hold.
pub(in crate::ui) fn fill_rect(context: &cairo::Context, rect: Rect, color: Rgb, awaiting: bool) {
    context.set_source_rgba(color.red, color.green, color.blue, hold_alpha(awaiting));
    context.rectangle(rect.x, rect.y, rect.width.max(1.0), rect.height.max(2.0));
    let _ = context.fill();
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::ResponseStatus;

    use super::{HOLD_FILL_ALPHA, hold_alpha, hold_css, is_awaiting, spoken_with_hold};

    #[test]
    fn only_an_unanswered_invitation_is_a_hold() {
        assert!(is_awaiting(ResponseStatus::NeedsAction));
        for answered in [
            ResponseStatus::Accepted,
            ResponseStatus::Tentative,
            ResponseStatus::Delegated,
            // Never reaches a client; the core hides it; and is not a hold if one ever did.
            ResponseStatus::Declined,
        ] {
            assert!(!is_awaiting(answered));
        }
    }

    #[test]
    fn a_hold_says_so_out_loud_and_a_commitment_is_left_alone() {
        assert_eq!(
            spoken_with_hold(
                "Sprint planning, 10:00–11:00, Work",
                ResponseStatus::NeedsAction
            ),
            format!(
                "Sprint planning, 10:00–11:00, Work, {}",
                crate::l10n::a11y_invitation_awaiting_response()
            )
        );
        assert_eq!(
            spoken_with_hold("Sprint planning", ResponseStatus::Accepted),
            "Sprint planning"
        );
    }

    #[test]
    fn a_commitment_keeps_its_whole_fill_and_a_hold_is_faded_once() {
        assert!((hold_alpha(false) - 1.0).abs() < f64::EPSILON);
        assert!((hold_alpha(true) - HOLD_FILL_ALPHA).abs() < f64::EPSILON);
    }

    #[test]
    fn the_composed_hatch_is_clipped_to_the_leading_gutter_and_dashes_its_edge() {
        let css = hold_css("rgba(22, 89, 141, 0.4)", "#0f3e63");
        assert!(css.contains("repeating-linear-gradient(135deg"));
        // Sized to the gutter and drawn once: a gradient that tiled would hatch the whole chip.
        assert!(css.contains("background-size: 4px 100%"));
        assert!(css.contains("background-repeat: no-repeat"));
        assert!(css.contains("border: 1px dashed #0f3e63"));
    }
}
