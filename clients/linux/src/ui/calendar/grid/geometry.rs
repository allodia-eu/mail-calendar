//! Where each event sits, in the coordinates of the surface that draws it.
//!
//! The header (day names and the all-day banner) and the hours are two surfaces, so a hit belongs
//! to one of them and is measured from that surface's own top.

use super::{
    super::{model::EventIdentity, paint::Rect},
    scene::{BandPaint, EventPaint, GUTTER, GridScene, HEADING_HEIGHT, LANE_HEIGHT, pixels},
};
use crate::l10n;

#[derive(Clone, Debug)]
pub(super) struct Hit {
    pub(super) identity: Option<EventIdentity>,
    pub(super) spoken: String,
    pub(super) rect: Rect,
}

impl Hit {
    pub(super) fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.x
            && x < self.rect.x + self.rect.width
            && y >= self.rect.y
            && y < self.rect.y + self.rect.height
    }
}

pub(super) struct Geometry {
    pub(super) day_width: f64,
    /// Timed events, in the scrolled hours' coordinates.
    pub(super) hits: Vec<Hit>,
    /// All-day bars and the per-day overflow chips, in the pinned header's coordinates.
    pub(super) header_hits: Vec<Hit>,
}

impl Geometry {
    pub(super) fn new(scene: &GridScene, width: f64) -> Self {
        let day_width = ((width - GUTTER) / pixels(scene.days.len().max(1))).max(1.0);
        let hits = scene
            .events
            .iter()
            .map(|event| timed_hit(scene, event, day_width))
            .collect();
        let mut header_hits = scene
            .bands
            .iter()
            .map(|band| band_hit(band, day_width))
            .collect::<Vec<_>>();
        for (day, hidden) in scene.hidden_per_day.iter().copied().enumerate() {
            if hidden > 0 {
                header_hits.push(Hit {
                    identity: None,
                    spoken: l10n::calendar_all_day_expand(i64::from(hidden)),
                    rect: Rect {
                        x: GUTTER + pixels(day) * day_width + 1.0,
                        y: HEADING_HEIGHT
                            + f64::from(scene.banner_lanes.saturating_sub(1)) * LANE_HEIGHT
                            + 1.0,
                        width: (day_width - 2.0).max(1.0),
                        height: LANE_HEIGHT - 2.0,
                    },
                });
            }
        }
        Self {
            day_width,
            hits,
            header_hits,
        }
    }
}

fn timed_hit(scene: &GridScene, event: &EventPaint, day_width: f64) -> Hit {
    let lane_width = day_width / f64::from(event.columns);
    Hit {
        identity: Some(event.identity.clone()),
        spoken: event.spoken.clone(),
        rect: Rect {
            x: GUTTER + pixels(event.day) * day_width + f64::from(event.column) * lane_width + 1.0,
            y: f64::from(event.start_minutes) * scene.hour_height / 60.0,
            width: (lane_width - 2.0).max(1.0),
            height: (f64::from(event.end_minutes - event.start_minutes) * scene.hour_height / 60.0)
                .max(3.0),
        },
    }
}

fn band_hit(band: &BandPaint, day_width: f64) -> Hit {
    Hit {
        identity: Some(band.identity.clone()),
        spoken: band.spoken.clone(),
        rect: Rect {
            x: GUTTER + pixels(band.day) * day_width + 1.0,
            y: HEADING_HEIGHT + f64::from(band.lane) * LANE_HEIGHT + 1.0,
            width: (pixels(band.days) * day_width - 2.0).max(1.0),
            height: LANE_HEIGHT - 2.0,
        },
    }
}
