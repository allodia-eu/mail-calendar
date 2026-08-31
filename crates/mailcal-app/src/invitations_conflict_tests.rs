//! Tests for the **diary** rules, which of the user's existing commitments genuinely clash with
//! a meeting, and how a stored event's own answer is read. The card rules are in
//! [`crate::invitations_tests`].

use engine_api::{CalendarDate, ParticipantRole, ParticipationStatus};
use mailcal_viewmodel::{InvitationKind, ResponseStatus};

use crate::{
    invitations::{DiaryEntry, count_conflicts, diary_participation, proposed_hold},
    invitations_build::Conflicts,
    invitations_test_support::{attendee, event_with, instant, organizer},
};

// --- the conflict window --------------------------------------------------------------

fn diary_entry(uid: &str, start: &str, end: &str, response: ResponseStatus) -> DiaryEntry {
    DiaryEntry {
        uid: uid.to_owned(),
        start: instant(start),
        end: instant(end),
        my_response: response,
    }
}

#[test]
fn the_invitations_own_tentative_hold_never_counts_as_a_conflict() {
    // Every server in use auto-schedules, so the hold is already on the calendar when the mail is
    // read. Counting it would report *every* invitation as clashing with itself, which trains the
    // user to ignore the number.
    let diary = vec![diary_entry(
        "meeting-1@test.local",
        "2026-07-30T14:30:00Z",
        "2026-07-30T15:30:00Z",
        ResponseStatus::NeedsAction,
    )];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        0
    );
}

#[test]
fn another_unanswered_hold_is_not_yet_a_conflict() {
    // Not a guess about intent: an Outlook organiser says it explicitly, sending
    // X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE alongside INTENDEDSTATUS:BUSY; hold it tentatively
    // until answered, busy once accepted.
    let diary = vec![diary_entry(
        "other@test.local",
        "2026-07-30T14:00:00Z",
        "2026-07-30T15:00:00Z",
        ResponseStatus::NeedsAction,
    )];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        0
    );
}

#[test]
fn a_declined_event_is_not_a_commitment() {
    // The user said no, and `docs/calendar.md` hides it from the grid: so counting it would
    // contradict what is on screen.
    let diary = vec![diary_entry(
        "declined@test.local",
        "2026-07-30T14:00:00Z",
        "2026-07-30T15:00:00Z",
        ResponseStatus::Declined,
    )];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        0
    );
}

#[test]
fn accepted_and_tentative_commitments_do_conflict() {
    let diary = vec![
        diary_entry(
            "accepted@test.local",
            "2026-07-30T14:00:00Z",
            "2026-07-30T15:00:00Z",
            ResponseStatus::Accepted,
        ),
        diary_entry(
            "tentative@test.local",
            "2026-07-30T15:00:00Z",
            "2026-07-30T16:00:00Z",
            ResponseStatus::Tentative,
        ),
    ];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        2,
        "a tentatively-accepted meeting is still a commitment the user made"
    );
}

#[test]
fn back_to_back_meetings_do_not_conflict() {
    // Half-open on both sides. Back-to-back is the normal way a diary is packed; flagging it
    // would make the number useless.
    let diary = vec![
        diary_entry(
            "before@test.local",
            "2026-07-30T13:30:00Z",
            "2026-07-30T14:30:00Z",
            ResponseStatus::Accepted,
        ),
        diary_entry(
            "after@test.local",
            "2026-07-30T15:30:00Z",
            "2026-07-30T16:30:00Z",
            ResponseStatus::Accepted,
        ),
    ];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        0
    );
}

#[test]
fn an_all_day_commitment_that_straddles_the_meeting_conflicts() {
    let diary = vec![diary_entry(
        "offsite@test.local",
        "2026-07-30T00:00:00Z",
        "2026-07-31T00:00:00Z",
        ResponseStatus::Accepted,
    )];
    assert_eq!(
        count_conflicts(
            "meeting-1@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        1
    );
}

// --- the stored-event answer, which drives both the grid and the conflict rule ---------

#[test]
fn a_users_own_appointment_is_a_commitment_not_an_unanswered_hold() {
    // An event with no attendees is something the user put in their own diary. Reading it as
    // "unanswered" would draw it dotted on the grid and let the conflict rule skip it: so a
    // meeting invitation would claim the slot was free.
    let own = event_with(vec![]);
    assert_eq!(
        diary_participation(&own, &["me@test.local".to_owned()]),
        ResponseStatus::Accepted
    );
}

#[test]
fn a_meeting_on_our_calendar_that_we_are_not_an_attendee_of_is_still_ours_to_keep() {
    // A room booking, or a colleague's event on a shared calendar: it has attendees, none of them
    // us. It is on our calendar, so it is a commitment: not an invitation awaiting our answer.
    let theirs = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("someone@test.local", ParticipationStatus::Accepted),
    ]);
    assert_eq!(
        diary_participation(&theirs, &["me@test.local".to_owned()]),
        ResponseStatus::Accepted
    );
}

