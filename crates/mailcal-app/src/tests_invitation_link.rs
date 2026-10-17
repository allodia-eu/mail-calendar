//! An invitation in a mail account that links to a calendar account: filed into, and answered
//! from, that calendar, with the mail account sending whatever has to be emailed
//! (`docs/invitations.md`, "Which calendar an invitation uses").
//!
//! The mail account `acct-a` has no calendar of its own; `cal-b` has no mail. The meeting invites
//! `info@`, an alias of `acct-a`, and `cal-b` signs in as another address altogether, so a test
//! that passes here proves the link carries the address set as well as the calendar.

use std::sync::{Arc, Mutex};

use engine_api::{ReplyDelivery, ScheduleMethod};
use engine_provider::RsvpResponse;
use mailcal_viewmodel::ResponseStatus;

use super::{
    Intent, InvitationResponse, MessageRef, Surface,
    fakes::{
        ALIAS, CALENDAR_ACCOUNT, EVENT_KEY, InvitationFake, InvitationRsvps, MESSAGE_KEY,
        OTHER_MAILBOX, RecordedSends, evt, invitation_app, linked_invitation_app,
        shared_calendar_app,
    },
};
use crate::{App, EventIntent, ReaderId};

fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

/// A preferences file naming `info@` as an alias of the mail account, which is what the calendar
/// needs to recognise the meeting as the user's: it has no message to read a `To:` from.
fn with_alias(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-invitation-link-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("preferences.toml");
    let mut prefs = mailcal_account::load_preferences(&path);
    prefs.set_account_aliases("acct-a", vec![ALIAS.to_owned()]);
    mailcal_account::save_preferences(&path, &prefs).unwrap();
    path
}

/// Boots the two accounts over `calendar`, linked or not, and returns the app with what the
/// mailbox was asked to send.
async fn booted(
    calendar: InvitationFake,
    linked: bool,
    prefs: Option<std::path::PathBuf>,
) -> (App<InvitationFake>, RecordedSends) {
    let mail = InvitationFake::new().with_mail_only();
    let sends = mail.sends();
    let surfaces: Arc<Mutex<Vec<Surface>>> = Arc::new(Mutex::new(Vec::new()));
    let app = linked_invitation_app(mail, calendar, linked, &surfaces, prefs);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    (app, sends)
}

async fn accept(app: &App<InvitationFake>) {
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        reply_subject: None,
    })
    .await;
}

#[tokio::test]
async fn an_invitation_is_answered_on_the_calendar_the_mailbox_links_to() {
    let calendar = InvitationFake::new();
    let rsvps = calendar.rsvps();
    let (app, sends) = booted(calendar, true, None).await;

    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: invite(),
    })
    .await;
    let card = app.reading_view().invitation.expect("an invitation card");
    assert!(card.can_respond, "the linked calendar's server schedules");
    assert!(
        !card.can_comment,
        "the calendar's transport decides the note, not the mailbox's"
    );

    accept(&app).await;
    let rsvps = rsvps.lock().unwrap();
    assert_eq!(
        rsvps.len(),
        1,
        "the answer is written on the linked calendar"
    );
    assert_eq!(rsvps[0].0, ALIAS);
    assert_eq!(rsvps[0].1, RsvpResponse::Accepted);
    assert!(
        sends.lock().unwrap().is_empty(),
        "the calendar's server tells the organiser, so the mailbox sends nothing"
    );
}

#[tokio::test]
async fn without_the_link_the_mailbox_answers_by_mail_alone() {
    let calendar = InvitationFake::new();
    let rsvps = calendar.rsvps();
    let (app, sends) = booted(calendar, false, None).await;

    accept(&app).await;
    assert!(
        rsvps.lock().unwrap().is_empty(),
        "no calendar is the mailbox's"
    );
    assert_eq!(sends.lock().unwrap().len(), 1, "the reply is emailed");
}

#[tokio::test]
async fn a_linked_calendar_that_does_not_schedule_files_the_meeting_and_the_mailbox_replies() {
    let calendar = InvitationFake::new().without_server_scheduling();
    let (puts, rsvps) = (calendar.puts(), calendar.rsvps());
    let (app, sends) = booted(calendar, true, None).await;

    accept(&app).await;
    assert_eq!(
        puts.lock().unwrap().len(),
        1,
        "the meeting is filed on the calendar"
    );
    assert_eq!(
        rsvps.lock().unwrap().len(),
        1,
        "and the answer stored on it"
    );
    let drafts = sends.lock().unwrap();
    let draft = drafts.first().expect("the mailbox emails the reply");
    let itip = draft.calendar.as_ref().expect("with an iTIP part");
    assert_eq!(itip.method, ScheduleMethod::Reply);
    assert!(itip.ical.contains(ALIAS));
}

#[tokio::test]
async fn a_calendar_server_that_does_not_schedule_as_the_invited_address_leaves_the_reply_to_mail()
{
    // It would store the answer and tell nobody: RFC 6638 has a server act only for an address
    // in its user's set, and this one names its login alone.
    let calendar = InvitationFake::new().scheduling_as(&["login@dav.test.local"]);
    let rsvps = calendar.rsvps();
    let (app, sends) = booted(calendar, true, None).await;

    accept(&app).await;
    assert_eq!(
        rsvps.lock().unwrap().len(),
        1,
        "the calendar's copy still moves"
    );
    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "and the mailbox tells the organiser"
    );
}

