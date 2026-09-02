//! Calendar-write builders: the provider-format glue that turns the app's
//! calendar intents into the engine's neutral `EventDraft` / `EventPatch` values.
//!
//! This lives here (the provider-format glue layer) so the app stays free of a
//! direct `provider-caldav` dependency; the host submits the results via
//! `Engine::create_calendar_event` / `Engine::patch_calendar_event`.

use engine_api::{
    DraftRecurrence, EventDeletion, EventDraft, EventPatch, Occurrence, PatchTarget,
    RecurrenceBound, resolve_instant, to_local,
};
use engine_core::{
    calendar::Event,
    ids::{CalendarId, Uid},
    time::{CalendarDate, CalendarDateTime, LocalDateTime, TimeZoneId, UtcDateTime},
};

use crate::{
    AccountError, EventRecurrence, RecurrenceChange, SimpleRecurrence, describe_recurrence,
    recurrence_rule_of, undrawable_reason,
};

/// Builds a calendar event create draft: a fresh event for `title` running from `start`
/// to `end`, created in `calendar` under a new `uid`. The host submits it via
/// `Engine::create_calendar_event`, which mints the object key (a CalDAV href or a JMAP
/// id): the caller never names one.
///
/// `all_day` selects the form of the event and how `start`/`end` are read:
/// - **timed** (`all_day == false`): if `timezone` is a non-empty IANA id, `start`/`end` are **wall
///   clocks in that zone** (`2026-07-01T10:00:00`) and the event is created zoned there: so a
///   client creates an event in the device's own zone (like every calendar app), and a `create →
///   view → edit` round-trip reads back the same clock. When `timezone` is `None`/empty,
///   `start`/`end` are RFC 3339 UTC instants (`2026-07-01T10:00:00Z`) created as zoned-in-UTC; the
///   original behaviour, kept so a caller that has not yet moved to the zoned form still works.
/// - **all-day** (`all_day == true`): `start`/`end` are bare calendar dates (`2026-07-01`), and, as
///   RFC 5545 §3.6.1 requires and `EventDraft` documents: the **end is exclusive**: a one-day event
///   on the 1st ends on the 2nd. The client converts its inclusive on-screen end to the exclusive
///   date it passes here. `timezone` is ignored (all-day events are zoneless).
///
/// `notes`, when non-empty, becomes the event's description; `location`, when non-empty,
/// becomes the event's location. A create is the one write that sets a location from
/// nothing: an edit reshapes it through [`EventEdit::location`].
///
/// `recurrence` makes the new event repeat; `None` creates a one-off. Changing the rule
/// afterwards goes through [`EventEdit::recurrence`].
///
/// `uid` should be globally unique; `stamp` is the caller's clock for the create's
/// `DTSTAMP` (engine time types cannot read the system clock).
///
/// # Errors
///
/// Returns [`AccountError::CalendarWrite`] if the uid, the times/zone, (for all-day) the
/// dates, or the repeat rule are invalid.
// A flat builder over an event's independent create fields; each argument is a distinct
// scalar the host supplies from the create intent, so a parameter struct would only move the
// same values behind one more type. Kept flat deliberately.
#[allow(clippy::too_many_arguments)]
pub fn build_event_draft(
    calendar: CalendarId,
    uid: &str,
    title: &str,
    start: &str,
    end: &str,
    all_day: bool,
    timezone: Option<&str>,
    notes: Option<&str>,
    location: Option<&str>,
    recurrence: Option<&SimpleRecurrence>,
    stamp: UtcDateTime,
) -> Result<EventDraft, AccountError> {
    let uid = Uid::new(uid).map_err(|err| AccountError::CalendarWrite(err.to_string()))?;
    let (start, end) = if all_day {
        (parse_all_day(start)?, parse_all_day(end)?)
    } else {
        (parse_timed(start, timezone)?, parse_timed(end, timezone)?)
    };
    let repeat = recurrence
        .map(|rule| draft_recurrence(rule, &start))
        .transpose()?;
    let mut draft = EventDraft::new(calendar, uid, title, start, end, stamp);
    if let Some(repeat) = repeat {
        draft = draft.repeating(repeat);
    }
    if let Some(notes) = notes.filter(|notes| !notes.is_empty()) {
        draft = draft.description(notes);
    }
    if let Some(location) = location.filter(|location| !location.is_empty()) {
        draft = draft.location(location);
    }
    Ok(draft)
}