#[test]
fn our_own_partstat_is_what_the_grid_draws() {
    let mine = vec!["me@test.local".to_owned()];
    for (status, expected) in [
        (
            ParticipationStatus::NeedsAction,
            ResponseStatus::NeedsAction,
        ),
        (ParticipationStatus::Accepted, ResponseStatus::Accepted),
        (ParticipationStatus::Tentative, ResponseStatus::Tentative),
        (ParticipationStatus::Declined, ResponseStatus::Declined),
    ] {
        let event = event_with(vec![
            organizer("Boss", "boss@test.local"),
            attendee("me@test.local", status.clone()),
        ]);
        assert_eq!(
            diary_participation(&event, &mine),
            expected,
            "PARTSTAT {status:?} must read as {expected:?}"
        );
    }
}

#[test]
fn an_alias_invitation_on_the_calendar_is_recognized_as_ours() {
    // The grid can only use the *persisted* address set; it has no message to read delivery
    // headers from: so the alias list is what makes an alias-addressed hold render as a hold
    // rather than as somebody else's settled meeting.
    let event = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("info@example.com", ParticipationStatus::NeedsAction),
    ]);
    assert_eq!(
        diary_participation(&event, &["alice@example.com".to_owned()]),
        ResponseStatus::Accepted,
        "without the alias it looks like somebody else's meeting"
    );
    assert_eq!(
        diary_participation(
            &event,
            &[
                "alice@example.com".to_owned(),
                "info@example.com".to_owned()
            ],
        ),
        ResponseStatus::NeedsAction,
        "with the alias configured it is correctly an unanswered hold"
    );
}

#[test]
fn a_meeting_we_called_is_a_commitment_even_with_no_partstat_of_our_own() {
    // Observed on a live CalDAV account (Soverin, 0.13.0): creating a meeting writes us in as an
    // `ATTENDEE` with no `PARTSTAT`, which RFC 5545 defaults to NEEDS-ACTION: so our own meeting
    // was drawn dotted on the grid, as though we had been invited to it and never replied.
    let merged = event_with(vec![
        attendee("guest@test.local", ParticipationStatus::Accepted),
        // The JSCalendar shape: one participant carrying both roles.
        {
            let mut me = attendee("me@test.local", ParticipationStatus::NeedsAction);
            me.roles.insert(ParticipantRole::Owner);
            me
        },
    ]);
    assert_eq!(
        diary_participation(&merged, &["me@test.local".to_owned()]),
        ResponseStatus::Accepted,
        "the person who called the meeting has not failed to reply to it"
    );

    // The split shape: a separate ORGANIZER line and an ATTENDEE line at the same address. Reading
    // only the matched attendee would miss this one and leave it dotted.
    let split = event_with(vec![
        organizer("Me", "me@test.local"),
        attendee("me@test.local", ParticipationStatus::NeedsAction),
        attendee("guest@test.local", ParticipationStatus::NeedsAction),
    ]);
    assert_eq!(
        diary_participation(&split, &["me@test.local".to_owned()]),
        ResponseStatus::Accepted
    );

    // And through an alias, since the grid matches against the whole persisted address set.
    let via_alias = event_with(vec![{
        let mut me = attendee("info@example.com", ParticipationStatus::NeedsAction);
        me.roles.insert(ParticipantRole::Owner);
        me
    }]);
    assert_eq!(
        diary_participation(
            &via_alias,
            &[
                "alice@example.com".to_owned(),
                "info@example.com".to_owned()
            ],
        ),
        ResponseStatus::Accepted
    );
}

#[test]
fn an_organizer_who_answered_keeps_their_own_answer() {
    // Only the *absent* answer is inferred: the same line `tally` draws. An organiser who declines
    // their own meeting means it, and `docs/calendar.md` §4 then hides it from every surface;
    // overriding that to Accepted would put a meeting the user cancelled on back on their grid.
    for (status, expected) in [
        (ParticipationStatus::Declined, ResponseStatus::Declined),
        (ParticipationStatus::Tentative, ResponseStatus::Tentative),
    ] {
        let event = event_with(vec![{
            let mut me = attendee("me@test.local", status.clone());
            me.roles.insert(ParticipantRole::Owner);
            me
        }]);
        assert_eq!(
            diary_participation(&event, &["me@test.local".to_owned()]),
            expected,
            "an organizer's explicit {status:?} is an answer, not an absence"
        );
    }
}

#[test]
fn somebody_elses_meeting_is_still_an_unanswered_hold() {
    // The guard against over-correcting: the fix keys on *our* address carrying the Owner role, so
    // a meeting someone else called must be untouched by it: this is the case the dotted border
    // exists for, and losing it would hide every real invitation on the grid.
    let theirs = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("me@test.local", ParticipationStatus::NeedsAction),
    ]);
    assert_eq!(
        diary_participation(&theirs, &["me@test.local".to_owned()]),
        ResponseStatus::NeedsAction
    );
}

