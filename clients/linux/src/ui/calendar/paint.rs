//! Colours, rectangles, how an event wears its calendar's colour, and how an unanswered invitation
//! looks on a calendar surface.
//!
//! The core decides *which* records are holds (`participation == NeedsAction`,
//! `docs/invitations.md`) and which colours an event wears (`calendar_event_swatch`,
//! `docs/calendar.md` §1); this file is only the drawing, kept in one place so the grid block, the
//! all-day bar, the month chip and the invitation card's preview cannot drift apart. The
//! deliberate twin of Windows' `Calendar/CalendarHold.cs` and Android's
//! `CalendarParticipation.kt`, constant for constant.
//!
//! A hold is told apart by shape: the bar of the calendar's colour becomes a hatch of it, the fill
//! fades and the edge is dashed, and nothing else about the block changes.
//!
//! **Two renderers, one set of numbers.** The time grid, the all-day banner and the card's preview
//! are Cairo; the month chips are GTK buttons styled by CSS, because a `DrawingArea` per chip would
//! cost far more than a hatch is worth. So the hatch exists twice, immediately below each other,
//! over one set of constants; change the pitch and both move by construction.
//!
//! The visual is never the whole disclosure. A dashed border is invisible to a screen reader, so
//! every surface that draws a hold also says it: [`spoken_with_hold`] appends "Awaiting your
//! response" (`docs/calendar.md` §4, the spoken-grid rule).

use std::{cell::RefCell, collections::HashMap};

use gtk::cairo;
use mailcal_bindings::ResponseStatus;

use super::blocks::{BAR_WIDTH, CORNER};
use crate::l10n;

/// The width of the hatched gutter down a hold's leading edge, which takes the bar's place, and
/// the diagonals' pitch.
const GUTTER: f64 = BAR_WIDTH;
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

