//! Tests for the **card** rules: the RSVP gate, alias matching, the attendee tally, and the
//! untrusted-text sanitiser. The diary rules (conflicts, and how a stored event's own answer is
//! read) are in [`crate::invitations_conflict_tests`].
//!
//! These are the contract of `docs/invitations.md`. They are unit tests over pure functions
//! precisely so the contract is a check that can fail: an eligibility rule that could only be
//! exercised through a live provider and an open message would be a rule nothing verifies.

use engine_api::{Participant, ParticipantRole, ParticipationStatus, ScheduleMethod};
use mailcal_viewmodel::{InvitationKind, ResponseStatus};

use crate::{
    invitations::{
        classify, description, location, matched_attendee, my_response, organizer_line, summary,
        supersede, tally,
    },
    invitations_test_support::{attendee, event_with, organizer},
};

// --- the two-condition RSVP gate ------------------------------------------------------

#[test]
fn the_rsvp_gate_is_a_table_and_both_conditions_are_load_bearing() {
    // Every case that matters, stated once. `PUBLISH` is the row this table exists for: it is
    // rejected *even when we are listed as an attendee*, because no organiser is awaiting a
    // reply (RFC 5546 §1.4), which is why attendee-matching alone cannot be the rule.
    let cases: &[(ScheduleMethod, bool, Option<InvitationKind>, &str)] = &[
        (
            ScheduleMethod::Request,
            true,
            Some(InvitationKind::Rsvp),
            "a request addressed to us is the whole point",
        ),
        (
            ScheduleMethod::Request,
            false,
            Some(InvitationKind::Informational),
            "somebody else's meeting: show it, offer no reply",
        ),
        (
            ScheduleMethod::Publish,
            true,
            None,
            "a PUBLISH listing us is still informational: no card, the .ics keeps its chip",
        ),
        (
            ScheduleMethod::Publish,
            false,
            None,
            "a published .ics newsletter is not an invitation",
        ),
        (
            ScheduleMethod::Cancel,
            true,
            Some(InvitationKind::Cancelled),
            "our meeting was cancelled: say so, and offer to clear the hold",
        ),
        (
            ScheduleMethod::Cancel,
            false,
            Some(InvitationKind::Informational),
            "a cancellation for a meeting that was never ours",
        ),
        (
            ScheduleMethod::Reply,
            true,
            None,
            "an attendee answering us needs its own UI, not an invitation card",
        ),
        (
            ScheduleMethod::Counter,
            true,
            None,
            "propose-new-time is not in v1 (a known gap)",
        ),
        (ScheduleMethod::Add, true, None, "ADD needs its own UI"),
        (
            ScheduleMethod::Refresh,
            true,
            None,
            "REFRESH is a request for data, not something to show",
        ),
        (
            ScheduleMethod::DeclineCounter,
            true,
            None,
            "DECLINECOUNTER needs its own UI",
        ),
        (
            ScheduleMethod::Other("x-vendor".to_owned()),
            true,
            None,
            "an unknown method must not be guessed at",
        ),
    ];

    for (method, mine, expected, why) in cases {
        assert_eq!(
            classify(method, *mine),
            *expected,
            "METHOD={method:?} attendee={mine}: {why}"
        );
    }
}

// --- supersession (RFC 5546 §2.1.5) ---------------------------------------------------

/// The stored event a calendar would hold for this meeting at `sequence`.
fn stored_at(sequence: u32) -> engine_api::Event {
    let mut event = event_with(vec![attendee(
        "me@test.local",
        ParticipationStatus::NeedsAction,
    )]);
    event.sequence = sequence;
    event
}

#[test]
fn a_lower_sequence_than_the_calendar_holds_is_superseded() {
    // The captured pair, exactly: an organiser shortened the meeting and Exchange re-sent it as
    // SEQUENCE 1, leaving SEQUENCE 0 in the mailbox still offering buttons over the old times.
    assert_eq!(
        supersede(InvitationKind::Rsvp, 0, Some(&stored_at(1))),
        InvitationKind::Superseded
    );
}

#[test]
fn the_current_revision_stays_answerable() {
    // The newer of the two captures. Equal sequences are *not* supersession: claiming it would
    // hide the buttons on the very invitation the organiser is waiting on.
    assert_eq!(
        supersede(InvitationKind::Rsvp, 1, Some(&stored_at(1))),
        InvitationKind::Rsvp
    );
}

