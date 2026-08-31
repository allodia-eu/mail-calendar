//! The calendar view-model: an agenda snapshot of events, soonest first.
//!
//! A pure projection over the engine's [`Event`]s, mirroring [`crate::view`] for mail;
//! state lives in the engine, the host renders the snapshot.

use engine_api::{Event, TimeZoneId, UtcDateTime, resolve_instant, resolve_instant_in};

/// An event paired with the id of the account it belongs to: the app drives several
/// accounts through one engine, so the account travels alongside each event into the
/// agenda (rows are tagged with it, so an action routes to the owning account). Mirrors
/// [`crate::view::AccountMessage`] for mail.
#[derive(Debug, Clone)]
pub struct AccountEvent {
    /// The owning account's id.
    pub account: String,
    /// The event.
    pub event: Event,
    /// Whether this account's calendar provider supports writes. The host uses this to
    /// hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// How this account answered. Supplied by the caller because only it knows the account's
    /// address **set** (its aliases), which is what decides which `ATTENDEE` line is "me".
    pub participation: crate::invitation::ResponseStatus,
}

/// An immutable agenda snapshot for a host to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarSnapshot {
    /// The events, soonest first.
    pub events: Vec<EventRow>,
    /// The IANA id of the active display zone the rows were ordered in and that the
    /// host localises each `Z`-suffixed [`EventRow::start`] to (empty only on the
    /// default snapshot before a zone is set).
    pub timezone: String,
}

/// One agenda row: an event's key, title, and formatted start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    /// The id of the account this event belongs to: so an action on the row (e.g.
    /// delete) routes to the owning account rather than scanning every account (two
    /// accounts can mint the same event key).
    pub account: String,
    /// The event's provider key (stable identity).
    pub key: String,
    /// The event's title (a placeholder if empty).
    pub title: String,
    /// The start as an RFC 3339-style string the host renders in the device's local
    /// zone: any zoned start (UTC or a named zone) is resolved to its absolute UTC
    /// instant and carries a `Z` suffix (`2026-06-27T20:00:00Z`) for the host to
    /// convert; a floating (zoneless) wall-clock has none; all-day/none is empty.
    /// (The view-model is tzdata-free; the engine resolves the instant and the native
    /// UI does the final localisation.)
    pub start: String,
    /// Whether this event's owning account supports calendar writes. The host uses this
    /// to hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// How this account answered, when the event is something it was invited to.
    ///
    /// The agenda lists an unanswered hold the same way the grid draws one; visually distinct
    /// **and** labelled, never one without the other. Declined events never reach the agenda:
    /// its event set comes from the same filtered occurrence cache the grid uses, so the one
    /// hiding rule covers every surface (`docs/invitations.md`).
    pub participation: crate::invitation::ResponseStatus,
}

/// Builds the agenda snapshot, soonest first, ordered in the `zone` display zone.
///
/// Ordering is by each event's **absolute instant** resolved in `zone`
/// ([`resolve_instant_in`]): a zoned event by its own zone, a floating event by
/// `zone`, an all-day event at `zone`'s local midnight: a correct total order even
/// when the agenda mixes zones and kinds (the previous wall-clock ordering misplaced
/// cross-zone events). The chosen `zone` is echoed in [`CalendarSnapshot::timezone`]
/// for the host to localise each row's start against.
#[must_use]
pub fn build(events: &[AccountEvent], zone: &TimeZoneId) -> CalendarSnapshot {
    let mut sorted: Vec<&AccountEvent> = events.iter().collect();
    sorted.sort_by_cached_key(|item| sort_key(&item.event, zone));
    let events = sorted
        .into_iter()
        .map(|item| EventRow {
            account: item.account.clone(),
            key: item.event.id.key().as_str().to_owned(),
            title: if item.event.title.is_empty() {
                "(no title)".to_owned()
            } else {
                item.event.title.clone()
            },
            start: start_instant(&item.event),
            can_write: item.can_write,
            participation: item.participation,
        })
        .collect();
    CalendarSnapshot {
        events,
        timezone: zone.as_str().to_owned(),
    }
}

/// The total-order sort key for an event in `zone`: its resolved absolute instant,
/// with the provider key as a deterministic tiebreaker. An event the bundled tzdb
/// cannot resolve (a custom/embedded `VTIMEZONE`, or an out-of-range value) has no
/// instant; the leading `is_none()` flag parks those **last** (`false` < `true`)
/// rather than at the top of a "soonest first" agenda.
fn sort_key(event: &Event, zone: &TimeZoneId) -> (bool, Option<UtcDateTime>, String) {
    let instant = resolve_instant_in(&event.start, zone).ok();
    (
        instant.is_none(),
        instant,
        event.id.key().as_str().to_owned(),
    )
}

/// The event start as a host-renderable string. Any **zoned** start; UTC or a
/// named IANA zone, is resolved through the engine's bundled tzdata to its absolute
/// UTC instant and emitted with a `Z` suffix (`2026-06-27T20:00:00Z`), so the host
/// localises it to the device zone no matter which zone the event was authored in. A
/// **floating** (zoneless) wall-clock has no fixed instant, so it is emitted as-is
/// without a suffix; an all-day or timeless start is empty. A zone the bundled
/// tzdata cannot resolve (a custom embedded `VTIMEZONE`) falls back to the bare
/// wall-clock rather than failing the snapshot.
fn start_instant(event: &Event) -> String {
    if let Ok(Some(instant)) = resolve_instant(&event.start) {
        return format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            instant.year(),
            instant.month(),
            instant.day(),
            instant.hour(),
            instant.minute(),
            instant.second(),
        );
    }
    event.start.local().map_or_else(String::new, |local| {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            local.year(),
            local.month(),
            local.day(),
            local.hour(),
            local.minute(),
            local.second(),
        )
    })
}