/// Parses a timed event's endpoint: a **wall clock in `timezone`** (`2026-07-01T09:00:00`) when
/// one is given, creating a zoned value there; else (back-compatibly) an RFC 3339 UTC instant
/// (`2026-07-01T09:00:00Z`) created as zoned-in-UTC.
fn parse_timed(value: &str, timezone: Option<&str>) -> Result<CalendarDateTime, AccountError> {
    match timezone.filter(|zone| !zone.is_empty()) {
        Some(zone) => {
            let zone = TimeZoneId::iana(zone).map_err(|err| {
                AccountError::CalendarWrite(format!("invalid zone {zone:?}: {err}"))
            })?;
            let local: LocalDateTime = value.parse().map_err(|err| {
                AccountError::CalendarWrite(format!("invalid wall clock {value:?}: {err}"))
            })?;
            Ok(CalendarDateTime::Zoned { local, zone })
        }
        None => utc_to_zoned(parse_utc(value)?),
    }
}

/// Parses an RFC 3339 UTC instant for a timed event's endpoint.
fn parse_utc(value: &str) -> Result<UtcDateTime, AccountError> {
    value
        .parse()
        .map_err(|err| AccountError::CalendarWrite(format!("invalid time {value:?}: {err}")))
}

/// Parses a bare `YYYY-MM-DD` calendar date into an all-day (zoneless) value.
fn parse_all_day(value: &str) -> Result<CalendarDateTime, AccountError> {
    let date: CalendarDate = value
        .parse()
        .map_err(|err| AccountError::CalendarWrite(format!("invalid date {value:?}: {err}")))?;
    Ok(CalendarDateTime::Date(date))
}

fn utc_to_zoned(instant: UtcDateTime) -> Result<CalendarDateTime, AccountError> {
    let local = to_local(instant, &TimeZoneId::utc()).map_err(|err| {
        AccountError::CalendarWrite(format!(
            "cannot resolve UTC instant {instant} in UTC: {err}"
        ))
    })?;
    Ok(CalendarDateTime::Zoned {
        local,
        zone: TimeZoneId::utc(),
    })
}

/// An edit to a **stored** calendar event, expressed in the event's **own wall clock**.
///
/// The wall clock is the point. A stored event is zoned (`Europe/Amsterdam`), floating, or
/// all-day, and an edit must not change which; resolving it to a UTC instant and writing
/// that back moves the event for every reader in another zone, and turns an all-day event
/// into an instant. So an edit says *what the clock on the wall reads*, and
/// [`build_event_patch`] renders it in whatever form the stored event already has (the
/// engine's patcher rejects a form change outright; this makes one impossible to ask for).
///
/// A host editing in a display zone other than the event's own must therefore convert to the
/// event's zone first (`engine_api::to_local` / `resolve_instant`): the core will not guess.
/// **A drag does not have to**: [`apply_event_drag`](crate::apply_event_drag) takes the
/// gesture as a signed offset in days and minutes, which is the same number in either zone,
/// and applies it to the event's own clock here. The reasoning in full is on that function.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventEdit {
    /// The new title, or `None` to leave it alone.
    pub title: Option<String>,
    /// The new start, as wall clock in the event's own zone (its date, if all-day).
    pub start: Option<LocalDateTime>,
    /// The new end, same terms. For an all-day event the end date is **exclusive**; a
    /// one-day event on the 1st ends on the 2nd.
    pub end: Option<LocalDateTime>,
    /// The new description ("notes"), three-state: `None` leaves it alone, `Some("")` clears
    /// it, `Some(text)` sets it.
    pub notes: Option<String>,
    /// The new location, same three-state semantics as [`notes`](Self::notes).
    pub location: Option<String>,
    /// What happens to the repeat rule: `None` leaves the series exactly as it is,
    /// [`Set`](RecurrenceChange::Set) replaces the rule, [`Clear`](RecurrenceChange::Clear)
    /// makes the event a single one.
    ///
    /// Only a **series** edit may carry one: a rule belongs to the series, and one
    /// occurrence is an instance *of* a rule rather than a holder of one. Setting a rule on
    /// an event whose stored rule this app cannot describe in full
    /// ([`EventRecurrence::Complex`]) is refused: the client seeded its editor from a
    /// partial picture, and saving it back would drop the parts it never saw.
    pub recurrence: Option<RecurrenceChange>,
    /// Which occurrence of a recurring event this edits, named by its **original** start.
    ///
    /// `None` edits the **whole series**; every occurrence moves. `Some(original_start)`
    /// edits that one occurrence, splitting a `RECURRENCE-ID` override out of the series
    /// and leaving the rest of it untouched. There is no default on purpose: dragging one
    /// Tuesday standup is not the same as rewriting every Monday to eternity, and only the
    /// user knows which they meant: so ask them.
    ///
    /// Editing a single occurrence requires `start` **and** `end` (the series master's are
    /// the *first* occurrence's times, not this one's); pass them unchanged if the edit is
    /// not a move.
    pub occurrence: Option<LocalDateTime>,
    /// The occurrence [`start`](Self::start) and [`end`](Self::end) were **read from**, when
    /// this edit is scoped to the series but its editor was opened on one occurrence.
    ///
    /// An editor opened on one occurrence shows that occurrence's times, so writing them onto
    /// the master would move the series' start to that occurrence and every earlier one would
    /// stop existing. Naming where the clocks came from turns the edit into the *shift* the
    /// user made, applied to the series' own clock, which is what a drag on a series does.
    ///
    /// `None` means the clocks are the series' own, which is what an editor opened on the
    /// series shows. Ignored when [`occurrence`](Self::occurrence) is set, since that edit
    /// lands on the occurrence its clocks already describe. Setting it requires **both** edges:
    /// a shift is not a shift with one end of it missing.
    pub times_from_occurrence: Option<LocalDateTime>,
}

