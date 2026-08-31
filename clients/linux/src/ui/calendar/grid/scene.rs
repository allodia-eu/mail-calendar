//! Immutable paint scene: everything a Cairo frame needs, derived once from the core page.

use super::super::{
    date::{clock, date_heading, parse_date, today_in},
    drag::CreateDrag,
    model::{CalendarModel, EventIdentity},
    paint::{self, Rect, Rgb},
};
use crate::l10n;

pub(super) const GUTTER: f64 = 68.0;
pub(super) const HEADING_HEIGHT: f64 = 52.0;
pub(super) const LANE_HEIGHT: f64 = 26.0;
const COLLAPSED_LANES: u32 = 3;
const VISIBLE_COLLAPSED_LANES: u32 = COLLAPSED_LANES - 1;

#[derive(Clone, Debug)]
pub(super) struct DayPaint {
    pub(super) date: time::Date,
    pub(super) label: String,
    pub(super) is_today: bool,
}

#[derive(Clone, Debug)]
pub(super) struct EventPaint {
    pub(super) identity: EventIdentity,
    pub(super) title: String,
    pub(super) spoken: String,
    pub(super) day: usize,
    pub(super) start_minutes: u32,
    pub(super) end_minutes: u32,
    pub(super) column: u32,
    pub(super) columns: u32,
    pub(super) background: Rgb,
    pub(super) foreground: Rgb,
    pub(super) border: Rgb,
    /// An invitation this account has not answered: draw it as a hold rather than a commitment.
    pub(super) awaiting: bool,
}

#[derive(Clone, Debug)]
pub(super) struct BandPaint {
    pub(super) identity: EventIdentity,
    pub(super) title: String,
    pub(super) spoken: String,
    pub(super) day: usize,
    pub(super) days: usize,
    pub(super) lane: u32,
    pub(super) background: Rgb,
    pub(super) foreground: Rgb,
    /// An invitation this account has not answered: draw it as a hold rather than a commitment.
    pub(super) awaiting: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CreatePaint {
    pub(super) background: Rgb,
    pub(super) border: Rgb,
}

#[derive(Clone, Debug)]
pub(super) struct GridScene {
    pub(super) week_number: u8,
    pub(super) days: Vec<DayPaint>,
    pub(super) events: Vec<EventPaint>,
    pub(super) bands: Vec<BandPaint>,
    pub(super) hidden_per_day: Vec<u32>,
    pub(super) banner_lanes: u32,
    pub(super) hour_height: f64,
    pub(super) visible_hours: u8,
    pub(super) use_24_hour: bool,
    pub(super) is_materialized: bool,
    pub(super) dark: bool,
    pub(super) timezone: String,
    pub(super) create: Option<CreatePaint>,
    pub(super) viewport_top: f64,
    pub(super) viewport_height: f64,
    pub(super) drag: Option<CreateDrag>,
}

impl GridScene {
    pub(super) fn empty() -> Self {
        Self {
            week_number: 0,
            days: Vec::new(),
            events: Vec::new(),
            bands: Vec::new(),
            hidden_per_day: Vec::new(),
            banner_lanes: 0,
            hour_height: 1.0,
            visible_hours: 12,
            use_24_hour: true,
            is_materialized: false,
            dark: false,
            timezone: String::new(),
            create: None,
            viewport_top: 0.0,
            viewport_height: 0.0,
            drag: None,
        }
    }

    pub(super) fn from_model(model: &CalendarModel, dark: bool) -> Self {
        let (first, count) = model.visible_day_range();
        let week_number = model
            .page
            .days
            .first()
            .and_then(|day| parse_date(&day.date))
            .map_or(0, time::Date::iso_week);
        let today = today_in(&model.page.timezone);
        let days = model
            .page
            .days
            .iter()
            .skip(first)
            .take(count)
            .map(|day| {
                let date = parse_date(&day.date).unwrap_or(today);
                DayPaint {
                    date,
                    label: date_heading(date),
                    is_today: date == today,
                }
            })
            .collect::<Vec<_>>();
        let events = timed_events(model, dark, first, count);
        let true_lanes = model.page.all_day_lanes;
        let drawn_lanes = if model.all_day_expanded || true_lanes <= COLLAPSED_LANES {
            true_lanes
        } else {
            VISIBLE_COLLAPSED_LANES
        };
        let banner_lanes = if model.all_day_expanded || true_lanes <= COLLAPSED_LANES {
            true_lanes
        } else {
            COLLAPSED_LANES
        };
        let (bands, hidden_per_day) = all_day_events(model, dark, first, count, drawn_lanes);
        let create = model
            .page
            .calendars
            .iter()
            .chain(model.month.calendars.iter())
            .find(|calendar| calendar.is_default && calendar.can_write)
            .map(|calendar| calendar_swatch(calendar, dark));
        Self {
            week_number,
            days,
            events,
            bands,
            hidden_per_day,
            banner_lanes,
            hour_height: 1.0,
            visible_hours: model.visible_hours.max(1),
            use_24_hour: model.use_24_hour,
            is_materialized: model.page.is_materialized,
            dark,
            timezone: model.page.timezone.clone(),
            create,
            viewport_top: 0.0,
            viewport_height: 0.0,
            drag: None,
        }
    }

