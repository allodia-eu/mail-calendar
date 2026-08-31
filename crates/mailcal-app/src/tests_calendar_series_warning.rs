//! Whether the user is warned before a series edit throws their own work away.
//!
//! The decision itself is pinned in `mailcal_account::series_warning`. What can only be
//! asserted here is that all three facts it needs actually reach it: the account's capability,
//! which lives on a connected provider and nowhere in the stored event; the overrides, which
//! live on the event and nowhere in the capability, and what the **edit** changes, which lives
//! in neither and arrives with the save. A query wired to two of the three reads perfectly well
//! and is wrong; silently for the first two, loudly for the third.

use std::sync::{Arc, Mutex};

use engine_api::{Recurrence, RecurrenceOverride};
use fakes::{CalendarFake, calendar_account, calendar_app, evt};
use mailcal_account::{EventEdit, SeriesEditWarning};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A weekly standup with one occurrence the user has singled out.
fn series_with_an_override() -> engine_api::Event {
    let event = fakes::weekly_event_with_a_moved_occurrence("standup", 0, 9, 30, 1, 14);
    assert!(
        event
            .recurrence
            .as_ref()
            .is_some_and(|recurrence| !recurrence.overrides.is_empty()),
        "the fixture must actually hold an override, or this file proves nothing"
    );
    event
}

/// The same series with nothing singled out.
fn clean_series() -> engine_api::Event {
    let mut event = fakes::weekly_event_from_today("standup", 0, 9, 30);
    event.recurrence = Some(Recurrence::from_rule(engine_api::RecurrenceRule::new(
        engine_api::Frequency::Weekly,
    )));
    event
}

/// An edit that leaves everything alone: the base every case below changes one thing on.
fn nothing() -> EventEdit {
    EventEdit {
        title: None,
        start: None,
        end: None,
        notes: None,
        location: None,
        recurrence: None,
        occurrence: None,
    }
}

/// An edit that moves the series to a time it certainly does not already have.
fn moves_it() -> EventEdit {
    EventEdit {
        start: Some(engine_api::LocalDateTime::new(2031, 3, 4, 8, 0, 0).unwrap()),
        end: Some(engine_api::LocalDateTime::new(2031, 3, 4, 9, 0, 0).unwrap()),
        ..nothing()
    }
}

/// An edit that only renames the series.
fn renames_it() -> EventEdit {
    EventEdit {
        title: Some("Renamed".to_owned()),
        ..nothing()
    }
}

/// The warning a client is given for `edit` on the opened event.
async fn warning_for(provider: CalendarFake, edit: EventEdit) -> Option<SeriesEditWarning> {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.series_edit_warning(&evt("acct-a", "standup"), &edit)
        .await
}

#[tokio::test]
async fn a_server_that_discards_overrides_warns_about_a_series_this_user_has_touched() {
    assert_eq!(
        warning_for(
            CalendarFake::losing_overrides(vec![series_with_an_override()]),
            moves_it(),
        )
        .await,
        Some(SeriesEditWarning::OccurrencesReset),
        "the capability reached the decision"
    );
}

#[tokio::test]
async fn the_same_server_says_nothing_about_a_series_nobody_has_touched() {
    // The half that keeps the warning meaningful. Wiring the capability and forgetting the
    // overrides would warn on every repeating event on the account, which is how a dialog
    // becomes something people click past.
    assert_eq!(
        warning_for(
            CalendarFake::losing_overrides(vec![clean_series()]),
            moves_it()
        )
        .await,
        None
    );
}

#[tokio::test]
async fn a_server_that_keeps_overrides_says_nothing_even_about_a_touched_series() {
    // The default fake keeps them, which is CalDAV and JMAP. Wiring the overrides and
    // forgetting the capability would warn on those two as well: a warning about a loss that
    // cannot happen.
    assert_eq!(
        warning_for(
            CalendarFake::with_events(vec![series_with_an_override()]),
            moves_it()
        )
        .await,
        None
    );
}

#[tokio::test]
async fn an_excluded_occurrence_counts_as_the_users_own_work() {
    // Cancelling one Tuesday is a per-occurrence change like any other, and a series edit
    // discards it the same way. It is stored as an exclusion rather than a patch, so a check
    // that only counted patches would miss it.
    let mut event = clean_series();
    event
        .recurrence
        .as_mut()
        .expect("the fixture repeats")
        .overrides
        .insert(
            engine_api::LocalDateTime::new(2026, 1, 12, 9, 30, 0).unwrap(),
            RecurrenceOverride::Excluded,
        );

    assert_eq!(
        warning_for(CalendarFake::losing_overrides(vec![event]), moves_it()).await,
        Some(SeriesEditWarning::OccurrencesReset)
    );
}

#[tokio::test]
async fn a_rename_is_not_warned_about_by_a_server_that_only_resets_times() {
    // The third fact. This fake destroys an override when the series **moves**, and leaves an
    // override's own fields alone: so a rename costs this user nothing, and saying otherwise
    // spends the attention the warning exists for on a loss that will not happen.
    assert_eq!(
        warning_for(
            CalendarFake::losing_overrides(vec![series_with_an_override()]),
            renames_it(),
        )
        .await,
        None,
        "a flag is owed by the edit that causes it, not by every edit"
    );
}

#[tokio::test]
async fn an_edit_that_changes_nothing_is_not_warned_about() {
    // A user who opens the editor and presses Save without touching anything has nothing to
    // lose, whatever the server does.
    assert_eq!(
        warning_for(
            CalendarFake::losing_overrides(vec![series_with_an_override()]),
            nothing(),
        )
        .await,
        None
    );
}

#[tokio::test]
async fn retyping_the_same_value_is_not_a_change() {
    // Why the comparison is against what is **stored** rather than against what the form was
    // seeded with: an editor that sends every field back would otherwise warn on every save.
    let stored = series_with_an_override();
    let edit = EventEdit {
        title: Some(stored.title.clone()),
        ..nothing()
    };
    assert_eq!(
        warning_for(CalendarFake::losing_overrides(vec![stored]), edit).await,
        None
    );
}

#[tokio::test]
async fn one_occurrences_edit_is_never_warned_about() {
    // It writes an override of its own and leaves every other occurrence alone, so there is
    // nothing to lose and nothing to ask; on any server, however much work the series holds.
    let edit = EventEdit {
        occurrence: Some(engine_api::LocalDateTime::new(2026, 1, 12, 9, 30, 0).unwrap()),
        ..moves_it()
    };
    assert_eq!(
        warning_for(
            CalendarFake::losing_overrides(vec![series_with_an_override()]),
            edit,
        )
        .await,
        None
    );
}