#[test]
fn a_mail_ahead_of_the_calendar_is_not_superseded() {
    // The update has arrived by mail but the calendar has not caught up. The mail is the *newer*
    // fact here, so it keeps its buttons; supersession is only ever "the calendar knows better".
    assert_eq!(
        supersede(InvitationKind::Rsvp, 2, Some(&stored_at(1))),
        InvitationKind::Rsvp
    );
}

#[test]
fn no_stored_event_is_not_evidence_that_the_mail_is_stale() {
    // The load-bearing case for a mailbox whose calendar never receives the invitation (an IMAP
    // account whose CalDAV server has no inbound iMIP bridge): there is nothing to compare
    // against, which is "we have not looked", never "this is current" *or* "this is stale". The
    // same distinction `conflicts_known` draws: an empty answer must not look like a real one.
    assert_eq!(
        supersede(InvitationKind::Rsvp, 0, None),
        InvitationKind::Rsvp
    );
}

#[test]
fn only_an_answerable_invitation_is_downgraded() {
    // A cancellation outranked by the calendar keeps saying "cancelled": that the meeting is off
    // matters more to the reader than the age of the mail carrying the news. Informational offers
    // no buttons to withdraw in the first place.
    for kind in [InvitationKind::Cancelled, InvitationKind::Informational] {
        assert_eq!(
            supersede(kind, 0, Some(&stored_at(9))),
            kind,
            "{kind:?} must pass through untouched"
        );
    }
}

// --- alias matching (D5) --------------------------------------------------------------

#[test]
fn an_attendee_matching_an_alias_is_me_and_the_matched_address_comes_back() {
    // The reported case: primary is alice@, the invitation names info@. Returning the *matched*
    // address is load-bearing: the CalDAV RSVP primitive patches the PARTSTAT of a named
    // ATTENDEE line, so handing it the primary would find no line and the RSVP would fail (D4).
    let event = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("info@example.com", ParticipationStatus::NeedsAction),
    ]);
    let addresses = vec![
        "alice@example.com".to_owned(),
        "info@example.com".to_owned(),
    ];
    assert_eq!(
        matched_attendee(&event, &addresses).as_deref(),
        Some("info@example.com")
    );
}

#[test]
fn matching_ignores_case_and_the_mailto_scheme() {
    // iCalendar writes `mailto:` inconsistently and cases domains freely. A missed match here
    // silently means "you are not invited to this", which hides the RSVP the user is waiting for.
    let event = event_with(vec![attendee(
        "mailto:Info@Test.LOCAL",
        ParticipationStatus::NeedsAction,
    )]);
    assert!(matched_attendee(&event, &["info@test.local".to_owned()]).is_some());
}

#[test]
fn an_attendee_who_is_not_us_does_not_match() {
    let event = event_with(vec![attendee(
        "someone@test.local",
        ParticipationStatus::NeedsAction,
    )]);
    assert!(matched_attendee(&event, &["me@test.local".to_owned()]).is_none());
    // A near-miss on the local part must not match either.
    let similar = event_with(vec![attendee(
        "me2@test.local",
        ParticipationStatus::Accepted,
    )]);
    assert!(matched_attendee(&similar, &["me@test.local".to_owned()]).is_none());
}

#[test]
fn my_response_reads_the_matched_attendees_partstat() {
    let event = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("me@test.local", ParticipationStatus::Tentative),
    ]);
    let mine = vec!["me@test.local".to_owned()];
    assert_eq!(my_response(&event, &mine), ResponseStatus::Tentative);

    // Not an attendee at all, and an attendee with an unknown vendor status, both read as
    // unanswered: the honest and the safe reading, since it offers the buttons rather than
    // claiming an answer the user never gave.
    assert_eq!(
        my_response(&event, &["other@test.local".to_owned()]),
        ResponseStatus::NeedsAction
    );
    let vendor = event_with(vec![attendee(
        "me@test.local",
        ParticipationStatus::Other("snoozed".to_owned()),
    )]);
    assert_eq!(my_response(&vendor, &mine), ResponseStatus::NeedsAction);
}

