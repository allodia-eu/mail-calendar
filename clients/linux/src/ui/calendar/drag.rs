//! Pure drag-to-create geometry for the Linux time grid.

use time::Date;

const SNAP_MINUTES: u32 = 15;
const HOUR_MINUTES: u32 = 60;
const DAY_MINUTES: u32 = 24 * HOUR_MINUTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CreateSlot {
    pub(super) date: Date,
    pub(super) start_minutes: u32,
    pub(super) end_minutes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CreatePreview {
    pub(super) day: usize,
    pub(super) start_minutes: u32,
    pub(super) end_minutes: u32,
}

impl CreatePreview {
    pub(super) const fn minutes(self) -> u32 {
        self.end_minutes - self.start_minutes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CreateDrag {
    date: Date,
    day: usize,
    anchor_minute: u32,
    raw_anchor_minute: u32,
    minute: u32,
    raw_minute: u32,
}

impl CreateDrag {
    pub(super) fn begin(date: Date, day: usize, raw_minute: u32) -> Self {
        let raw_minute = raw_minute.min(DAY_MINUTES);
        let minute = snapped(raw_minute);
        Self {
            date,
            day,
            anchor_minute: minute,
            raw_anchor_minute: raw_minute,
            minute,
            raw_minute,
        }
    }

    pub(super) fn moved_to(mut self, raw_minute: u32) -> Self {
        self.raw_minute = raw_minute.min(DAY_MINUTES);
        self.minute = snapped(self.raw_minute);
        self
    }

    pub(super) fn preview(self) -> CreatePreview {
        self.preview_using(self.minute)
    }

    pub(super) fn live_preview(self) -> CreatePreview {
        self.preview_using(self.raw_minute)
    }

    pub(super) fn slot(self) -> CreateSlot {
        let preview = self.preview();
        CreateSlot {
            date: self.date,
            start_minutes: preview.start_minutes,
            end_minutes: preview.end_minutes,
        }
    }

    fn preview_using(self, pointer: u32) -> CreatePreview {
        let band =
            (self.raw_anchor_minute / HOUR_MINUTES * HOUR_MINUTES).min(DAY_MINUTES - HOUR_MINUTES);
        CreatePreview {
            day: self.day,
            start_minutes: band.min(pointer),
            end_minutes: (band + HOUR_MINUTES).max(pointer),
        }
    }
}

fn snapped(minute: u32) -> u32 {
    ((minute + SNAP_MINUTES / 2) / SNAP_MINUTES * SNAP_MINUTES).min(DAY_MINUTES)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DragMetrics {
    pub(super) gutter: f64,
    pub(super) content_top: f64,
    pub(super) day_width: f64,
    pub(super) hour_height: f64,
    pub(super) days: usize,
}

impl DragMetrics {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn point(self, x: f64, y: f64) -> Option<(usize, u32)> {
        if self.days == 0
            || !self.day_width.is_finite()
            || !self.hour_height.is_finite()
            || self.day_width <= 0.0
            || self.hour_height <= 0.0
            || x < self.gutter
            || y < self.content_top
        {
            return None;
        }
        let day = ((x - self.gutter) / self.day_width).floor() as usize;
        let raw_minute =
            ((y - self.content_top) / self.hour_height * f64::from(HOUR_MINUTES)).round();
        if day >= self.days || !(0.0..=f64::from(DAY_MINUTES)).contains(&raw_minute) {
            return None;
        }
        Some((day, raw_minute as u32))
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn clamped_minute(self, y: f64) -> Option<u32> {
        if !self.hour_height.is_finite() || self.hour_height <= 0.0 || !y.is_finite() {
            return None;
        }
        let raw = ((y - self.content_top) / self.hour_height * f64::from(HOUR_MINUTES))
            .round()
            .clamp(0.0, f64::from(DAY_MINUTES));
        Some(raw as u32)
    }
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::{CreateDrag, CreatePreview, DragMetrics};

    fn date() -> Date {
        Date::from_calendar_date(2026, Month::July, 22).unwrap()
    }

    fn drag(raw_minute: u32) -> CreateDrag {
        CreateDrag::begin(date(), 2, raw_minute)
    }

    #[test]
    fn a_press_fills_the_hour_band_the_pointer_is_inside() {
        let create = drag(16 * 60 + 53);
        assert_eq!(
            create.preview(),
            CreatePreview {
                day: 2,
                start_minutes: 16 * 60,
                end_minutes: 17 * 60,
            }
        );
        assert_eq!(create.slot().date, date());
    }

    #[test]
    fn dragging_down_and_up_takes_the_union_with_that_hour() {
        assert_eq!(
            drag(10 * 60 + 5).moved_to(11 * 60 + 28).preview(),
            CreatePreview {
                day: 2,
                start_minutes: 10 * 60,
                end_minutes: 11 * 60 + 30,
            }
        );
        assert_eq!(
            drag(10 * 60 + 5).moved_to(8 * 60 + 28).preview(),
            CreatePreview {
                day: 2,
                start_minutes: 8 * 60 + 30,
                end_minutes: 11 * 60,
            }
        );
    }

    #[test]
    fn the_block_glides_while_the_readout_stays_on_the_snap() {
        let create = drag(10 * 60).moved_to(11 * 60 + 28);
        assert_eq!(create.preview().end_minutes, 11 * 60 + 30);
        assert_eq!(create.preview().minutes(), 90);
        assert_eq!(create.live_preview().end_minutes, 11 * 60 + 28);
    }

    #[test]
    fn a_create_stays_inside_its_day() {
        assert_eq!(drag(30).moved_to(0).preview().start_minutes, 0);
        assert_eq!(
            drag(23 * 60 + 30).moved_to(24 * 60).preview().end_minutes,
            24 * 60
        );
    }

    #[test]
    fn points_invert_the_drawn_grid_and_reject_its_chrome() {
        let metrics = DragMetrics {
            gutter: 68.0,
            content_top: 52.0,
            day_width: 100.0,
            hour_height: 60.0,
            days: 3,
        };
        assert_eq!(metrics.point(268.0, 52.0 + 9.25 * 60.0), Some((2, 555)));
        assert_eq!(metrics.point(67.0, 600.0), None);
        assert_eq!(metrics.point(100.0, 51.0), None);
        assert_eq!(metrics.point(368.0, 600.0), None);
        assert_eq!(metrics.point(100.0, 52.0 + 24.0 * 60.0 + 1.0), None);
    }
}
