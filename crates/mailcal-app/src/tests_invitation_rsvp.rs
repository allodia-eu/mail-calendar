//! Answering an invitation, end to end through [`super::App`]: the address the answer goes
//! out as, the two controls a transport may not honour, and the refusals that must not be
//! silent.
//!
//! These go through the real `Intent` rather than calling the command, because the whole
//! feature is a *dispatch*: a client sends a message key and a choice, and every other input;
//! the alias, the event, the guard, is derived here. Fixtures live in
//! `tests_fakes/invitation.rs`.

use std::sync::{Arc, Mutex};

use engine_provider::RsvpResponse;
use fakes::{ALIAS, InvitationFake, MESSAGE_KEY, invitation_app};

use super::{CalendarWriteStatus, Intent, InvitationResponse, MessageRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The message the invitation arrived in, named the way a client names it.
fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

/// Boots an app, syncs mail and calendar, and answers.
async fn answer(
    provider: InvitationFake,
    response: InvitationResponse,
    comment: Option<&str>,
    notify: bool,
) -> (
    fakes::InvitationRsvps,
    Arc<Mutex<Vec<super::Surface>>>,
    super::App<InvitationFake>,
) {
    let rsvps = provider.rsvps();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response,
        comment: comment.map(str::to_owned),
        notify_organizer: notify,
        reply_subject: None,
    })
    .await;
    (rsvps, surfaces, app)
}

#[tokio::test]
async fn the_answer_goes_out_as_the_address_the_invitation_matched() {
    // D5, end to end. The account's identity is `me@test.local`; the invitation is addressed
    // to `info@test.local` and reaches the matcher through the message's own `To:` header,
    // with no alias configured anywhere. Answering as the *identity* would name an ATTENDEE
    // the event does not have, and the reply would reach nobody, which is exactly the bug
    // this rule exists to prevent, and it fails silently on a server that accepts the write.
    let (rsvps, _surfaces, _app) = answer(
        InvitationFake::new(),
        InvitationResponse::Accept,
        None,
        true,
    )
    .await;

    let sent = rsvps.lock().unwrap();
    assert_eq!(sent.len(), 1, "exactly one RSVP reached the provider");
    assert_eq!(sent[0].0, ALIAS, "answered as the alias, not the identity");
    assert_eq!(sent[0].1, RsvpResponse::Accepted);
}

#[tokio::test]
async fn each_button_sends_its_own_answer() {
    for (choice, expected) in [
        (InvitationResponse::Accept, RsvpResponse::Accepted),
        (InvitationResponse::Tentative, RsvpResponse::Tentative),
        (InvitationResponse::Decline, RsvpResponse::Declined),
    ] {
        let (rsvps, ..) = answer(InvitationFake::new(), choice, None, true).await;
        assert_eq!(rsvps.lock().unwrap()[0].1, expected);
    }
}

#[tokio::test]
async fn answering_tells_the_organizer_by_default() {
    // RFC 5546's default, and Outlook's: an invitation asks for a reply, so answering sends
    // one. A client that forgot to pass the flag must not accidentally answer in silence.
    let (rsvps, ..) = answer(
        InvitationFake::new(),
        InvitationResponse::Accept,
        None,
        true,
    )
    .await;
    assert!(rsvps.lock().unwrap()[0].3, "the organizer is told");
}

#[tokio::test]
async fn a_blank_note_is_not_a_note() {
    // A client whose note field is always present would otherwise send `Some("")` on every
    // answer, and be refused on CalDAV and JMAP, which carry no note at all. The user typed
    // nothing, nothing is what should travel.
    let (rsvps, _surfaces, _app) = answer(
        InvitationFake::new(),
        InvitationResponse::Accept,
        Some("   "),
        true,
    )
    .await;

    let sent = rsvps.lock().unwrap();
    assert_eq!(
        sent.len(),
        1,
        "the answer still went, on a no-note transport"
    );
    assert!(sent[0].2.is_none());
}

#[tokio::test]
async fn a_note_reaches_a_transport_that_can_carry_one() {
    let (rsvps, _surfaces, _app) = answer(
        InvitationFake::new().with_full_controls(),
        InvitationResponse::Tentative,
        Some("Might be ten minutes late"),
        true,
    )
    .await;

    let sent = rsvps.lock().unwrap();
    assert_eq!(sent[0].2.as_deref(), Some("Might be ten minutes late"));
}

#[tokio::test]
async fn declining_quietly_is_honoured_where_the_transport_allows_it() {
    let (rsvps, _surfaces, _app) = answer(
        InvitationFake::new().with_full_controls(),
        InvitationResponse::Decline,
        None,
        false,
    )
    .await;

    let sent = rsvps.lock().unwrap();
    assert_eq!(sent[0].1, RsvpResponse::Declined);
    assert!(!sent[0].3, "the organizer is not emailed");
}

