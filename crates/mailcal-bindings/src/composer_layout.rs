//! The composer's two layout rules, decided once for every client (`docs/composer-layout.md`):
//! how tall the editor is, and how far the column scrolls to keep the caret in view.
//!
//! Every length is in the host's own layout unit (points on Apple, device-independent pixels on
//! Windows, logical pixels on GTK and Android), which is what a CSS pixel in the editor's page is
//! on each of them.

/// The room kept between the caret and the edge of the column's visible part, so the line being
/// typed is not flush against the window's edge.
const CARET_MARGIN: f64 = 24.0;

/// The editor's height: what its content needs, or the room the column has left under the header
/// and the files, whichever is more (`docs/composer-layout.md`, rule 5). A length that is not a
/// finite, non-negative number counts as nothing.
#[must_use]
#[uniffi::export]
pub fn composer_editor_height(content_px: f64, room_px: f64) -> f64 {
    length(content_px).max(length(room_px))
}

/// Where to scroll the column so the caret is in view, or `None` when it already is
/// (`docs/composer-layout.md`, rule 6).
///
/// `offset_px` is the column's scroll offset and `visible_px` the height of its visible part;
/// `caret_top_px` and `caret_bottom_px` are the caret's edges in the column's own coordinates,
/// which is the page's coordinates plus where the editor starts in the column. The answer moves
/// the column as little as it can: a caret above the visible part is brought to its top, one below
/// it to its bottom, each with a margin to spare. A caret taller than the visible part is
/// brought to the top, where its first line is.
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
    use super::{composer_caret_scroll, composer_editor_height};

    #[test]
    fn a_short_message_fills_the_room_left_and_a_long_one_takes_what_it_needs() {
        assert!((composer_editor_height(120.0, 300.0) - 300.0).abs() < f64::EPSILON);
        assert!((composer_editor_height(900.0, 300.0) - 900.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_room_at_all_still_gives_the_editor_its_content() {
        // A header and a file list taller than the window leave nothing over.
        assert!((composer_editor_height(240.0, -80.0) - 240.0).abs() < f64::EPSILON);
        assert!((composer_editor_height(f64::NAN, f64::INFINITY)).abs() < f64::EPSILON);
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
