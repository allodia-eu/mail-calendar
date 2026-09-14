//! Where the day sits vertically: the opening framing, and the reader's place across a resize.

use std::cell::{Cell, RefCell};

use adw::prelude::*;

use super::{super::date::now_in, scene::GridScene};

const DAY_MINUTES: f64 = 24.0 * 60.0;

/// Half a pixel: the grain below which two viewport heights are the same viewport.
const SAME: f64 = 0.5;

/// The four numbers every offset in this file is arithmetic on.
#[derive(Clone, Copy, Debug)]
struct Frame {
    content_top: f64,
    hour_height: f64,
    viewport_height: f64,
    content_height: f64,
}

impl Frame {
    /// The frame the grid is laid out in, or `None` while that is not yet a real question.
    ///
    /// The grid asks for its height from an idle, so for a pass or two after the viewport changes
    /// the scrolled window is still measuring a day that belongs to the previous one. Both an
    /// offset computed against that and a minute read back out of it are wrong, and wrong by
    /// however much the window moved, which is why `caught_up` is checked before either.
    fn of(adjustment: &gtk::Adjustment, scene: &GridScene) -> Option<Self> {
        let frame = Self {
            content_top: scene.content_top(),
            hour_height: scene.hour_height,
            viewport_height: adjustment.page_size(),
            content_height: adjustment.upper(),
        };
        (frame.is_real() && frame.caught_up(scene)).then_some(frame)
    }

    /// Whether these numbers describe a day that has been laid out at all.
    fn is_real(&self) -> bool {
        [
            self.content_top,
            self.hour_height,
            self.viewport_height,
            self.content_height,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.hour_height > 0.0
            && self.viewport_height > 0.0
            && self.content_height > self.viewport_height
    }

    /// Whether the scrolled window is measuring the day the scene describes.
    fn caught_up(&self, scene: &GridScene) -> bool {
        (self.content_height - scene.height()).abs() < 1.5
    }

    /// The offset that puts `minutes` at the middle of the viewport, clamped to the day's edges.
    fn centred_on(&self, minutes: f64) -> f64 {
        let line = self.content_top + minutes.clamp(0.0, DAY_MINUTES) * self.hour_height / 60.0;
        (line - self.viewport_height / 2.0).clamp(0.0, self.content_height - self.viewport_height)
    }

    /// The minute at the middle of the viewport, for an offset of `value`.
    fn centre_minutes(&self, value: f64) -> f64 {
        ((value + self.viewport_height / 2.0 - self.content_top) * 60.0 / self.hour_height)
            .clamp(0.0, DAY_MINUTES)
    }
}

/// The grid's vertical position: the framing it still owes, and where the reader is.
///
/// The position is kept in **minutes**, not pixels, because an hour is as tall as the viewport
/// says: resize the window and every pixel offset in the grid means a different time, while the
/// time the reader was looking at means the same thing it did. The minute is the one at the
/// **middle** of the viewport, which degrades correctly at both ends of the day, since the offset
/// that restores a middle minute clamps back to the edge a grid parked there was already at.
#[derive(Debug, Default)]
pub(super) struct Framing {
    /// Set while the grid still owes its opening framing on the current hour.
    pending: Cell<bool>,
    minutes: Cell<f64>,
    /// The viewport the minute was measured against.
    viewport_height: Cell<f64>,
}

impl Framing {
    /// Asks for the opening framing. It is a state rather than a moment: the grid keeps owing it
    /// until a viewport real enough to frame against arrives.
    pub(super) fn open(&self) {
        self.pending.set(true);
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending.get()
    }

    /// Whether [`Framing::settle`] has anything to do, so a render that moved nothing costs no
    /// idle.
    pub(super) fn owes(&self, adjustment: &gtk::Adjustment) -> bool {
        self.is_pending() || self.viewport_changed(adjustment)
    }

    /// Records where the reader is.
    ///
    /// An offset that moves while the viewport is not the one this was measured against is not the
    /// reader moving: it is the scrolled window clamping the old offset into a range it computed
    /// from a content height that has not caught up. Following that stores the accident instead of
    /// the position, and the accident is `0` whenever the window grew past the old day's height,
    /// which is how growing a window used to throw the calendar back to midnight.
    pub(super) fn follow(&self, adjustment: &gtk::Adjustment, scene: &RefCell<GridScene>) {
        if self.viewport_changed(adjustment) {
            return;
        }
        let scene = scene.borrow();
        if let Some(frame) = Frame::of(adjustment, &scene) {
            self.minutes.set(frame.centre_minutes(adjustment.value()));
        }
    }