// --- the attendee tally ---------------------------------------------------------------

#[test]
fn the_tally_counts_every_bucket_and_totals_to_their_sum() {
    let event = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("a@test.local", ParticipationStatus::Accepted),
        attendee("b@test.local", ParticipationStatus::Declined),
        attendee("c@test.local", ParticipationStatus::Tentative),
        attendee("d@test.local", ParticipationStatus::NeedsAction),
        attendee("e@test.local", ParticipationStatus::Delegated),
        // An ATTENDEE line with no cal-address cannot be anybody; counting it would inflate
        // "of N" with an entry the user can never see.
        Participant {
            email: None,
            ..Participant::attendee("ignored@test.local")
        },
    ]);
    let tally = tally(&event);
    assert_eq!(
        tally.total, 6,
        "the organizer counts, the address-less does not"
    );
    assert_eq!(tally.accepted, 2, "the organizer is ACCEPTED, plus a@");
    assert_eq!(tally.declined, 1);
    assert_eq!(tally.tentative, 1);
    assert_eq!(
        tally.needs_action, 2,
        "delegated groups with unanswered so the buckets sum to total"
    );
    assert_eq!(
        tally.accepted + tally.declined + tally.tentative + tally.needs_action,
        tally.total
    );
}

#[test]
fn an_organizer_who_never_said_is_attending_their_own_meeting() {
    // Found by `gmail-request.eml`: Google emits an `ORGANIZER` line and no matching
    // `ATTENDEE`, so with iCalendar's default `PARTSTAT` the sender of a two-person
    // invitation was reported as not having replied to it; "0 accepted · 2 awaiting".
    // Stalwart lists itself `PARTSTAT=ACCEPTED` and read correctly, so this is invisible
    // until you hold two real senders side by side.
    let event = event_with(vec![
        Participant {
            participation_status: ParticipationStatus::NeedsAction,
            ..organizer("Boss", "boss@test.local")
        },
        attendee("me@test.local", ParticipationStatus::NeedsAction),
    ]);
    let tally = tally(&event);
    assert_eq!(
        tally.accepted, 1,
        "the organizer is attending by definition"
    );
    assert_eq!(tally.needs_action, 1, "only the invitee is still to answer");
    assert_eq!(tally.total, 2);
}

#[test]
fn an_organizer_who_did_say_keeps_their_answer() {
    // Only the *absent* answer is inferred. An organiser who declined their own meeting has
    // told us something, and overwriting it would be the same silent lie in reverse.
    let event = event_with(vec![
        Participant {
            participation_status: ParticipationStatus::Declined,
            ..organizer("Boss", "boss@test.local")
        },
        attendee("me@test.local", ParticipationStatus::Accepted),
    ]);
    let tally = tally(&event);
    assert_eq!(tally.declined, 1, "the organizer's own decline stands");
    assert_eq!(tally.accepted, 1);
}

// --- untrusted text -------------------------------------------------------------------
//
// `plain_text` itself is tested where it lives, in `mailcal_viewmodel::text`: the calendar's
// attendee list sanitises through it too, so it is no longer an invitation-only rule. What stays
// here is how *this* card applies it.

#[test]
fn a_gmail_style_filler_description_is_cut_and_says_so() {
    let mut event = event_with(vec![]);
    event.description = Some("-::~:~::~:~:~:~:~:~:~:~:~::~:~::-".repeat(80));
    let (text, truncated) = description(&event);
    assert!(
        truncated,
        "a wall of filler must not push the body off screen"
    );
    assert_eq!(text.chars().count(), 500);
}

#[test]
fn an_empty_location_reads_as_no_location() {
    // Sabre/CalDAV invitations frequently carry an empty LOCATION; a blank one must read as
    // "no location", not as a location that happens to be blank.
    let mut event = event_with(vec![]);
    event.locations = vec![engine_api::Location::named("   ")];
    assert_eq!(location(&event), "");

    event.locations = vec![engine_api::Location::named("Amsterdam HQ")];
    assert_eq!(location(&event), "Amsterdam HQ");
}