#[cfg(test)]
mod tests {
    use engine_api::Event;
    use engine_core::{
        ids::{CalendarId, EventId, Uid},
        membership::Memberships,
        time::{CalendarDateTime, LocalDateTime, TimeZoneId},
    };

    use super::{AccountEvent, build};

    fn utc() -> TimeZoneId {
        TimeZoneId::utc()
    }

    /// Tags an [`Event`] with an account so it can be projected by [`build`]; the tests
    /// here exercise ordering/formatting, so the account is a fixed placeholder.
    fn tagged(event: Event) -> AccountEvent {
        AccountEvent {
            account: "acct".to_owned(),
            event,
            can_write: true,
            // These tests exercise ordering and formatting; a fixed accepted answer keeps them
            // about that. The unanswered-hold rendering is covered in `grid_tests`.
            participation: crate::invitation::ResponseStatus::Accepted,
        }
    }

    fn event(id: &str, uid: &str, title: &str, hour: u8) -> AccountEvent {
        let mut event = Event::new(
            EventId::try_from(id).unwrap(),
            Uid::new(uid).unwrap(),
            Memberships::of_one(CalendarId::try_from("cal").unwrap()),
            CalendarDateTime::utc(LocalDateTime::new(2026, 6, 1, hour, 0, 0).unwrap()),
        );
        event.title = title.to_owned();
        tagged(event)
    }

    #[test]
    fn agenda_is_soonest_first_with_formatted_starts() {
        let events = vec![
            event("e2", "u2@h", "Lunch", 12),
            event("e1", "u1@h", "Standup", 9),
        ];
        let snapshot = build(&events, &utc());
        assert_eq!(snapshot.events.len(), 2);
        // 09:00 sorts before 12:00.
        assert_eq!(snapshot.events[0].title, "Standup");
        // A UTC start is emitted as a Z-suffixed instant for the host to localise.
        assert_eq!(snapshot.events[0].start, "2026-06-01T09:00:00Z");
        assert_eq!(snapshot.events[1].title, "Lunch");
        // The chosen display zone is echoed for the host to localise against.
        assert_eq!(snapshot.timezone, "Etc/UTC");
    }

    #[test]
    fn a_floating_start_has_no_zone_suffix() {
        // A floating (zoneless) wall-clock is shown as-is: no `Z` (not a UTC instant).
        let mut floating = event("e3", "u3@h", "Floating", 9);
        floating.event.start =
            CalendarDateTime::Floating(LocalDateTime::new(2026, 6, 1, 9, 0, 0).unwrap());
        let snapshot = build(&[floating], &utc());
        assert_eq!(snapshot.events[0].start, "2026-06-01T09:00:00");
    }

    #[test]
    fn a_named_zone_start_is_resolved_to_a_utc_instant() {
        // The real fix: an event authored in Europe/Amsterdam (summer, UTC+2) at
        // 22:00 is emitted as its absolute instant 20:00Z, so the host localises it
        // correctly regardless of the device's zone: not the bare 22:00 wall-clock.
        let mut zoned = event("e4", "u4@h", "Amsterdam evening", 22);
        zoned.event.start = CalendarDateTime::Zoned {
            local: LocalDateTime::new(2026, 6, 27, 22, 0, 0).unwrap(),
            zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
        };
        let snapshot = build(&[zoned], &utc());
        assert_eq!(snapshot.events[0].start, "2026-06-27T20:00:00Z");
    }

    #[test]
    fn agenda_orders_by_absolute_instant_across_zones() {
        // Ordering must use the resolved instant, not the wall-clock. A New York
        // event at 09:00 (UTC-4 summer = 13:00Z) comes AFTER an Amsterdam event at
        // 11:00 (UTC+2 = 09:00Z), even though 11:00 > 09:00 as bare wall-clocks.
        let mut ams = event("ams", "ams@h", "Amsterdam 11:00", 0);
        ams.event.start = CalendarDateTime::Zoned {
            local: LocalDateTime::new(2026, 6, 27, 11, 0, 0).unwrap(),
            zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
        };
        let mut ny = event("ny", "ny@h", "New York 09:00", 0);
        ny.event.start = CalendarDateTime::Zoned {
            local: LocalDateTime::new(2026, 6, 27, 9, 0, 0).unwrap(),
            zone: TimeZoneId::iana("America/New_York").unwrap(),
        };
        let snapshot = build(&[ny, ams], &utc());
        assert_eq!(snapshot.events[0].title, "Amsterdam 11:00"); // 09:00Z first
        assert_eq!(snapshot.events[1].title, "New York 09:00"); // 13:00Z second
    }

    #[test]
    fn an_unresolvable_zone_sorts_last_not_first() {
        // An event whose zone the bundled tzdb cannot resolve (a custom/embedded
        // VTIMEZONE the CalDAV path can produce) has no instant; it must park at the
        // END of a soonest-first agenda, not jump to the top.
        let normal = event("ok", "ok@h", "Resolvable noon", 12);
        let mut custom = event("cu", "cu@h", "Custom-zone event", 0);
        custom.event.start = CalendarDateTime::Zoned {
            local: LocalDateTime::new(2026, 6, 1, 1, 0, 0).unwrap(),
            zone: TimeZoneId::custom("/example.com/MyTZ").unwrap(),
        };
        let snapshot = build(&[custom, normal], &utc());
        assert_eq!(snapshot.events[0].title, "Resolvable noon");
        assert_eq!(snapshot.events[1].title, "Custom-zone event");
    }
}