    /// Puts the grid where it belongs, now that the geometry may have caught up with the scene.
    ///
    /// Two jobs share this moment because both wait on the same thing, a viewport that is real:
    /// the opening framing on the current hour, and, once that is done, the reader's own place
    /// after the viewport moved under it.
    pub(super) fn settle(&self, adjustment: &gtk::Adjustment, scene: &RefCell<GridScene>) {
        if self.is_pending() {
            let minutes = {
                let scene = scene.borrow();
                let Some((_, minutes)) = now_in(&scene.timezone) else {
                    return;
                };
                f64::from(minutes)
            };
            self.seat_at(adjustment, scene, minutes);
            return;
        }
        if self.viewport_changed(adjustment) {
            self.seat_at(adjustment, scene, self.minutes.get());
        }
    }

    /// Centres the grid on `minutes`, and answers whether the geometry let it.
    ///
    /// ⚠️ The scene is released before the offset is written. Setting it notifies the adjustment's
    /// listeners synchronously, and one of them takes the scene mutably; holding a borrow across
    /// the write panics.
    pub(super) fn seat_at(
        &self,
        adjustment: &gtk::Adjustment,
        scene: &RefCell<GridScene>,
        minutes: f64,
    ) -> bool {
        let seat = {
            let scene = scene.borrow();
            Frame::of(adjustment, &scene)
                .map(|frame| (frame.viewport_height, frame.centred_on(minutes)))
        };
        let Some((viewport_height, value)) = seat else {
            return false;
        };
        self.pending.set(false);
        self.viewport_height.set(viewport_height);
        adjustment.set_value(value);
        // The adjustment clamps, so the minute worth keeping is the one the grid is really at.
        self.follow(adjustment, scene);
        true
    }

    fn viewport_changed(&self, adjustment: &gtk::Adjustment) -> bool {
        (adjustment.page_size() - self.viewport_height.get()).abs() >= SAME
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    fn frame(content_top: f64, hour_height: f64, viewport_height: f64) -> Frame {
        Frame {
            content_top,
            hour_height,
            viewport_height,
            content_height: content_top + 24.0 * hour_height,
        }
    }

    #[test]
    fn current_time_is_placed_at_the_viewport_midpoint() {
        let viewport = 480.0;
        let line = 52.0 + 12.0 * 60.0;
        let value = frame(52.0, 60.0, viewport).centred_on(12.0 * 60.0);
        assert!((line - value - viewport / 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_day_edges_clamp_without_exposing_empty_space() {
        assert!((frame(52.0, 60.0, 480.0).centred_on(30.0)).abs() < f64::EPSILON);
        assert!(
            (frame(52.0, 60.0, 480.0).centred_on(23.0 * 60.0 + 30.0)
                - (52.0 + 24.0 * 60.0 - 480.0))
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn framing_waits_for_real_layout_metrics() {
        assert!(frame(52.0, 60.0, 480.0).is_real());
        assert!(!frame(52.0, 60.0, 0.0).is_real());
        assert!(!frame(52.0, 0.0, 480.0).is_real());
        let mut as_short_as_its_viewport = frame(52.0, 60.0, 480.0);
        as_short_as_its_viewport.content_height = 480.0;
        assert!(!as_short_as_its_viewport.is_real());
    }

    #[test]
    fn a_minute_survives_the_round_trip_through_an_offset() {
        let frame = frame(78.0, 62.5, 750.0);
        let minutes = 9.0 * 60.0 + 25.0;
        assert!((frame.centre_minutes(frame.centred_on(minutes)) - minutes).abs() < 0.001);
    }

    /// The property the whole anchor exists for: the hour on screen is the hour that stays.
    ///
    /// A taller window means a taller hour, so the offset that showed 09:25 in the middle is a
    /// different number afterwards; keeping the number is what moved the reader three hours.
    #[test]
    fn a_taller_window_keeps_the_minute_rather_than_the_offset() {
        let before = frame(78.0, 750.0 / 12.0, 750.0);
        let after = frame(78.0, 1500.0 / 12.0, 1500.0);
        let minutes = 9.0 * 60.0 + 25.0;
        let offset = before.centred_on(minutes);

        assert!((after.centre_minutes(after.centred_on(minutes)) - minutes).abs() < 0.001);
        assert!(
            (after.centre_minutes(offset) - minutes).abs() > 60.0,
            "the offset alone should not have survived, or this test proves nothing"
        );
    }

    /// Every offset this produces is inside the content, which is the contract's own wording.
    #[test]
    fn a_restored_offset_is_always_inside_the_content() {
        for viewport in [120.0, 480.0, 750.0, 1500.0, 2400.0] {
            let frame = frame(130.0, viewport / 12.0, viewport);
            for minutes in [0.0, 1.0, 600.0, 1439.0, 1440.0] {
                let value = frame.centred_on(minutes);
                assert!(value >= 0.0);
                assert!(value <= frame.content_height - frame.viewport_height);
            }
        }
    }
}