    pub(super) fn content_top(&self) -> f64 {
        HEADING_HEIGHT + f64::from(self.banner_lanes) * LANE_HEIGHT
    }

    pub(super) fn height(&self) -> f64 {
        self.content_top() + 24.0 * self.hour_height
    }

    pub(super) fn fit_viewport(&mut self, viewport_height: f64) {
        self.viewport_height = viewport_height.max(1.0);
        self.hour_height = viewport_height.max(1.0) / f64::from(self.visible_hours.max(1));
    }

    pub(super) fn set_viewport_top(&mut self, viewport_top: f64) {
        self.viewport_top = viewport_top.max(0.0);
    }

    pub(super) fn geometry(&self, width: f64) -> Geometry {
        Geometry::new(self, width)
    }

    pub(super) fn semantic_status(&self) -> Option<String> {
        (!self.is_materialized).then(|| l10n::calendar_loading_range().to_owned())
    }
}

fn timed_events(model: &CalendarModel, dark: bool, first: usize, count: usize) -> Vec<EventPaint> {
    model
        .page
        .timed
        .iter()
        .filter_map(|event| {
            let day = usize::try_from(event.day).ok()?.checked_sub(first)?;
            (day < count).then(|| {
                let (background, foreground, border, calendar_name) =
                    colors(&model.page.calendars, &event.account, &event.calendar, dark);
                let title = title(&event.title);
                let range = format!(
                    "{}–{}",
                    clock(event.start_minutes, model.use_24_hour),
                    clock(event.end_minutes, model.use_24_hour)
                );
                EventPaint {
                    identity: EventIdentity {
                        account: event.account.clone(),
                        key: event.event.clone(),
                        occurrence: event.occurrence_start.clone(),
                    },
                    // The dashed border and hatched gutter below are invisible to a screen
                    // reader, so the label says it too (`docs/calendar.md` §4).
                    spoken: paint::spoken_with_hold(
                        &l10n::calendar_event_a11y(&title, &range, &calendar_name),
                        event.participation,
                    ),
                    awaiting: paint::is_awaiting(event.participation),
                    title,
                    day,
                    start_minutes: event.start_minutes,
                    end_minutes: event.end_minutes,
                    column: event.column,
                    columns: event.columns.max(1),
                    background,
                    foreground,
                    border,
                }
            })
        })
        .collect()
}

fn all_day_events(
    model: &CalendarModel,
    dark: bool,
    first: usize,
    count: usize,
    drawn_lanes: u32,
) -> (Vec<BandPaint>, Vec<u32>) {
    let mut hidden_per_day = vec![0; count];
    let mut bands = Vec::new();
    for band in &model.page.all_day {
        let start = usize::try_from(band.day).unwrap_or(usize::MAX);
        let end = start.saturating_add(usize::try_from(band.days).unwrap_or(0));
        let clipped_start = start.max(first);
        let clipped_end = end.min(first.saturating_add(count));
        if clipped_start >= clipped_end {
            continue;
        }
        if band.lane >= drawn_lanes {
            for day in clipped_start..clipped_end {
                hidden_per_day[day - first] += 1;
            }
            continue;
        }
        let (background, foreground, _, calendar_name) =
            colors(&model.page.calendars, &band.account, &band.calendar, dark);
        let title = title(&band.title);
        bands.push(BandPaint {
            identity: EventIdentity {
                account: band.account.clone(),
                key: band.event.clone(),
                occurrence: band.occurrence_start.clone(),
            },
            spoken: paint::spoken_with_hold(
                &l10n::calendar_event_a11y(&title, l10n::calendar_all_day(), &calendar_name),
                band.participation,
            ),
            awaiting: paint::is_awaiting(band.participation),
            title,
            day: clipped_start - first,
            days: clipped_end - clipped_start,
            lane: band.lane,
            background,
            foreground,
        });
    }
    (bands, hidden_per_day)
}

#[derive(Clone, Debug)]
pub(super) struct Hit {
    pub(super) identity: Option<EventIdentity>,
    pub(super) spoken: String,
    pub(super) rect: Rect,
}

pub(super) struct Geometry {
    pub(super) day_width: f64,
    pub(super) hits: Vec<Hit>,
}

impl Geometry {
    fn new(scene: &GridScene, width: f64) -> Self {
        let day_width = ((width - GUTTER) / pixels(scene.days.len().max(1))).max(1.0);
        let mut hits = scene
            .events
            .iter()
            .map(|event| timed_hit(scene, event, day_width))
            .collect::<Vec<_>>();
        hits.extend(scene.bands.iter().map(|band| band_hit(band, day_width)));
        for (day, hidden) in scene.hidden_per_day.iter().copied().enumerate() {
            if hidden > 0 {
                hits.push(Hit {
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
        Self { day_width, hits }
    }
}

fn timed_hit(scene: &GridScene, event: &EventPaint, day_width: f64) -> Hit {
    let lane_width = day_width / f64::from(event.columns);
    Hit {
        identity: Some(event.identity.clone()),
        spoken: event.spoken.clone(),
        rect: Rect {
            x: GUTTER + pixels(event.day) * day_width + f64::from(event.column) * lane_width + 1.0,
            y: scene.content_top() + f64::from(event.start_minutes) * scene.hour_height / 60.0,
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

pub(super) fn pixels(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

fn title(value: &str) -> String {
    if value.trim().is_empty() {
        l10n::event_no_title().to_owned()
    } else {
        value.to_owned()
    }
}

fn colors(
    calendars: &[mailcal_bindings::CalendarRow],
    account: &str,
    calendar: &str,
    dark: bool,
) -> (Rgb, Rgb, Rgb, String) {
    let Some(row) = calendars
        .iter()
        .find(|row| row.account == account && row.id == calendar)
    else {
        let (background, foreground, border) = paint::neutral_swatch();
        return (background, foreground, border, String::new());
    };
    let swatch = if dark {
        &row.color.dark
    } else {
        &row.color.light
    };
    (
        Rgb::from_hex(&swatch.background),
        Rgb::from_hex(&swatch.text),
        Rgb::from_hex(&swatch.border),
        row.name.clone(),
    )
}

fn calendar_swatch(calendar: &mailcal_bindings::CalendarRow, dark: bool) -> CreatePaint {
    let swatch = if dark {
        &calendar.color.dark
    } else {
        &calendar.color.light
    };
    CreatePaint {
        background: Rgb::from_hex(&swatch.background),
        border: Rgb::from_hex(&swatch.border),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreatePaint, DayPaint, EventPaint, Geometry, GridScene, Rect, Rgb, VISIBLE_COLLAPSED_LANES,
    };
    use crate::ui::calendar::EventIdentity;

    #[test]
    fn geometry_multiplies_only_day_minute_and_column_fractions() {
        let scene = GridScene {
            week_number: 30,
            days: vec![
                DayPaint {
                    date: time::Date::from_calendar_date(2026, time::Month::July, 20).unwrap(),
                    label: "Mon 20".to_owned(),
                    is_today: false,
                },
                DayPaint {
                    date: time::Date::from_calendar_date(2026, time::Month::July, 21).unwrap(),
                    label: "Tue 21".to_owned(),
                    is_today: true,
                },
            ],
            events: vec![EventPaint {
                identity: EventIdentity {
                    account: "a".to_owned(),
                    key: "e".to_owned(),
                    occurrence: String::new(),
                },
                title: "Fixture".to_owned(),
                spoken: "Fixture, 10:00–11:00, Work".to_owned(),
                day: 1,
                start_minutes: 600,
                end_minutes: 660,
                column: 1,
                columns: 2,
                background: Rgb::new(0.0, 0.0, 0.0),
                foreground: Rgb::new(1.0, 1.0, 1.0),
                border: Rgb::new(0.0, 0.0, 0.0),
                awaiting: false,
            }],
            bands: Vec::new(),
            hidden_per_day: vec![0, 0],
            banner_lanes: 0,
            hour_height: 60.0,
            visible_hours: 12,
            use_24_hour: true,
            is_materialized: true,
            dark: false,
            timezone: "Europe/Amsterdam".to_owned(),
            create: Some(CreatePaint {
                background: Rgb::new(0.2, 0.3, 0.4),
                border: Rgb::new(0.1, 0.2, 0.3),
            }),
            viewport_top: 0.0,
            viewport_height: 720.0,
            drag: None,
        };
        let geometry = Geometry::new(&scene, 468.0);
        assert!((geometry.day_width - 200.0).abs() < f64::EPSILON);
        assert_eq!(
            geometry.hits[0].rect,
            Rect {
                x: 369.0,
                y: 652.0,
                width: 98.0,
                height: 60.0
            }
        );
        assert_eq!(geometry.hits[0].spoken, "Fixture, 10:00–11:00, Work");
    }

    #[test]
    fn collapsed_banner_reserves_its_last_lane_for_honest_counts() {
        assert_eq!(VISIBLE_COLLAPSED_LANES, 2);
    }

    #[test]
    fn hour_height_tracks_the_current_viewport() {
        let mut scene = GridScene::empty();
        scene.banner_lanes = 2;
        scene.visible_hours = 12;
        scene.fit_viewport(480.0);
        assert!((scene.hour_height - 40.0).abs() < f64::EPSILON);

        scene.fit_viewport(360.0);
        assert!((scene.hour_height - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unanswered_ranges_expose_their_loading_status() {
        assert_eq!(
            GridScene::empty().semantic_status().as_deref(),
            Some(crate::l10n::calendar_loading_range())
        );
    }
}