/// Builds a calendar event **patch** from `edit` against the stored `event`.
///
/// The returned [`PatchTarget`] and [`EventPatch`] are the intent a host submits via
/// `Engine::patch_calendar_event`. The adapter applies the patch to the stored provider
/// payload rather than rebuilding it, so the recurrence rule, attendees, alarms and
/// timezone survive.
///
/// `stamp` is the caller's clock: engine time types cannot read the system clock, and the
/// revision bookkeeping needs a `DTSTAMP`.
///
/// This builds the *intent* only; it does not re-validate what the adapter already does.
/// An inverted interval (an end before its start; including a start dragged past the
/// unchanged end) is caught by the engine's patcher against the event's *effective* end,
/// which this builder cannot see, so it is not re-checked here.
///
/// # Errors
///
/// Returns [`AccountError::CalendarWrite`] if a wall clock cannot be rendered in the
/// stored event's form (an out-of-range date), or if the edit asks for a recurrence change
/// that must not be written; see [`EventEdit::recurrence`].
pub fn build_event_patch(
    event: &Event,
    edit: &EventEdit,
    stamp: UtcDateTime,
) -> Result<(PatchTarget, EventPatch), AccountError> {
    let mut patch = EventPatch::new(stamp);
    if let Some(title) = &edit.title {
        patch = patch.summary(title);
    }
    // Clocks read from one occurrence, on an edit meant for the series, are a shift rather than
    // the series' own times; see `EventEdit::times_from_occurrence` for why writing them straight
    // through deletes every occurrence before the one the editor was opened on.
    let (start, end) = match (edit.times_from_occurrence, edit.occurrence) {
        (Some(read_from), None) => {
            let (Some(edited_start), Some(edited_end)) = (edit.start, edit.end) else {
                return Err(AccountError::CalendarWrite(
                    "an edit shifted from an occurrence needs both its start and its end"
                        .to_owned(),
                ));
            };
            let (start, end) = crate::calendar_drag::series_bounds_after(
                event,
                read_from,
                edited_start,
                edited_end,
            )?;
            (Some(start), Some(end))
        }
        _ => (edit.start, edit.end),
    };
    if let Some(start) = start {
        patch = patch.start(in_event_form(&event.start, start)?);
    }
    if let Some(end) = end {
        patch = patch.end(in_event_form(&event.start, end)?);
    }
    if let Some(notes) = &edit.notes {
        patch = if notes.is_empty() {
            patch.clear_description()
        } else {
            patch.description(notes)
        };
    }
    if let Some(location) = &edit.location {
        patch = if location.is_empty() {
            patch.clear_location()
        } else {
            patch.location(location)
        };
    }
    if let Some(change) = &edit.recurrence {
        if edit.occurrence.is_some() {
            return Err(AccountError::CalendarWrite(
                "a repeat rule belongs to the series, not to one occurrence".to_owned(),
            ));
        }
        patch = match change {
            RecurrenceChange::Clear => patch.clear_recurrence(),
            RecurrenceChange::Set(rule) => {
                // The caller can only have seen what `describe_recurrence` showed it, so a
                // rule it could not describe in full is a rule it must not write back;
                // whatever the editor holds is missing the parts that made it complex, and
                // the save would silently drop them. A client is gated on the same answer;
                // this checks it again, because a write must not trust its caller.
                if matches!(
                    event.recurrence.as_ref().and_then(describe_recurrence),
                    Some(EventRecurrence::Complex)
                ) {
                    return Err(AccountError::CalendarWrite(
                        "this event's repeat rule is richer than the editor can describe"
                            .to_owned(),
                    ));
                }
                patch.recurrence(draft_recurrence(rule, &event.start)?)
            }
        };
    }
    let target = match edit.occurrence {
        Some(original) => PatchTarget::Instance(event_occurrence(event, original)?),
        None => PatchTarget::Series,
    };

    Ok((target, patch))
}