#[tokio::test]
async fn a_note_a_transport_cannot_carry_fails_the_answer_rather_than_being_dropped() {
    // The honesty rule, from the user's side: on CalDAV or JMAP a note has nowhere to go. If
    // it were dropped the answer would succeed and the user would believe the organiser read
    // their message. Failing is the lesser harm, and the card's `can_comment` is what stops a
    // client ever getting here.
    let (rsvps, _surfaces, app) = answer(
        InvitationFake::new(),
        InvitationResponse::Decline,
        Some("Sorry, away that week"),
        true,
    )
    .await;

    assert!(
        rsvps.lock().unwrap().is_empty(),
        "the write was refused before it reached the provider"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn asking_to_stay_quiet_where_the_server_always_replies_fails_rather_than_pretending() {
    let (rsvps, _surfaces, app) = answer(
        InvitationFake::new(),
        InvitationResponse::Accept,
        None,
        false,
    )
    .await;

    assert!(rsvps.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn a_scheduling_server_that_never_filed_the_invitation_gets_it_filed_for_it() {
    // **The reported bug.** `caldav.soverin.net` advertises `calendar-auto-schedule`, so the
    // capability reads `true` and this is the server route, and the meeting is still on no
    // calendar, because advertising that you schedule is not a promise to move invitations out
    // of somebody's mailbox. Nothing in any RFC assigns that job. So the account looked
    // answerable, and answering failed with "the meeting is not in this account's calendar".
    //
    // Storing it first is the attendee flow of RFC 6638 §3.2.2: put the scheduling object in
    // your own calendar, and the server turns the changed PARTSTAT into the REPLY. The core
    // sends no mail of its own here, that would be the second reply the organiser receives.
    let provider = InvitationFake::new().without_the_meeting();
    let puts = provider.puts();
    let sends = provider.sends();
    let (rsvps, _surfaces, app) = answer(provider, InvitationResponse::Accept, None, true).await;

    assert_eq!(puts.lock().unwrap().len(), 1, "the meeting was filed");
    assert_eq!(rsvps.lock().unwrap().len(), 1, "and then answered");
    assert!(
        sends.lock().unwrap().is_empty(),
        "the server sends the reply here; ours would be a duplicate"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Saved);
}

#[tokio::test]
async fn the_card_reports_the_calendars_answer_not_the_frozen_emails() {
    // The invitation email is fixed at the moment it was sent: its ATTENDEE line says
    // NEEDS-ACTION and always will. Reading `my_response` from it would mean accepting a
    // meeting, reopening the message, and being asked again; with the buttons implying the
    // first answer never happened.
    let provider = InvitationFake::new();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;
    let card = app
        .reading_view()
        .invitation
        .expect("the message carries an invitation");
    assert_eq!(
        card.my_response,
        mailcal_viewmodel::ResponseStatus::NeedsAction
    );
    assert!(card.can_respond, "a server-scheduled account can answer");
    assert!(!card.can_comment, "CalDAV/JMAP carry no note");
    assert!(
        !card.can_choose_notify,
        "a server-scheduled reply cannot be suppressed"
    );
}

#[tokio::test]
async fn an_invitation_the_calendar_has_moved_past_is_marked_superseded() {
    // An organiser who moves a meeting re-sends the whole invitation, and both copies stay in
    // the mailbox. The older one would otherwise keep offering Accept over times that are no
    // longer the meeting's, which is what the calendar's higher SEQUENCE settles.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(
        InvitationFake::new().superseded_by_a_newer_revision(),
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;
    let card = app
        .reading_view()
        .invitation
        .expect("the message carries an invitation");
    assert_eq!(card.kind, mailcal_viewmodel::InvitationKind::Superseded);
    assert!(
        !card.can_respond,
        "a superseded invitation must not offer buttons; its times are not the meeting's"
    );
}

#[tokio::test]
async fn answering_a_superseded_invitation_is_refused_rather_than_landing_on_the_newer_one() {
    // The card hides the buttons, but the write refuses too, and the two are not redundant: the
    // RSVP resolves the event by UID, so it would land on the meeting **as it now is** while the
    // user was reading the old mail's times. Accepting a slot you were never shown is worse than
    // a visible refusal.
    let (rsvps, _surfaces, app) = answer(
        InvitationFake::new().superseded_by_a_newer_revision(),
        InvitationResponse::Accept,
        None,
        true,
    )
    .await;

    assert!(
        rsvps.lock().unwrap().is_empty(),
        "no answer may reach the provider for a superseded invitation"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}
