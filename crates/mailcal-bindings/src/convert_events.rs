//! Conversions for calendar-event writes: the FFI [`EventIntent`] into the core's. Split from
//! `convert` to keep each file under the 500-line limit.

use engine_api::LocalDateTime;
use mailcal_account::{EventDrag, EventEdge as AppEventEdge, EventEdit};
use mailcal_app::{EventIntent as AppEventIntent, EventRef};

use crate::{EventEdge, EventIntent};

/// Binds each write's account and key into one [`EventRef`] at the boundary, as
/// `Intent::SelectFolder` binds a folder's, so no write can name a key without its account. A
/// malformed reference or wall clock drops the whole intent rather than editing the wrong event
/// or the wrong time.
pub(crate) fn event_intent(intent: EventIntent) -> Result<AppEventIntent, String> {
    let event = |account: String, key: String| {
        EventRef::from_parts(&account, key).ok_or_else(|| "invalid event reference".to_owned())
    };
    Ok(match intent {
        EventIntent::Create {
            title,
            start,
            end,
            account,
            calendar,
            all_day,
            timezone,
            notes,
            location,
            recurrence,
            invitees,
        } => AppEventIntent::Create {
            title,
            start,
            end,
            account,
            calendar,
            all_day,
            timezone,
            notes,
            location,
            recurrence: recurrence.map(Into::into),
            invitees: crate::records_meeting::invitees(invitees)?,
        },
        EventIntent::Update {
            account,
            key,
            title,
            start,
            end,
            notes,
            location,
            occurrence,
            recurrence,
            times_from_occurrence,
            invitees,
        } => AppEventIntent::Update {
            event: event(account, key)?,
            edit: EventEdit {
                title: title.filter(|title| !title.is_empty()),
                start: parse_local(start)?,
                end: parse_local(end)?,
                notes,
                location,
                recurrence: recurrence.map(Into::into),
                invitees: crate::records_meeting::invitee_patch(invitees)?,
                occurrence: parse_local(occurrence)?,
                times_from_occurrence: parse_local(times_from_occurrence)?,
            },
        },
        EventIntent::Move {
            account,
            key,
            edge,
            days,
            minutes,
            occurrence,
        } => AppEventIntent::Move {
            event: event(account, key)?,
            drag: EventDrag {
                edge: match edge {
                    EventEdge::Whole => AppEventEdge::Whole,
                    EventEdge::Start => AppEventEdge::Start,
                    EventEdge::End => AppEventEdge::End,
                },
                days,
                minutes,
                // The same parse as the editor's, on the same token: a malformed value
                // drops the whole intent rather than quietly moving the entire series when
                // the user asked for one Tuesday.
                occurrence: parse_local(occurrence)?,
            },
        },
        EventIntent::Delete {
            account,
            key,
            occurrence,
        } => AppEventIntent::Delete {
            event: event(account, key)?,
            // The same parse as the editor's, on the same token: a malformed value drops
            // the whole intent rather than deleting the entire series when the user asked
            // for one Tuesday.
            occurrence: parse_local(occurrence)?,
        },
    })
}

/// A wall-clock edit field: absent or empty leaves the property unchanged; a value is parsed as
/// a [`LocalDateTime`] in the event's own zone.
fn parse_local(value: Option<String>) -> Result<Option<LocalDateTime>, String> {
    match value.filter(|value| !value.is_empty()) {
        Some(value) => value
            .parse::<LocalDateTime>()
            .map(Some)
            .map_err(|err| format!("invalid wall-clock {value:?}: {err}")),
        None => Ok(None),
    }
}