/// A calendar record's whole spoken label: the hold, the meeting and the repeat, each said because
/// each is drawn as a shape or a glyph a screen reader cannot see.
pub(in crate::ui) fn spoken_event(
    label: &str,
    participation: ResponseStatus,
    has_conference: bool,
    repeats: bool,
) -> String {
    let label = super::meeting::spoken(spoken_with_hold(label, participation), has_conference);
    if repeats {
        format!("{label}, {}", l10n::a11y_event_repeats())
    } else {
        label
    }
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

/// The diagonal hatching down a hold's leading edge, in place of the bar; the part of the
/// treatment that survives being looked at quickly, when a dashed border at a zoomed-out hour
/// height does not.
fn hatch(context: &cairo::Context, rect: Rect, color: Rgb) {
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

/// The hold's treatment for the **composed** surface: the month grid's chips, which are GTK
/// buttons rather than draw calls.
///
/// A chip is a few points tall, so the hatch is what survives and the dashes are the decoration
/// that rides beside it; the leading gutter is a `repeating-linear-gradient` sized to [`GUTTER`],
/// which is the only way to clip a hatch to a strip in GTK CSS.
pub(in crate::ui) fn hold_css(background: &str, border: &str) -> String {
    format!(
        "background-color: {background}; \
         background-image: repeating-linear-gradient(135deg, {border} 0px, {border} \
         {EDGE_THICKNESS}px, transparent {EDGE_THICKNESS}px, transparent {HATCH_STEP}px); \
         background-size: {GUTTER}px 100%; background-repeat: no-repeat; \
         background-position: left top; border: {EDGE_THICKNESS}px dashed {border};"
    )
}

/// How an event wears its calendar's colour in one theme: a tint to fill it, the colour itself down
/// its leading edge, and a title that clears WCAG AA on the tint (`docs/calendar.md` §1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct EventColors {
    pub(in crate::ui) fill: Rgb,
    pub(in crate::ui) accent: Rgb,
    pub(in crate::ui) text: Rgb,
}

/// The colour an event draws in when no calendar owns it: Allodia Blue. The invitation card's
/// preview draws every block in it, because that preview carries no calendar list at all.
pub(in crate::ui) const NEUTRAL_HEX: &str = "#16598D";

thread_local! {
    static EVENT_COLORS: RefCell<HashMap<(String, bool), EventColors>> =
        RefCell::new(HashMap::new());
}

/// The event colours the calendar colour `hex` resolves to in a theme, asked of the core once per
/// colour and theme rather than for every block drawn.
pub(in crate::ui) fn event_colors(hex: &str, dark: bool) -> EventColors {
    EVENT_COLORS.with(|cache| {
        *cache
            .borrow_mut()
            .entry((hex.to_owned(), dark))
            .or_insert_with(|| {
                let swatch = mailcal_bindings::calendar_event_swatch(hex.to_owned(), dark);
                EventColors {
                    fill: Rgb::from_hex(&swatch.fill),
                    accent: Rgb::from_hex(&swatch.accent),
                    text: Rgb::from_hex(&swatch.text),
                }
            })
    })
}

/// An event's block: the tint, then the calendar's colour down the leading edge; for a hold, a
/// fainter tint, the edge hatched and the border dashed. The caller draws the label inside.
pub(in crate::ui) fn event_block(
    context: &cairo::Context,
    rect: Rect,
    colors: EventColors,
    awaiting: bool,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let corner = CORNER.min(rect.width / 2.0).min(rect.height / 2.0);
    let _ = context.save();
    rounded(context, rect, corner);
    let fill = colors.fill;
    context.set_source_rgba(fill.red, fill.green, fill.blue, hold_alpha(awaiting));
    let _ = context.fill_preserve();
    context.clip();
    if awaiting {
        hatch(context, rect, colors.accent);
    } else {
        set_source(context, colors.accent);
        context.rectangle(rect.x, rect.y, BAR_WIDTH.min(rect.width), rect.height);
        let _ = context.fill();
    }
    let _ = context.restore();
    // Inset by half the stroke, which straddles its path: on the boundary its outer half would be
    // clipped away and read as a rendering artefact rather than as a deliberate edge.
    let half = EDGE_THICKNESS / 2.0;
    if !awaiting || rect.width <= half * 2.0 || rect.height <= half * 2.0 {
        return;
    }
    let _ = context.save();
    set_source(context, colors.accent);
    context.set_dash(&DASH, 0.0);
    context.set_line_width(EDGE_THICKNESS);
    let inner = Rect {
        x: rect.x + half,
        y: rect.y + half,
        width: rect.width - half * 2.0,
        height: rect.height - half * 2.0,
    };
    rounded(context, inner, (corner - half).max(0.0));
    let _ = context.stroke();
    let _ = context.restore();
}

fn rounded(context: &cairo::Context, rect: Rect, radius: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};

    let Rect {
        x,
        y,
        width,
        height,
    } = rect;
    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    context.arc(x + radius, y + height - radius, radius, FRAC_PI_2, PI);
    context.arc(x + radius, y + radius, radius, PI, PI + FRAC_PI_2);
    context.close_path();
}

