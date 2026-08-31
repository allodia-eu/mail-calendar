//! Drag-to-create interaction over the immutable grid scene.

use super::scene::{GUTTER, GridScene};
use crate::ui::calendar::drag::{CreateDrag, CreateSlot, DragMetrics};

impl GridScene {
    pub(super) const fn drag(&self) -> Option<CreateDrag> {
        self.drag
    }

    pub(super) fn begin_create(&mut self, x: f64, y: f64, width: f64) -> bool {
        if !self.is_materialized || self.create.is_none() {
            return false;
        }
        let geometry = self.geometry(width);
        if geometry.hits.iter().any(|hit| {
            hit.identity.is_some()
                && x >= hit.rect.x
                && x < hit.rect.x + hit.rect.width
                && y >= hit.rect.y
                && y < hit.rect.y + hit.rect.height
        }) {
            return false;
        }
        let Some((day, raw_minute)) = self.drag_metrics(width).point(x, y) else {
            return false;
        };
        let Some(date) = self.days.get(day).map(|value| value.date) else {
            return false;
        };
        self.drag = Some(CreateDrag::begin(date, day, raw_minute));
        true
    }

    pub(super) fn update_create(&mut self, y: f64, width: f64) {
        let Some(drag) = self.drag else {
            return;
        };
        if let Some(raw_minute) = self.drag_metrics(width).clamped_minute(y) {
            self.drag = Some(drag.moved_to(raw_minute));
        }
    }

    pub(super) fn finish_create(&mut self) -> Option<CreateSlot> {
        self.drag.take().map(CreateDrag::slot)
    }

    pub(super) fn cancel_create(&mut self) {
        self.drag = None;
    }

    fn drag_metrics(&self, width: f64) -> DragMetrics {
        DragMetrics {
            gutter: GUTTER,
            content_top: self.content_top(),
            day_width: self.geometry(width).day_width,
            hour_height: self.hour_height,
            days: self.days.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::GridScene;
    use crate::ui::calendar::{
        EventIdentity,
        grid::scene::{CreatePaint, DayPaint, EventPaint},
        paint::Rgb,
    };

    fn date() -> Date {
        Date::from_calendar_date(2026, Month::July, 22).unwrap()
    }

    fn scene() -> GridScene {
        let mut scene = GridScene::empty();
        scene.days = vec![DayPaint {
            date: date(),
            label: "Wed 22".to_owned(),
            is_today: false,
        }];
        scene.hour_height = 60.0;
        scene.is_materialized = true;
        scene.create = Some(CreatePaint {
            background: Rgb::new(0.2, 0.3, 0.4),
            border: Rgb::new(0.1, 0.2, 0.3),
        });
        scene
    }

    #[test]
    fn a_settled_grid_drag_names_the_date_and_snapped_range_it_drew() {
        let mut scene = scene();
        assert!(scene.begin_create(80.0, 52.0 + 10.0 * 60.0 + 5.0, 168.0));
        scene.update_create(52.0 + 11.0 * 60.0 + 28.0, 168.0);
        let slot = scene.finish_create().unwrap();
        assert_eq!(slot.date, date());
        assert_eq!(slot.start_minutes, 10 * 60);
        assert_eq!(slot.end_minutes, 11 * 60 + 30);
    }

    #[test]
    fn an_event_keeps_its_click_instead_of_starting_a_create_drag() {
        let mut scene = scene();
        scene.events.push(EventPaint {
            identity: EventIdentity {
                account: "account".to_owned(),
                key: "event".to_owned(),
                occurrence: String::new(),
            },
            title: "Planning".to_owned(),
            spoken: "Planning, 10:00–11:00, Work".to_owned(),
            day: 0,
            start_minutes: 10 * 60,
            end_minutes: 11 * 60,
            column: 0,
            columns: 1,
            background: Rgb::new(0.2, 0.3, 0.4),
            foreground: Rgb::new(1.0, 1.0, 1.0),
            border: Rgb::new(0.1, 0.2, 0.3),
            awaiting: false,
        });
        assert!(!scene.begin_create(80.0, 52.0 + 10.5 * 60.0, 168.0));
        assert!(scene.drag().is_none());
    }
}
