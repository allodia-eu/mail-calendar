//! Pure calendar editor state and provider-neutral intent construction.

use mailcal_bindings::{EventAttendee, EventDetail, EventRecurrence, Intent, RepeatSummary};
use time::{Duration, PrimitiveDateTime, Time};

use super::{
    date::{date_from_wall, now_wall, parse_date, parse_wall, wall_string},
    drag::CreateSlot,
};

/// A writable calendar offered by the create form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CalendarChoice {
    pub(super) account: String,
    pub(super) id: String,
    pub(super) label: String,
    pub(super) is_default: bool,
}

/// One stored event, reduced to the fields the Linux detail/editor needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EventDetails {
    pub(super) account: String,
    pub(super) key: String,
    pub(super) calendar: String,
    pub(super) calendar_name: String,
    pub(super) title: String,
    pub(super) all_day: bool,
    pub(super) timezone: String,
    pub(super) start: String,
    pub(super) end: String,
    pub(super) location: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) reminder_minutes: Option<i32>,
    pub(super) recurrence: Option<EventRecurrence>,
    /// The rule as the parts a sentence needs, decided by the core: see `repeat::sentence`.
    /// `None` for an event with no rule, and for one whose rule the core would not state exactly.
    pub(super) repeat_summary: Option<RepeatSummary>,
    pub(super) is_recurring: bool,
    pub(super) can_write: bool,
    /// The occurrence this detail describes, as the **core resolved** it; empty when it
    /// describes the series, which is what an agenda row and a one-off event always do. Non-empty
    /// is what makes a write ask *This event · All events* first.
    pub(super) occurrence: String,
    /// Everyone on the event, organiser first. Read-only; attendees change by iTIP, which is a
    /// separate feature.
    pub(super) attendees: Vec<EventAttendee>,
}

impl EventDetails {
    pub(super) fn from_binding(detail: EventDetail, calendar_name: String) -> Self {
        Self {
            account: detail.account,
            key: detail.key,
            calendar: detail.calendar,
            calendar_name,
            title: detail.title,
            all_day: detail.all_day,
            timezone: detail.timezone,
            start: detail.start,
            end: detail.end,
            location: detail.location,
            notes: detail.notes,
            reminder_minutes: detail.reminder_minutes,
            recurrence: detail.recurrence,
            repeat_summary: detail.repeat_summary,
            is_recurring: detail.is_recurring,
            can_write: detail.can_write,
            occurrence: detail.occurrence_start,
            attendees: detail.attendees,
        }
    }
}

/// Values read from the GTK form when the user saves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EventForm {
    pub(crate) title: String,
    pub(crate) start: String,
    pub(crate) end: String,
    pub(crate) all_day: bool,
    pub(crate) location: String,
    pub(crate) notes: String,
    pub(crate) calendar_index: u32,
}

/// An open create/edit form. All validation and date-shape decisions stay outside GTK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EventEditor {
    pub(super) editing: Option<EventDetails>,
    pub(super) zone: String,
    pub(super) title: String,
    pub(super) start: String,
    pub(super) end: String,
    pub(super) all_day: bool,
    pub(super) location: String,
    pub(super) notes: String,
    pub(super) choices: Vec<CalendarChoice>,
    pub(super) selected: u32,
}

impl EventEditor {
    pub(super) fn create(choices: Vec<CalendarChoice>, zone: String) -> Self {
        Self::create_at(choices, zone, now_wall())
    }

    fn create_at(choices: Vec<CalendarChoice>, zone: String, now: PrimitiveDateTime) -> Self {
        let in_one_hour = now + Duration::hours(1);
        let start = in_one_hour.replace_minute(0).unwrap_or(in_one_hour);
        let start = start.replace_second(0).unwrap_or(start);
        let selected = default_index(&choices);
        Self {
            editing: None,
            zone,
            title: String::new(),
            start: wall_string(start),
            end: wall_string(start + Duration::hours(1)),
            all_day: false,
            location: String::new(),
            notes: String::new(),
            choices,
            selected,
        }
    }

    pub(super) fn create_slot(
        choices: Vec<CalendarChoice>,
        zone: String,
        slot: CreateSlot,
    ) -> Self {
        let midnight = PrimitiveDateTime::new(slot.date, Time::MIDNIGHT);
        let start = midnight + Duration::minutes(i64::from(slot.start_minutes));
        let end = midnight + Duration::minutes(i64::from(slot.end_minutes));
        let selected = default_index(&choices);
        Self {
            editing: None,
            zone,
            title: String::new(),
            start: wall_string(start),
            end: wall_string(end),
            all_day: false,
            location: String::new(),
            notes: String::new(),
            choices,
            selected,
        }
    }

