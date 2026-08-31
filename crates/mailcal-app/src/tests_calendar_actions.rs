//! Calendar write-action tests for [`super::App`]: routing a delete to the owning account on a
//! key collision, the revision guard a delete carries, the reconcile a write runs (and the full
//! refresh it must *not*), the provider-neutral patch an edit sends, and the `CalendarWriteStatus`
//! a create/edit/delete drives (saved, failed-not-swallowed, cleared by a refresh). Split out of
//! `tests_actions.rs` to keep each file under the 500-line limit. Fixtures live in
//! `tests_fakes.rs`.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_provider::PatchTarget;
use fakes::{CalendarFake, calendar_account, calendar_app, evt};

use super::{CalendarWriteStatus, Intent, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn an_event_action_routes_by_owning_account_on_a_key_collision() {
    // The calendar counterpart of the message-collision test: two accounts whose stores
    // BOTH hold an event with the SAME key ("shared-event"). Deleting it FOR ACCOUNT B
    // must reach only B's calendar provider: the old all-account event scan returned the
    // first match (account A) and would have misrouted B's delete.
    let a = CalendarFake::with_event("shared-event");
    let b = CalendarFake::with_event("shared-event");
    let a_deletions = a.deletions();
    let b_deletions = b.deletions();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account("acct-a", a), calendar_account("acct-b", b)],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-b", "shared-event"),
        occurrence: None,
    })
    .await;
    assert_eq!(
        b_deletions.lock().unwrap().as_slice(),
        ["shared-event"],
        "B's calendar provider got the delete"
    );
    assert!(
        a_deletions.lock().unwrap().is_empty(),
        "A's calendar provider was untouched"
    );
}

#[tokio::test]
async fn a_delete_is_guarded_by_the_revision_the_event_was_read_at() {
    // `delete_event` builds `EventDeletion::of(&stored)`, which conditions the delete on the
    // revision the caller read (CalDAV's `If-Match`) so it cannot silently discard a newer edit
    // made server-side. An unconditional delete (`guard: None`) would be the regression.
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let guards = provider.delete_guards();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-a", "standup"),
        occurrence: None,
    })
    .await;

    let guards = guards.lock().unwrap();
    assert_eq!(guards.len(), 1, "exactly one delete reached the provider");
    let guard = guards[0]
        .as_ref()
        .expect("the delete is guarded, not unconditional");
    assert!(
        !guard.is_empty(),
        "the guard carries the revision the event was read at"
    );
}

#[tokio::test]
async fn a_write_reconciles_the_event_scope_without_a_full_refresh() {
    // A write's own reconcile is a single event-scope delta; it must not fall back on the full
    // `refresh_calendar` (a calendars + events sync, plus a re-expand, two provider round-trips
    // per account). Counting the provider's syncs proves the write took the cheap path.
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let syncs = provider.syncs();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let before = syncs.load(Ordering::Relaxed);
    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;
    let after = syncs.load(Ordering::Relaxed);

    assert_eq!(
        after - before,
        1,
        "a create costs exactly one reconcile delta, not a full refresh"
    );
}

#[tokio::test]
async fn editing_an_event_sends_a_patch_with_only_the_changed_properties() {
    // The whole point of the new write API: an edit is a provider-neutral patch, not a
    // rebuilt six-property document. The provider receives the intent and applies it to the
    // stored payload, so the recurrence rule, alarm and X- property survive.
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "standup"),
        edit: mailcal_account::EventEdit {
            title: Some("Standup (kort)".to_owned()),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    let sent = patches.lock().unwrap();
    let patch = sent.first().expect("the provider received a patch");
    // Only the title changed, and the target is the whole series, asserted by value, not by
    // grepping a `Debug` string.
    assert_eq!(patch.target, PatchTarget::Series);
    assert_eq!(patch.summary.as_deref(), Some("Standup (kort)"));
    assert_eq!(patch.start, None, "an untouched time is not re-sent");
    assert_eq!(patch.end, None);
}

