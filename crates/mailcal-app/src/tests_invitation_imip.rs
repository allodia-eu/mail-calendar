//! Answering an invitation when the calendar server will not do it for us, end to end.
//!
//! This is the route that fixes the reported bug: a Microsoft invitation delivered to an IMAP
//! mailbox whose CalDAV calendar implements RFC 4791 and not RFC 6638. Nothing bridges the two,
//! so pressing Accept used to store a `PARTSTAT` that told nobody, or, once the honest refusal
//! landed, to fail outright.
//!
//! Every test here asserts on **both** halves, because both are the feature: what was stored on
//! the calendar, and what was put in the post. A suite that watched only the calendar would have
//! passed throughout the entire life of the bug.
//!
//! Fixtures live in `tests_fakes/invitation.rs`; the server-scheduled route is next door in
//! `tests_invitation_rsvp.rs`.

use std::sync::{Arc, Mutex};

use engine_api::ScheduleMethod;
use engine_provider::RsvpResponse;
use fakes::{
    ALIAS, InvitationFake, MEETING_UID, MESSAGE_KEY, RecordedPuts, RecordedSends, invitation_app,
};

use super::{CalendarWriteStatus, Intent, InvitationResponse, MessageRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

/// What one answer produced: the RSVPs the calendar took, the documents it stored, the drafts
/// the mail side sent, and the app to read the write status off.
struct Answered {
    rsvps: fakes::InvitationRsvps,
    puts: RecordedPuts,
    sends: RecordedSends,
    app: super::App<InvitationFake>,
}

/// Boots an app over `provider`, syncs both sides, and answers.
async fn answer(
    provider: InvitationFake,
    response: InvitationResponse,
    comment: Option<&str>,
    notify: bool,
    reply_subject: Option<&str>,
) -> Answered {
    let rsvps = provider.rsvps();
    let puts = provider.puts();
    let sends = provider.sends();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response,
        comment: comment.map(str::to_owned),
        notify_organizer: notify,
        reply_subject: reply_subject.map(str::to_owned),
    })
    .await;
    Answered {
        rsvps,
        puts,
        sends,
        app,
    }
}

/// The iTIP object a draft carries, or a failure naming what it carried instead.
fn itip_of(sends: &RecordedSends) -> (ScheduleMethod, String) {
    let drafts = sends.lock().unwrap();
    let draft = drafts.first().expect("a draft was submitted");
    let calendar = draft
        .calendar
        .as_ref()
        .expect("the draft carries an iTIP object, not a plain message");
    (calendar.method.clone(), calendar.ical.clone())
}

#[tokio::test]
async fn a_calendar_that_does_not_schedule_stores_the_meeting_and_emails_the_reply() {
    // The whole route, in one assertion set. Three things have to happen and the order matters:
    // the meeting reaches the calendar (so the user can see what they agreed to), the answer is
    // stored on it, and the reply is posted (so the organiser learns of it). Any one of them
    // missing is a bug a user would report.
    let answered = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Accept,
        None,
        true,
        None,
    )
    .await;

    assert_eq!(
        answered.puts.lock().unwrap().len(),
        1,
        "the invitation was put on the calendar"
    );
    let rsvps = answered.rsvps.lock().unwrap();
    assert_eq!(rsvps.len(), 1, "the answer was stored on it");
    assert_eq!(rsvps[0].0, ALIAS, "stored as the alias, not the identity");
    assert_eq!(rsvps[0].1, RsvpResponse::Accepted);
    drop(rsvps);

    let (method, ical) = itip_of(&answered.sends);
    assert_eq!(method, ScheduleMethod::Reply);
    assert!(ical.contains("METHOD:REPLY"));
    assert_eq!(
        answered.app.calendar_write_status(),
        CalendarWriteStatus::Saved
    );
}