#[test]
fn the_organizer_line_prefers_the_owner_and_normalizes_the_address() {
    let event = event_with(vec![
        attendee("me@test.local", ParticipationStatus::NeedsAction),
        organizer("The Boss", "mailto:Boss@Test.local"),
    ]);
    assert_eq!(organizer_line(&event), "The Boss <boss@test.local>");

    // No name → bare address; no organiser at all → empty, so the *client* supplies the
    // "unknown organiser" wording (localisation is client-side).
    let unnamed = event_with(vec![{
        let mut p = Participant::attendee("boss@test.local");
        p.roles.insert(ParticipantRole::Owner);
        p
    }]);
    assert_eq!(organizer_line(&unnamed), "boss@test.local");
    assert_eq!(organizer_line(&event_with(vec![])), "");
}

#[test]
fn the_organizer_is_the_owner_even_when_a_chair_is_listed_first() {
    // `ROLE=CHAIR` is an ordinary attendee role, and iCalendar fixes no property order: so a
    // single "Owner or Chair" search names whoever the sender happened to write first. Naming the
    // wrong person as the organiser is the kind of wrong the user cannot detect.
    let event = event_with(vec![
        {
            let mut chair = Participant::attendee("chair@test.local");
            chair.roles.insert(ParticipantRole::Chair);
            chair
        },
        organizer("The Boss", "boss@test.local"),
    ]);
    assert_eq!(organizer_line(&event), "The Boss <boss@test.local>");
}

#[test]
fn the_organizers_address_is_sanitized_like_every_other_untrusted_field() {
    // The address half is as attacker-controlled as the name half: the engine's
    // `normalize_address` only lowercases and strips `mailto:`, so a bidi override in an
    // `ORGANIZER` would otherwise reach a native label and reverse the line it sits on, and an
    // unbounded one would be an unbounded label.
    let event = event_with(vec![organizer(
        "Boss",
        "mailto:bo\u{202E}ss@test.local\u{200B}",
    )]);
    let line = organizer_line(&event);
    assert!(
        !line.contains('\u{202E}') && !line.contains('\u{200B}'),
        "got {line}"
    );

    let long = event_with(vec![{
        let mut p = Participant::attendee(format!("{}@test.local", "a".repeat(500)));
        p.roles.insert(ParticipantRole::Owner);
        p
    }]);
    assert!(organizer_line(&long).chars().count() <= 200);
}

#[test]
fn a_blank_summary_is_left_empty_for_the_client_to_label() {
    let event = event_with(vec![]);
    assert_eq!(summary(&event), "", "the core invents no localized text");
}

/// The routing table, as a table. Every row is an account shape that exists in the wild, and
/// the pair of capabilities behind it is `(calendar_rsvp.is_some(), calendar_scheduling,
/// scheduling_submission)`.
#[test]
fn each_account_shape_routes_its_answer_the_only_way_it_can() {
    use crate::invitations::{Delivery, delivery};

    let cases: &[(bool, bool, bool, Delivery, &str)] = &[
        (
            true,
            true,
            true,
            Delivery::Server,
            "Graph and Google: the server schedules, so sending our own reply would tell the \
             organizer twice",
        ),
        (
            true,
            true,
            false,
            Delivery::Server,
            "JMAP, and CalDAV auto-schedule beside a JMAP mailbox: the server still schedules",
        ),
        (
            true,
            false,
            true,
            Delivery::ClientImip,
            "the reported bug; IMAP mail beside a plain RFC 4791 calendar",
        ),
        (
            true,
            false,
            false,
            Delivery::None,
            "a plain calendar beside a mailbox that cannot put method= on a body part: the \
             PARTSTAT would store perfectly and reach nobody",
        ),
        (
            false,
            false,
            true,
            Delivery::ClientImip,
            "a bare mailbox with no calendar: nothing to store, nothing contradicted, and the \
             organizer still learns the answer",
        ),
        (
            false,
            true,
            true,
            Delivery::None,
            "a scheduling server over a calendar we cannot write: it schedules on the write we \
             cannot make, and answering by mail alone would leave the user's own diary \
             disagreeing",
        ),
        (false, false, false, Delivery::None, "nothing at all"),
    ];

    for (can_store, server_schedules, can_send_imip, expected, why) in cases {
        assert_eq!(
            delivery(*can_store, *server_schedules, *can_send_imip),
            *expected,
            "({can_store}, {server_schedules}, {can_send_imip}): {why}"
        );
    }
}