    pub(super) fn edit(detail: EventDetails, choices: Vec<CalendarChoice>) -> Self {
        let selected = choices
            .iter()
            .position(|choice| choice.account == detail.account && choice.id == detail.calendar)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0);
        let end = if detail.all_day {
            date_from_wall(&detail.end)
                .and_then(time::Date::previous_day)
                .map_or_else(|| detail.end.clone(), |date| date.to_string())
        } else {
            detail.end.clone()
        };
        Self {
            zone: detail.timezone.clone(),
            title: detail.title.clone(),
            start: detail.start.clone(),
            end,
            all_day: detail.all_day,
            location: detail.location.clone().unwrap_or_default(),
            notes: detail.notes.clone().unwrap_or_default(),
            choices,
            selected,
            editing: Some(detail),
        }
    }

    pub(super) const fn can_edit_form(&self) -> bool {
        self.editing.is_none()
    }

    /// Whether saving has to ask *This event · All events* first; true exactly when this
    /// editor was opened on one occurrence of a series.
    pub(super) fn asks_about_the_series(&self) -> bool {
        self.editing
            .as_ref()
            .is_some_and(|target| !target.occurrence.is_empty())
    }

    /// The intent a Save dispatches.
    ///
    /// `this_occurrence_only` splits an override out of the series instead of rewriting it. Both
    /// edges always travel: an occurrence's own times are not the series', so a single-occurrence
    /// edit naming neither would move it onto the master's clock.
    pub(super) fn intent(
        &self,
        form: &EventForm,
        this_occurrence_only: bool,
    ) -> Result<Intent, &'static str> {
        let title = form.title.trim();
        if title.is_empty() {
            return Err("title");
        }
        let (start, end) = if form.all_day {
            let start = date_from_wall(form.start.trim()).ok_or("start")?;
            let end = date_from_wall(form.end.trim()).ok_or("end")?;
            if end < start {
                return Err("range");
            }
            let exclusive = end.next_day().ok_or("end")?;
            (start.to_string(), exclusive.to_string())
        } else {
            let start = parse_wall(form.start.trim()).ok_or("start")?;
            let end = parse_wall(form.end.trim()).ok_or("end")?;
            if end <= start {
                return Err("range");
            }
            (wall_string(start), wall_string(end))
        };
        let notes = optional_text(&form.notes);
        let location = optional_text(&form.location);
        if let Some(detail) = &self.editing {
            return Ok(Intent::UpdateEvent {
                account: detail.account.clone(),
                key: detail.key.clone(),
                title: Some(title.to_owned()),
                start: Some(start),
                end: Some(end),
                notes: Some(notes.unwrap_or_default()),
                location: Some(location.unwrap_or_default()),
                occurrence: (this_occurrence_only && !detail.occurrence.is_empty())
                    .then(|| detail.occurrence.clone()),
                recurrence: None,
            });
        }
        let choice = usize::try_from(form.calendar_index)
            .ok()
            .and_then(|index| self.choices.get(index));
        Ok(Intent::CreateEvent {
            title: title.to_owned(),
            start,
            end,
            account: choice.map(|value| value.account.clone()),
            calendar: choice.map(|value| value.id.clone()),
            all_day: form.all_day,
            timezone: (!form.all_day && !self.zone.is_empty()).then(|| self.zone.clone()),
            notes,
            location,
            recurrence: None,
        })
    }

    pub(super) fn values_for_mode(start: &str, end: &str, all_day: bool) -> (String, String) {
        if all_day {
            return (
                date_from_wall(start).map_or_else(|| start.to_owned(), |date| date.to_string()),
                date_from_wall(end).map_or_else(|| end.to_owned(), |date| date.to_string()),
            );
        }
        let start_date = parse_date(start);
        let end_date = parse_date(end);
        let start = start_date.map_or_else(|| start.to_owned(), |date| format!("{date}T09:00:00"));
        let end_hour = if start_date == end_date { 10 } else { 9 };
        let end = end_date.map_or_else(
            || end.to_owned(),
            |date| format!("{date}T{end_hour:02}:00:00"),
        );
        (start, end)
    }
}

fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

fn default_index(choices: &[CalendarChoice]) -> u32 {
    choices
        .iter()
        .position(|choice| choice.is_default)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