#[tokio::test]
async fn an_answer_from_the_linked_calendar_is_emailed_through_its_mail_account() {
    let mut calendar = InvitationFake::new().without_server_scheduling();
    calendar.materialized = true;
    let rsvps = calendar.rsvps();
    let (app, sends) = booted(calendar, true, Some(with_alias("from-calendar"))).await;
    let event = evt(CALENDAR_ACCOUNT, EVENT_KEY);

    let detail = app
        .event_detail(&event, None)
        .await
        .expect("the meeting is on the calendar");
    assert_eq!(
        detail.my_answer,
        Some(ResponseStatus::NeedsAction),
        "the calendar answers as its mail account's alias"
    );

    app.dispatch(Intent::Events(EventIntent::Respond {
        event,
        response: InvitationResponse::Tentative,
        reply_subject: None,
    }))
    .await;
    assert_eq!(rsvps.lock().unwrap()[0].1, RsvpResponse::Tentative);
    let drafts = sends.lock().unwrap();
    let draft = drafts.first().expect("the mail account emails the reply");
    assert!(
        draft
            .calendar
            .as_ref()
            .expect("an iTIP part")
            .ical
            .contains(ALIAS)
    );
}

#[tokio::test]
async fn a_reply_the_linked_calendar_could_not_deliver_is_the_mail_accounts_to_send() {
    let calendar = InvitationFake::new().reporting_delivery(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    });
    let (app, sends) = booted(calendar, true, None).await;

    accept(&app).await;
    let prompt = app.reply_prompt().expect("a failure raises the question");
    assert_eq!(
        prompt.account.as_str(),
        "acct-a",
        "a standing permission to send mail belongs to the account that sends it"
    );
    app.dispatch(Intent::AnswerReplyPrompt {
        send: true,
        remember: false,
        reply_subject: None,
    })
    .await;
    assert_eq!(sends.lock().unwrap().len(), 1, "the mailbox emails it");
}

/// The invitation's own account, mail and calendar on one sign-in, over `provider`.
async fn one_account(
    provider: InvitationFake,
) -> (App<InvitationFake>, InvitationRsvps, RecordedSends) {
    let (rsvps, sends) = (provider.rsvps(), provider.sends());
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    (app, rsvps, sends)
}

#[tokio::test]
async fn a_server_naming_only_part_of_its_set_keeps_the_server_route_for_an_alias() {
    // Graph and Google name the primary address alone and schedule for the aliases too; taking
    // the mail route for an alias there would send the organiser a second reply.
    let (app, rsvps, sends) = one_account(
        InvitationFake::new()
            .scheduling_as_at_least(&["me@test.local"])
            .sending_imip(),
    )
    .await;

    accept(&app).await;
    assert_eq!(rsvps.lock().unwrap().len(), 1, "the server route");
    assert!(sends.lock().unwrap().is_empty(), "and no reply of our own");
}

#[tokio::test]
async fn a_server_on_the_same_account_that_does_not_schedule_as_the_alias_leaves_it_to_mail() {
    // IMAP beside CalDAV on one sign-in: the principal's whole set is the login alone.
    let (app, rsvps, sends) = one_account(
        InvitationFake::new()
            .scheduling_as(&["me@test.local"])
            .sending_imip(),
    )
    .await;

    accept(&app).await;
    assert_eq!(rsvps.lock().unwrap().len(), 1, "the answer is stored");
    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "and the account emails the reply"
    );
}

#[tokio::test]
async fn a_calendar_without_mail_offers_no_answer_its_server_would_not_send() {
    let mut calendar = InvitationFake::new().scheduling_as(&["login@dav.test.local"]);
    calendar.materialized = true;
    let prefs = with_alias("calendar-only");
    let mut stored = mailcal_account::load_preferences(&prefs);
    stored.set_account_aliases(CALENDAR_ACCOUNT, vec![ALIAS.to_owned()]);
    mailcal_account::save_preferences(&prefs, &stored).unwrap();
    let (app, _) = booted(calendar, false, Some(prefs)).await;

    let detail = app
        .event_detail(&evt(CALENDAR_ACCOUNT, EVENT_KEY), None)
        .await
        .expect("the meeting is on the calendar");
    assert_eq!(
        detail.my_answer, None,
        "the server would store it and tell nobody, and there is no mail to send it"
    );
}

#[tokio::test]
async fn a_linked_calendar_that_is_not_connected_leaves_the_mailbox_replying() {
    let calendar = InvitationFake::new().without_server_scheduling();
    let (app, sends) = booted(calendar, true, None).await;
    // The link still holds in Settings while the calendar account is not in the running app.
    app.remove_account(&engine_api::AccountId::try_from(CALENDAR_ACCOUNT).unwrap())
        .await;

    accept(&app).await;
    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "the reply is emailed although the meeting cannot be filed"
    );
}

#[tokio::test]
async fn a_calendar_two_mailboxes_file_into_answers_from_the_mailbox_it_invited() {
    let mut calendar = InvitationFake::new().without_server_scheduling();
    calendar.materialized = true;
    let (mail, other) = (
        InvitationFake::new().with_mail_only(),
        InvitationFake::new().with_mail_only(),
    );
    let (sends, other_sends) = (mail.sends(), other.sends());
    let surfaces: Arc<Mutex<Vec<Surface>>> = Arc::new(Mutex::new(Vec::new()));
    let app = shared_calendar_app(mail, other, calendar, &surfaces, Some(with_alias("shared")));
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::Events(EventIntent::Respond {
        event: evt(CALENDAR_ACCOUNT, EVENT_KEY),
        response: InvitationResponse::Accept,
        reply_subject: None,
    }))
    .await;
    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "the meeting invites `acct-a`'s alias, so `acct-a` emails the reply"
    );
    assert!(
        other_sends.lock().unwrap().is_empty(),
        "not {OTHER_MAILBOX}, although the calendar sends through it"
    );
}
