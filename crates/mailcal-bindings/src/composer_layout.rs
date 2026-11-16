//! The composer's two layout rules, decided once for every desktop client
//! (`docs/composer-layout.md`): which of its two scrolls a gesture moves, and how far the column
//! scrolls to keep the caret in view.
//!
//! Every length is in the host's own layout unit (points on Apple, device-independent pixels on
//! Windows, logical pixels on GTK), which is what a CSS pixel in the editor's page is on each of
//! them.

/// The room kept between the caret and the edge of the column's visible part, so the line being
/// typed is not flush against the window's edge.
const CARET_MARGIN: f64 = 24.0;

/// Within this of its end, the column counts as at its end: an offset is a rounded length.
const END_SLACK: f64 = 0.5;

/// The scroll a step of a gesture moves (`docs/composer-layout.md`, rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ComposerScroller {
    /// The column, which carries the header away and brings it back.
    Column,
    /// The message, inside its own box under the toolbar.
    Message,
}

/// Which scroll a step of a gesture moves: the header goes first and comes back last.
///
/// `column_offset_px` is the column's scroll offset and `column_end_px` the furthest it scrolls,
/// which is where the message's box fills its visible part; `message_offset_px` is how far the
/// message is scrolled inside its box, from the page's `scroll` notice; `towards_end` is a step
/// down the message. Down, the column moves until the header has gone, then the message. Up, the
/// message moves until it is back at its top, then the column. A length that is not a finite
/// number counts as nothing.
#[must_use]
#[uniffi::export]
pub fn composer_scroller(
    column_offset_px: f64,
    column_end_px: f64,
    message_offset_px: f64,
    towards_end: bool,
) -> ComposerScroller {
    if towards_end {
        if length(column_offset_px) + END_SLACK < length(column_end_px) {
            ComposerScroller::Column
        } else {
            ComposerScroller::Message
        }
    } else if length(message_offset_px) > 0.0 {
        ComposerScroller::Message
    } else {
        ComposerScroller::Column
    }
}

/// Where to scroll the column so the caret is in view, or `None` when it already is
/// (`docs/composer-layout.md`, rule 6).
///
/// `offset_px` is the column's scroll offset and `visible_px` the height of its visible part;
/// `caret_top_px` and `caret_bottom_px` are the caret's edges in the column's own coordinates,
/// which is the page's coordinates plus where the editor starts in the column. The answer moves
/// the column as little as it can: a caret above the visible part is brought to its top, one below
/// it to its bottom, each with a margin to spare. A caret taller than the visible part is
/// brought to the top, where its first line is. A target past the column's end is the column's
/// own to clamp: the message's box then fills the visible part, and the page keeps its caret in
/// view inside it.
#[must_use]
#[uniffi::export]
pub fn composer_caret_scroll(
    offset_px: f64,
    visible_px: f64,
    caret_top_px: f64,
    caret_bottom_px: f64,
) -> Option<f64> {
    let values = [offset_px, visible_px, caret_top_px, caret_bottom_px];
    if values.iter().any(|value| !value.is_finite()) || visible_px <= 0.0 {
        return None;
    }
    let top = caret_top_px - CARET_MARGIN;
    let bottom = caret_bottom_px + CARET_MARGIN;
    let target = if top < offset_px || bottom - top > visible_px {
        top
    } else if bottom > offset_px + visible_px {
        bottom - visible_px
    } else {
        return None;
    };
    Some(target.max(0.0))
}

fn length(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ComposerScroller, composer_caret_scroll, composer_scroller};

    #[test]
    fn down_the_header_goes_first_then_the_message() {
        assert_eq!(
            composer_scroller(0.0, 240.0, 0.0, true),
            ComposerScroller::Column
        );
        assert_eq!(
            composer_scroller(239.0, 240.0, 0.0, true),
            ComposerScroller::Column
        );
        assert_eq!(
            composer_scroller(239.8, 240.0, 0.0, true),
            ComposerScroller::Message
        );
        assert_eq!(
            composer_scroller(240.0, 240.0, 500.0, true),
            ComposerScroller::Message
        );
    }

    #[test]
    fn up_the_message_goes_back_to_its_top_before_the_header_returns() {
        assert_eq!(
            composer_scroller(240.0, 240.0, 300.0, false),
            ComposerScroller::Message
        );
        assert_eq!(
            composer_scroller(240.0, 240.0, 0.0, false),
            ComposerScroller::Column
        );
        assert_eq!(
            composer_scroller(100.0, 240.0, 0.0, false),
            ComposerScroller::Column
        );
    }

    #[test]
    fn a_column_with_nothing_to_carry_away_hands_every_step_to_the_message() {
        assert_eq!(
            composer_scroller(0.0, 0.0, 0.0, true),
            ComposerScroller::Message
        );
        assert_eq!(
            composer_scroller(f64::NAN, f64::NAN, f64::NAN, true),
            ComposerScroller::Message
        );
    }

    #[test]
    fn a_caret_already_in_view_moves_nothing() {
        assert_eq!(composer_caret_scroll(100.0, 400.0, 200.0, 218.0), None);
    }

    #[test]
    fn a_caret_typed_past_the_bottom_brings_the_column_down_by_as_little_as_it_can() {
        // Visible 100..500; the caret's line runs 490..508, plus the margin to 532.
        assert_eq!(
            composer_caret_scroll(100.0, 400.0, 490.0, 508.0),
            Some(132.0)
        );
    }

    #[test]
    fn a_caret_above_the_visible_part_is_brought_to_its_top() {
        assert_eq!(
            composer_caret_scroll(300.0, 400.0, 250.0, 268.0),
            Some(226.0)
        );
    }

    #[test]
    fn the_column_never_scrolls_above_its_start() {
        assert_eq!(composer_caret_scroll(30.0, 400.0, 10.0, 28.0), Some(0.0));
    }

    #[test]
    fn a_caret_taller_than_the_window_shows_its_first_line() {
        assert_eq!(composer_caret_scroll(0.0, 100.0, 300.0, 600.0), Some(276.0));
    }

    #[test]
    fn nothing_to_measure_against_moves_nothing() {
        assert_eq!(composer_caret_scroll(0.0, 0.0, 300.0, 318.0), None);
        assert_eq!(composer_caret_scroll(f64::NAN, 400.0, 300.0, 318.0), None);
    }
}