#[tokio::test]
async fn the_stored_document_is_the_invitation_minus_its_method() {
    // RFC 4791 §4.1 forbids `METHOD` on a stored resource, and Sabre/DAV rejects the `PUT`. But
    // the `ATTENDEE` line is the entire reason to store the invitation rather than a plain
    // appointment; without it there is nothing to answer on afterwards, which is precisely why
    // this cannot go through `EventDraft`/`create_event`.
    let answered = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Accept,
        None,
        true,
        None,
    )
    .await;

    let puts = answered.puts.lock().unwrap();
    let (href, document) = puts.first().expect("a document was stored");
    assert!(!document.contains("METHOD:"), "{document}");
    assert!(document.contains(&format!(
        "ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:{ALIAS}"
    )));
    assert!(document.contains("ORGANIZER;CN=Boss:mailto:boss@test.local"));
    // And at the address a server would itself have chosen for this UID; percent-encoded as a
    // single path segment, byte for byte the way the CalDAV adapter encodes one, which is what
    // makes the create's `If-None-Match: *` able to *find* a copy rather than clobber it. This
    // UID carries an `@`, so the two spellings are not the same address.
    assert_eq!(href, "/cal/meeting-9%40test.local.ics");
    assert!(
        MEETING_UID.contains('@'),
        "the fixture must exercise encoding"
    );
}

#[tokio::test]
async fn the_reply_is_addressed_from_the_alias_the_invitation_matched() {
    // The same alias rule as the stored answer, now on the envelope. An organiser's scheduler
    // that finds no `ATTENDEE` matching the sender is entitled to ignore the reply: so sending
    // as the account's primary identity would be delivered, accepted, and silently discarded.
    let answered = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Tentative,
        None,
        true,
        None,
    )
    .await;

    let drafts = answered.sends.lock().unwrap();
    let draft = drafts.first().expect("a draft was submitted");
    assert_eq!(draft.from.email, ALIAS);
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].email, "boss@test.local");
    drop(drafts);

    let (_, ical) = itip_of(&answered.sends);
    assert!(ical.contains("PARTSTAT=TENTATIVE"));
    assert!(ical.contains(&format!("UID:{MEETING_UID}")));
}

#[tokio::test]
async fn each_button_writes_its_own_partstat_into_the_reply() {
    for (choice, expected) in [
        (InvitationResponse::Accept, "PARTSTAT=ACCEPTED"),
        (InvitationResponse::Tentative, "PARTSTAT=TENTATIVE"),
        (InvitationResponse::Decline, "PARTSTAT=DECLINED"),
    ] {
        let answered = answer(
            InvitationFake::new().without_server_scheduling(),
            choice,
            None,
            true,
            None,
        )
        .await;
        let (_, ical) = itip_of(&answered.sends);
        assert!(ical.contains(expected), "{choice:?} produced {ical}");
    }
}

#[tokio::test]
async fn a_note_reaches_the_organizer_on_a_transport_that_could_never_carry_one() {
    // CalDAV has nowhere to put a note, which is why `RsvpControls.comment` is `false` and why
    // the calendar write below is made **without** it; passing it there would be refused and
    // lose the whole answer. On this route the note is ours to carry: it becomes a `COMMENT` in
    // the reply we build, and a line in the message a human reads.
    let answered = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Tentative,
        Some("Might be ten minutes late"),
        true,
        None,
    )
    .await;

    let rsvps = answered.rsvps.lock().unwrap();
    assert_eq!(rsvps.len(), 1, "the answer was still stored");
    assert!(
        rsvps[0].2.is_none(),
        "the note must not be handed to a transport that refuses one"
    );
    drop(rsvps);

    let (_, ical) = itip_of(&answered.sends);
    assert!(ical.contains("COMMENT:Might be ten minutes late"), "{ical}");
    let drafts = answered.sends.lock().unwrap();
    assert!(drafts[0].text_body.contains("Might be ten minutes late"));
}

