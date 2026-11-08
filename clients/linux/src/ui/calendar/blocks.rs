//! How much of an event block's label the grid draws at a zoom, decided without a widget so a plain
//! test can drive it (`docs/calendar.md` §4, "An event block's label").
//!
//! The twin of Apple's `CalendarBlocks.swift`, number for number, and of Android's
//! `CalendarBlockLabels.kt` in its rules: a block draws a title only where a whole line fits, wraps
//! it by word over every whole line it has room for less the one its time takes, and keeps its
//! corner glyphs only while the title keeps room of its own.

/// The bar of the calendar's colour down an event's leading edge, and the width of a hold's hatch
/// that replaces it.
pub(in crate::ui) const BAR_WIDTH: f64 = 3.0;

/// The corner radius of a block, a bar and a chip.
pub(in crate::ui) const CORNER: f64 = 4.0;

/// Between a block's edge and the next block beside or below it.
pub(super) const GAP: f64 = 1.0;

/// The room between the bar and the label, and between the label and the trailing edge.
pub(super) const LABEL_PADDING: f64 = 4.0;

/// The narrowest a title may be squeezed to make room for the corner glyphs: a glyph is worth less
/// than the name of the event.
pub(super) const MINIMUM_TITLE_WIDTH: f64 = 36.0;

/// Between two corner glyphs, and between the glyphs and the title.
pub(super) const GLYPH_GAP: f64 = 3.0;

/// The step a title's wrapping width is rounded down to, so a resize finds the layout it made a
/// frame ago instead of shaping the text again (`docs/calendar.md` §7).
pub(super) const LABEL_STEP: f64 = 4.0;

/// The vertical inset of a block's label: a short block keeps every pixel it has.
pub(super) fn inset(minutes: u32) -> f64 {
    if minutes < 30 { GAP } else { GAP + 1.0 }
}

/// The vertical space a `minutes`-long block leaves for its label at this zoom.
pub(super) fn label_space(minutes: u32, hour_height: f64) -> f64 {
    hour_height * f64::from(minutes) / 60.0 - inset(minutes) * 2.0
}

/// The line pitch a `minutes`-long block's label is counted in.
pub(super) fn line_height(minutes: u32) -> f64 {
    if minutes < 30 { 11.0 } else { 14.0 }
}

/// The type size that goes with it.
pub(super) fn font_size(minutes: u32) -> f64 {
    if minutes < 30 { 9.0 } else { 11.0 }
}

/// Whether a block is tall enough to hold its title **at this zoom**. A block that is not stays a
/// coloured block and keeps its spoken label, rather than drawing a title cut through the middle.
pub(super) fn shows_label(minutes: u32, hour_height: f64) -> bool {
    label_space(minutes, hour_height) >= line_height(minutes)
}

/// Whether a block earns its time beneath the title: only once there is room for two lines.
pub(super) fn shows_time(minutes: u32, hour_height: f64) -> bool {
    label_space(minutes, hour_height) >= line_height(minutes) * 2.0
}

/// How many lines the title may wrap over: every whole line the block has room for, less the one
/// its time takes when it draws one. Zero where there is no room for a label.
pub(super) fn title_lines(minutes: u32, hour_height: f64) -> u32 {
    if !shows_label(minutes, hour_height) {
        return 0;
    }
    let whole = (label_space(minutes, hour_height) / line_height(minutes)).floor();
    let time = if shows_time(minutes, hour_height) {
        1.0
    } else {
        0.0
    };
    // Bounded by a day's height over the smallest pitch, so far inside `u32`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lines = (whole - time).max(1.0) as u32;
    lines
}

/// The width a title is wrapped against: the room there is, rounded down to [`LABEL_STEP`].
pub(super) fn wrap_width(available: f64) -> f64 {
    (available.max(0.0) / LABEL_STEP).floor() * LABEL_STEP
}

/// The cache key for a wrapping width on [`LABEL_STEP`]'s grid.
pub(super) fn wrap_key(width: f64) -> i32 {
    // A block is never wider than the screen, so this is far inside `i32`.
    #[allow(clippy::cast_possible_truncation)]
    let key = (width / LABEL_STEP).floor() as i32;
    key
}