#[tokio::test]
async fn a_rejected_edit_is_reported_as_failed_not_swallowed() {
    // There is no outbox drainer behind this write: if the patch fails, the user's edit did not
    // happen. `create_event` discards that with `let _ =`; an edit must not, or the app will
    // cheerfully show a change the server never accepted.
    let provider = CalendarFake::rejecting_writes(vec![fakes::stored_event("standup", "\"v7\"")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let outcome = app
        .update_event(
            &evt("acct-a", "standup"),
            &mailcal_account::EventEdit {
                title: Some("Standup (kort)".to_owned()),
                ..mailcal_account::EventEdit::default()
            },
        )
        .await;
    assert!(outcome.is_err(), "a rejected patch was reported as a save");
    // And the calendar status says so, so the host can show the warning rather than a check.
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn a_create_reports_saved_and_signals_the_calendar_status() {
    // The user needs to see that their create took. A successful create settles to `Saved`
    // and signals `Surface::CalendarStatus` so the host can pull the new state and paint it.
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    surfaces.lock().unwrap().clear();

    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Saved);
    assert!(
        surfaces.lock().unwrap().contains(&Surface::CalendarStatus),
        "the host was told the calendar write status changed"
    );
}

#[tokio::test]
async fn a_write_whose_reconcile_fails_reports_failed_not_saved() {
    // The write landed on the server, but the post-write reconcile (an event delta) failed, so
    // the local view is unconfirmed. The status must be `Failed` (the warning icon) not a
    // confident `Saved`. The core must NOT re-issue the write to "fix" it (that would write
    // twice); it only re-reads, which here also fails, leaving the status honest.
    let provider = CalendarFake::failing_reconcile(vec![fakes::stored_event("standup", "\"v7\"")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn a_background_refresh_preserves_failure_until_an_explicit_retry() {
    // A full refresh reconciles the whole event scope, so it heals what a `Failed` write status
    // stood for, and the terminal status is per-write feedback, not a persistent health light.
    // So the host's "retry" (a RefreshCalendar) clears the warning back to `Idle`.
    let provider = CalendarFake::failing_reconcile(vec![fakes::stored_event("standup", "\"v7\"")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);

    app.refresh_calendar_in_background().await;
    assert_eq!(
        app.calendar_write_status(),
        CalendarWriteStatus::Failed,
        "an unasked refresh must not dismiss the user's failed-write warning"
    );

    app.dispatch(Intent::RefreshCalendar).await;
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Idle);
}

#[tokio::test]
async fn a_create_routes_to_a_writable_account_not_the_first_calendar_account() {
    // A client enables its create affordance when ANY calendar reports `can_write`, so the
    // routed write must agree with that promise: with a read-only subscription listed first,
    // the create goes to the writable account. The old "first calendar-capable account" rule
    // would hand it to the subscription: a write the UI just said was possible, failing.
    let reader = CalendarFake::read_only(Vec::new());
    let writer = CalendarFake::with_event("editable");
    let reader_creations = reader.creations();
    let writer_creations = writer.creations();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![
            calendar_account("reader", reader),
            calendar_account("writer", writer),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(
        writer_creations.lock().unwrap().len(),
        1,
        "the writable account's provider got the create"
    );
    assert!(
        reader_creations.lock().unwrap().is_empty(),
        "the read-only account's provider was never asked to write"
    );
}

#[tokio::test]
async fn a_create_is_a_no_op_when_no_account_can_write() {
    // Every calendar row reports `can_write: false`, so every client's create affordance is
    // disabled: a create that arrives anyway (a future host over the same commands) must be
    // the documented no-op, not an attempted write the provider is guaranteed to refuse.
    let reader = CalendarFake::read_only(Vec::new());
    let creations = reader.creations();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("reader", reader)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert!(
        creations.lock().unwrap().is_empty(),
        "no write was attempted against the read-only provider"
    );
    assert_eq!(
        app.calendar_write_status(),
        CalendarWriteStatus::Idle,
        "a no-op create reports nothing; there was no write to report on"
    );
}

#[tokio::test]
async fn a_create_routes_to_the_chosen_calendar_not_the_first() {
    // The calendar picker's whole point: a create carrying a calendar key lands in *that*
    // calendar. The old create always used `calendars.first()`, so a user with a "work" and a
    // "personal" calendar could not put an event anywhere but the first.
    let provider = CalendarFake::with_calendars(&["work", "personal"]);
    let targets = provider.create_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "1:1".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct-a".to_owned()),
        calendar: Some("personal".to_owned()),
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(
        targets.lock().unwrap().as_slice(),
        ["personal"],
        "the create landed in the chosen calendar, not the first"
    );
}

#[tokio::test]
async fn a_create_with_an_unknown_calendar_still_lands_rather_than_dropping() {
    // A stale picker choice (a calendar since removed) must fall back to a real calendar of
    // the account rather than silently dropping the create. (Which one is the "first" is the
    // store's ordering, not ours to assert, that it landed at all is the contract.)
    let provider = CalendarFake::with_calendars(&["work", "personal"]);
    let targets = provider.create_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "1:1".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct-a".to_owned()),
        calendar: Some("archived".to_owned()),
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    let targets = targets.lock().unwrap();
    assert_eq!(targets.len(), 1, "the create still landed, not dropped");
    assert!(
        ["work", "personal"].contains(&targets[0].as_str()),
        "it fell back to a real calendar of the account, was {:?}",
        targets[0]
    );
}

#[tokio::test]
async fn a_create_routes_to_the_chosen_account() {
    // Two writable accounts. The create names the *second*; it must go there, not to the
    // default (first-writable) account the no-argument create would pick.
    let a = CalendarFake::with_event("a-evt");
    let b = CalendarFake::with_calendars(&["b-cal"]);
    let a_creations = a.creations();
    let b_targets = b.create_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account("acct-a", a), calendar_account("acct-b", b)],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "1:1".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct-b".to_owned()),
        calendar: Some("b-cal".to_owned()),
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(b_targets.lock().unwrap().as_slice(), ["b-cal"]);
    assert!(
        a_creations.lock().unwrap().is_empty(),
        "the unchosen account's provider was never asked to write"
    );
}