/// Builds a calendar event **delete** against the stored `event`: the whole series, or the
/// single occurrence that originally started at `occurrence`.
///
/// The two are different requests, and only the user knows which they meant; deleting
/// Tuesday's standup is either cancelling that Tuesday or cancelling the standup: so there
/// is no default here either. Removing one occurrence is an *edit* of the stored document on
/// the transports that have no instance to delete, which is why it carries `stamp`.
///
/// # Errors
///
/// Returns [`AccountError::CalendarWrite`] if the occurrence's wall clock cannot be rendered
/// in the stored event's form.
pub fn build_event_deletion(
    event: &Event,
    occurrence: Option<LocalDateTime>,
    stamp: UtcDateTime,
) -> Result<EventDeletion, AccountError> {
    Ok(match occurrence {
        None => EventDeletion::of(event),
        Some(original) => {
            EventDeletion::occurrence(event, event_occurrence(event, original)?, stamp)
        }
    })
}

/// Names one occurrence of `event` by its **original** start, in the terms every transport
/// addresses one with.
///
/// The wall clock names the occurrence on three transports; Google addresses one by that
/// start **in UTC** and refuses a timed target that has not resolved it. No adapter carries
/// the tzdata to resolve one, but this crate reaches the engine's (`resolve_instant`), and it
/// has both halves here: so it resolves rather than handing the adapter something it will
/// reject. An all-day or floating occurrence resolves to `None` and needs none: Google
/// addresses that one by date.
fn event_occurrence(event: &Event, original: LocalDateTime) -> Result<Occurrence, AccountError> {
    let start = in_event_form(&event.start, original)?;
    let instant =
        resolve_instant(&start).map_err(|err| AccountError::CalendarWrite(err.to_string()))?;
    Ok(match instant {
        Some(instant) => Occurrence::at(start, instant),
        None => Occurrence::starting(start),
    })
}

/// Builds the engine's [`DraftRecurrence`] for a series whose event starts at `start`.
///
/// A rule that ends on a date ends at a **wall clock in the event's own zone**, and RFC 5545
/// §3.3.10 requires `UNTIL` in UTC once the event is zoned: a conversion that needs tzdata no
/// adapter carries. So the instant is resolved here and travels with the rule. A floating or
/// all-day series resolves to none and needs none.
fn draft_recurrence(
    rule: &SimpleRecurrence,
    start: &CalendarDateTime,
) -> Result<DraftRecurrence, AccountError> {
    // Before anything is built: a rule this app cannot expand stores an event that draws
    // nowhere, and an event absent from the grid is indistinguishable from one that was never
    // saved. Both writes funnel through here, so both are covered by the one check.
    if let Some(reason) = undrawable_reason(rule) {
        return Err(AccountError::CalendarWrite(format!(
            "the repeat rule cannot be shown: {reason}"
        )));
    }
    let rule = recurrence_rule_of(rule).ok_or_else(|| {
        AccountError::CalendarWrite("the repeat rule describes no series".to_owned())
    })?;
    let RecurrenceBound::Until(until) = rule.bound else {
        return Ok(DraftRecurrence::new(rule));
    };
    let end = in_event_form(start, until)?;
    let instant =
        resolve_instant(&end).map_err(|err| AccountError::CalendarWrite(err.to_string()))?;
    Ok(match instant {
        Some(instant) => DraftRecurrence::ending_at(rule, instant),
        None => DraftRecurrence::new(rule),
    })
}

/// Renders a wall clock in the same **form** the stored event uses; zoned in its own
/// zone, floating, or an all-day date: so an edit can never silently convert the event.
///
/// This is what keeps the engine's form guard from ever firing in practice: the core
/// cannot even express "move this Amsterdam event to a UTC instant".
fn in_event_form(
    current: &CalendarDateTime,
    local: LocalDateTime,
) -> Result<CalendarDateTime, AccountError> {
    Ok(match current {
        CalendarDateTime::Date(_) => CalendarDateTime::Date(
            CalendarDate::new(local.year(), local.month(), local.day())
                .map_err(|err| AccountError::CalendarWrite(err.to_string()))?,
        ),
        CalendarDateTime::Floating(_) => CalendarDateTime::Floating(local),
        CalendarDateTime::Zoned { zone, .. } => CalendarDateTime::Zoned {
            local,
            zone: zone.clone(),
        },
    })
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "calendar_series_shift_tests.rs"]
mod series_shift_tests;

#[cfg(test)]
#[path = "calendar_recurrence_tests.rs"]
mod recurrence_tests;