/// Whether the title wraps by word: only where its widest word fits a line. A word wider than the
/// block would be broken across two, which reads as a broken grid, so the title then runs on as one
/// line and is cut at the block's edge.
pub(super) fn wraps(widest_word: f64, wrap_width: f64) -> bool {
    widest_word <= wrap_width
}

/// The room the title keeps beside the corner glyphs, or `None` where drawing them would leave the
/// title less than [`MINIMUM_TITLE_WIDTH`]: then the glyphs give way and the title has it all.
pub(super) fn beside_glyphs(width: f64, glyphs_width: f64) -> Option<f64> {
    if glyphs_width <= 0.0 {
        return None;
    }
    let room = width - glyphs_width - GLYPH_GAP;
    (room >= MINIMUM_TITLE_WIDTH).then_some(room)
}

/// The width the meeting and repeat glyphs take side by side, each `size` square.
pub(super) fn glyphs_width(count: u32, size: f64) -> f64 {
    if count == 0 {
        return 0.0;
    }
    f64::from(count) * size + f64::from(count - 1) * GLYPH_GAP
}

/// Whether the time line draws the range (`09:00–12:00`) or, where the column is too narrow for
/// it, the start alone.
pub(super) fn shows_range(range_width: f64, width: f64) -> bool {
    range_width <= width
}

#[cfg(test)]
mod tests {
    use super::{
        beside_glyphs, glyphs_width, shows_label, shows_range, shows_time, title_lines, wrap_key,
        wrap_width, wraps,
    };

    #[test]
    fn a_block_too_short_for_a_whole_line_draws_no_title() {
        // A quarter of an hour at twenty pixels an hour is five pixels tall.
        assert!(!shows_label(15, 20.0));
        assert_eq!(title_lines(15, 20.0), 0);
        // The same quarter at sixty an hour has room for one line, and no time beneath it.
        assert!(shows_label(15, 60.0));
        assert!(!shows_time(15, 60.0));
        assert_eq!(title_lines(15, 60.0), 1);
    }

    #[test]
    fn a_long_block_wraps_its_title_over_every_whole_line_less_the_time() {
        // Three hours at sixty an hour: 176 px of room over a 14 px pitch is 12 whole lines, one of
        // which the time takes.
        assert!(shows_time(180, 60.0));
        assert_eq!(title_lines(180, 60.0), 11);
        // An hour: 56 px is four lines, three for the title.
        assert_eq!(title_lines(60, 60.0), 3);
        // Half an hour at sixty: 26 px holds one line, so no time and one title line.
        assert!(!shows_time(30, 60.0));
        assert_eq!(title_lines(30, 60.0), 1);
    }

    #[test]
    fn a_title_never_breaks_a_word() {
        assert!(wraps(40.0, 40.0));
        assert!(!wraps(40.5, 40.0));
    }

    #[test]
    fn the_wrapping_width_is_held_on_a_step_so_a_resize_reuses_a_layout() {
        assert!((wrap_width(51.9) - 48.0).abs() < f64::EPSILON);
        assert!((wrap_width(48.0) - 48.0).abs() < f64::EPSILON);
        assert!((wrap_width(-3.0)).abs() < f64::EPSILON);
        assert_eq!(wrap_key(wrap_width(49.0)), wrap_key(wrap_width(51.0)));
        assert_ne!(wrap_key(wrap_width(51.0)), wrap_key(wrap_width(52.0)));
    }

    #[test]
    fn the_corner_glyphs_draw_only_while_the_title_keeps_thirty_six_points() {
        let two = glyphs_width(2, 11.0);
        assert!((two - 25.0).abs() < f64::EPSILON);
        assert!(beside_glyphs(64.0, two).is_some_and(|room| (room - 36.0).abs() < f64::EPSILON));
        assert_eq!(beside_glyphs(63.0, two), None);
        assert_eq!(beside_glyphs(200.0, glyphs_width(0, 11.0)), None);
    }

    #[test]
    fn the_time_is_a_range_where_it_fits_and_the_start_where_it_does_not() {
        assert!(shows_range(60.0, 60.0));
        assert!(!shows_range(61.0, 60.0));
    }
}