#[test]
fn a_meeting_we_called_counts_against_a_new_invitation() {
    // The half of the bug that is invisible until it costs you: `count_conflicts` skips
    // NEEDS-ACTION, so while our own meetings read as unanswered the card said "Nothing else in
    // your calendar then" over a slot we had already committed to.
    let ours = event_with(vec![{
        let mut me = attendee("me@test.local", ParticipationStatus::NeedsAction);
        me.roles.insert(ParticipantRole::Owner);
        me
    }]);
    let diary = vec![diary_entry(
        "ours@test.local",
        "2026-07-30T14:00:00Z",
        "2026-07-30T15:00:00Z",
        diary_participation(&ours, &["me@test.local".to_owned()]),
    )];
    assert_eq!(
        count_conflicts(
            "invitation@test.local",
            instant("2026-07-30T14:30:00Z"),
            instant("2026-07-30T15:30:00Z"),
            &diary,
        ),
        1,
        "a meeting we called clashes with a new invitation over it"
    );
}

// --- zero is not the same answer as "we have not looked" --------------------------------

#[test]
fn an_unread_calendar_is_unknown_rather_than_zero() {
    // The regression: every failure path in `conflicts_for` used to return `0`, and a client
    // renders `0` as "Nothing else in your calendar then". Opening an invitation before the
    // first calendar sync, which happens on every cold start, because mail syncs first;
    // therefore stated that a day was free while it held two clashes. `docs/calendar.md` §4
    // forbids exactly that: the same reason a grid page says "loading" instead of drawing a
    // confidently empty week.
    assert!(!Conflicts::Unknown.is_known());
    assert_eq!(
        Conflicts::Unknown.count(),
        0,
        "the count degrades to zero, which is why it may never be read without is_known()"
    );
    assert!(Conflicts::Known(0).is_known());
    assert_eq!(Conflicts::Known(2).count(), 2);
    // The two zeroes are distinguishable, which is the whole point of the type.
    assert_ne!(Conflicts::Known(0), Conflicts::Unknown);
}

#[test]
fn a_day_outside_the_materialized_window_is_not_covered() {
    // `calendar_covers` is `covers` over the calendar cache's window, and `None` (no sync yet) is
    // the state a cold start is in. It must read as "not covered", never as an empty day.
    assert!(
        !crate::calendar_cache::covers(None, &[CalendarDate::new(2026, 7, 27).unwrap()]),
        "before the first calendar sync nothing is covered"
    );
}

// --- the meeting the card is about, when no calendar holds it -------------------------------

#[test]
fn a_meeting_no_calendar_holds_is_drawn_on_the_preview() {
    // The reported bug. Where nothing files an invitation into the calendar: a bare mailbox, an
    // IMAP+CalDAV account with no bridge from the mail store; "Around this meeting" drew every
    // block except the meeting it is about, so the same invitation showed a different picture
    // depending on the server behind it.
    assert!(proposed_hold(
        InvitationKind::Rsvp,
        None,
        ResponseStatus::NeedsAction
    ));
}

#[test]
fn a_meeting_the_calendar_already_holds_is_not_drawn_twice() {
    // An auto-scheduling server files it, and the stored copy is drawn from the diary read like
    // any other block. Adding a second would double every invitation on the servers that work.
    let stored = event_with(vec![]);
    assert!(!proposed_hold(
        InvitationKind::Rsvp,
        Some(&stored),
        ResponseStatus::NeedsAction
    ));
}

#[test]
fn a_forwarded_invitation_still_shows_where_it_would_land() {
    // No reply is owed, but the question the picture answers; "where would this sit in my
    // day", is the reader's next one either way.
    assert!(proposed_hold(
        InvitationKind::Informational,
        None,
        ResponseStatus::NeedsAction
    ));
}

#[test]
fn a_cancelled_meeting_gets_no_hold() {
    // The meeting is off. Drawing a block for it invents a commitment the reader then has to
    // disprove: the opposite of what the card above it says.
    assert!(!proposed_hold(
        InvitationKind::Cancelled,
        None,
        ResponseStatus::NeedsAction
    ));
    assert!(!proposed_hold(
        InvitationKind::Superseded,
        None,
        ResponseStatus::NeedsAction
    ));
}

#[test]
fn a_meeting_the_user_declined_is_hidden_here_too() {
    // Same rule the diary read applies one function up, and the same reason `docs/calendar.md`
    // hides a declined event from the grid: the user said no, so it is not a commitment.
    assert!(!proposed_hold(
        InvitationKind::Rsvp,
        None,
        ResponseStatus::Declined
    ));
    // An answer the mail itself already carries is drawn as that answer, not as an unanswered
    // hold: the block and the card's own response line cannot then contradict each other.
    assert!(proposed_hold(
        InvitationKind::Rsvp,
        None,
        ResponseStatus::Accepted
    ));
}