/// The composed surface's commitment: the tint, and the calendar's colour as a bar down the leading
/// edge, drawn the way [`hold_css`] draws its hatch so the two chips line up.
pub(in crate::ui) fn commitment_css(fill: &str, accent: &str) -> String {
    format!(
        "background-color: {fill}; \
         background-image: linear-gradient({accent}, {accent}); \
         background-size: {GUTTER}px 100%; background-repeat: no-repeat; \
         background-position: left top; border: {EDGE_THICKNESS}px solid transparent;"
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct Rgb {
    pub(in crate::ui) red: f64,
    pub(in crate::ui) green: f64,
    pub(in crate::ui) blue: f64,
}

impl Rgb {
    pub(in crate::ui) const fn new(red: f64, green: f64, blue: f64) -> Self {
        Self { red, green, blue }
    }

    /// The colour as `#rrggbb`, for a surface styled by CSS.
    pub(in crate::ui) fn hex(self) -> String {
        // Each component is in 0..=1, so the byte is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            byte(self.red),
            byte(self.green),
            byte(self.blue)
        )
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

    use super::{
        HOLD_FILL_ALPHA, Rect, Rgb, commitment_css, event_block, event_colors, hold_alpha,
        hold_css, is_awaiting, spoken_event, spoken_with_hold,
    };

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
        assert!(css.contains("background-size: 3px 100%"));
        assert!(css.contains("background-repeat: no-repeat"));
        assert!(css.contains("border: 1px dashed #0f3e63"));
    }

    #[test]
    fn a_composed_commitment_wears_a_solid_bar_where_a_hold_hatches_one() {
        let css = commitment_css("#dbe6f0", "#16598d");
        assert!(css.contains("background-color: #dbe6f0"));
        assert!(css.contains("linear-gradient(#16598d, #16598d)"));
        assert!(css.contains("background-size: 3px 100%"));
        assert!(!css.contains("dashed"));
    }

    #[test]
    fn the_event_colours_are_the_cores_and_are_asked_once_per_colour_and_theme() {
        let swatch = mailcal_bindings::calendar_event_swatch("#2f6fa8".to_owned(), false);
        let colors = event_colors("#2f6fa8", false);
        assert_eq!(colors.fill, Rgb::from_hex(&swatch.fill));
        assert_eq!(colors.accent, Rgb::from_hex(&swatch.accent));
        assert_eq!(colors.text, Rgb::from_hex(&swatch.text));
        assert_eq!(event_colors("#2f6fa8", false), colors);
        assert_ne!(event_colors("#2f6fa8", true), colors);
    }

    /// The block drawn onto a 40×40 surface, read back as `(red, green, blue)` bytes per pixel.
    fn drawn(awaiting: bool) -> impl Fn(usize, usize) -> (u8, u8, u8) {
        use gtk::cairo;
        let mut surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 40, 40).unwrap();
        {
            let context = cairo::Context::new(&surface).unwrap();
            context.set_source_rgb(1.0, 1.0, 1.0);
            context.paint().unwrap();
            let rect = Rect {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 40.0,
            };
            event_block(&context, rect, event_colors("#2f6fa8", false), awaiting);
        }
        let stride = usize::try_from(surface.stride()).unwrap();
        let data = surface.data().unwrap().to_vec();
        // Rgb24 is native-endian xRGB: blue, green, red, unused on a little-endian host.
        move |x, y| {
            let at = y * stride + x * 4;
            (data[at + 2], data[at + 1], data[at])
        }
    }

    fn bytes(color: Rgb) -> (u8, u8, u8) {
        let hex = color.hex();
        let byte = |range| u8::from_str_radix(&hex[range], 16).unwrap();
        (byte(1..3), byte(3..5), byte(5..7))
    }

    #[test]
    fn a_commitment_wears_a_solid_bar_and_a_hold_a_hatch_and_a_dashed_edge() {
        let colors = event_colors("#2f6fa8", false);
        let commitment = drawn(false);
        // The bar runs the whole leading edge; inside, the tint; and no edge of its own.
        for y in 5..35 {
            assert_eq!(commitment(1, y), bytes(colors.accent), "bar at y {y}");
        }
        assert_eq!(commitment(20, 20), bytes(colors.fill));
        assert_eq!(commitment(20, 0), bytes(colors.fill));

        let hold = drawn(true);
        let tint = hold(20, 20);
        let accent = bytes(colors.accent);
        // Which of the two a pixel is nearer, so antialiasing at a stripe's edge decides nothing.
        let nearer_accent = |pixel: (u8, u8, u8)| distance(pixel, accent) < distance(pixel, tint);
        // The tint is fainter than a commitment's.
        assert_ne!(tint, bytes(colors.fill));
        // The leading edge is hatched: stripes of the colour with the tint between them.
        let edge = (5..35)
            .map(|y| nearer_accent(hold(1, y)))
            .collect::<Vec<_>>();
        assert!(edge.contains(&true) && edge.contains(&false), "{edge:?}");
        // The top edge is dashed: on, then off.
        let top = (6..34)
            .map(|x| nearer_accent(hold(x, 0)))
            .collect::<Vec<_>>();
        assert!(top.contains(&true) && top.contains(&false), "{top:?}");
    }

    fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
        let channel = |x: u8, y: u8| u32::from(x.abs_diff(y));
        channel(a.0, b.0) + channel(a.1, b.1) + channel(a.2, b.2)
    }

    #[test]
    fn the_spoken_label_says_the_hold_the_meeting_and_the_repeat_in_that_order() {
        let label = spoken_event("Standup", ResponseStatus::NeedsAction, true, true);
        assert_eq!(
            label,
            format!(
                "Standup, {}, {}, {}",
                crate::l10n::a11y_invitation_awaiting_response(),
                crate::l10n::event_video_meeting(),
                crate::l10n::a11y_event_repeats()
            )
        );
        assert_eq!(
            spoken_event("Standup", ResponseStatus::Accepted, false, false),
            "Standup"
        );
    }
}