#[tokio::test]
async fn clearing_the_notify_tick_stores_the_answer_and_posts_nothing() {
    // On CalDAV this control could never be offered, because the server replies the moment the
    // `PARTSTAT` changes and there is no way to ask it not to. Here *we* are the sender, so the
    // tick means exactly what it says, and the answer is still recorded in the user's own
    // diary, which is the point of clearing it rather than not answering at all.
    let answered = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Decline,
        None,
        false,
        None,
    )
    .await;

    assert_eq!(answered.rsvps.lock().unwrap().len(), 1);
    assert!(
        answered.sends.lock().unwrap().is_empty(),
        "nothing may be posted when the user asked us not to"
    );
    assert_eq!(
        answered.app.calendar_write_status(),
        CalendarWriteStatus::Saved
    );
}

#[tokio::test]
async fn a_mailbox_with_no_calendar_still_tells_the_organizer() {
    // A bare IMAP account. There is nothing to store the answer in and nothing lost by not
    // storing it: no diary exists to contradict the reply. Refusing here would mean an
    // invitation the user can read, can see the times of, and cannot answer, for no better
    // reason than that they have not added a calendar.
    let answered = answer(
        InvitationFake::new().with_mail_only(),
        InvitationResponse::Accept,
        None,
        true,
        None,
    )
    .await;

    assert!(answered.puts.lock().unwrap().is_empty());
    assert!(answered.rsvps.lock().unwrap().is_empty());
    let (method, ical) = itip_of(&answered.sends);
    assert_eq!(method, ScheduleMethod::Reply);
    assert!(ical.contains("PARTSTAT=ACCEPTED"));
}

#[tokio::test]
async fn an_account_with_no_route_refuses_instead_of_pretending() {
    // A calendar that does not schedule beside a mail transport that cannot put `method=` on a
    // body part; JMAP's shape. Storing the `PARTSTAT` here would succeed and tell nobody,
    // which is the exact silence this whole change exists to end.
    let answered = answer(
        InvitationFake::new().with_no_route(),
        InvitationResponse::Accept,
        None,
        true,
        None,
    )
    .await;

    assert!(answered.rsvps.lock().unwrap().is_empty());
    assert!(answered.sends.lock().unwrap().is_empty());
    assert_eq!(
        answered.app.calendar_write_status(),
        CalendarWriteStatus::Failed
    );
}

#[tokio::test]
async fn the_client_supplies_the_subject_and_the_core_falls_back_to_the_invitations_own() {
    // The subject is copy a stranger reads, and the core has no locale: so the client composes
    // it. A client that passes nothing must still send something correct rather than English:
    // `Re:` plus the organiser's own words is true in every language.
    let localized = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Accept,
        None,
        true,
        Some("Geaccepteerd: Sprint planning"),
    )
    .await;
    assert_eq!(
        localized.sends.lock().unwrap()[0].subject,
        "Geaccepteerd: Sprint planning"
    );

    let fallback = answer(
        InvitationFake::new().without_server_scheduling(),
        InvitationResponse::Accept,
        None,
        true,
        None,
    )
    .await;
    assert_eq!(
        fallback.sends.lock().unwrap()[0].subject,
        "Re: Sprint planning"
    );
}

#[tokio::test]
async fn the_card_offers_the_two_controls_this_route_can_actually_honour() {
    // The mirror of the write path: a plain CalDAV account gains a note field and a notify tick
    // it has never had, because on this route both are ours to honour rather than a transport's
    // to refuse. `docs/invitations.md` forbids offering a control that lies; in either
    // direction.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(InvitationFake::new().without_server_scheduling(), &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;

    let card = app
        .reading_view()
        .invitation
        .expect("the message carries an invitation");
    assert!(
        card.can_respond,
        "the reply can be posted, so the buttons belong"
    );
    assert!(card.can_comment, "the note becomes a COMMENT in our reply");
    assert!(
        card.can_choose_notify,
        "we are the sender, so not sending is a real choice"
    );
}

#[tokio::test]
async fn a_card_with_no_route_offers_no_buttons_at_all() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(InvitationFake::new().with_no_route(), &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;

    let card = app
        .reading_view()
        .invitation
        .expect("the message still shows, with its details");
    assert!(!card.can_respond);
    assert!(!card.can_comment);
    assert!(!card.can_choose_notify);
}
