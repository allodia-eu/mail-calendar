use engine_api::EmailAddress;
use engine_core::{
    ids::{AccountId, MailboxId, MessageId},
    mail::Message,
    membership::Memberships,
};

use super::*;

/// Builds a message in `mailbox` from `from`, with `subject` and an optional received time,
/// projected into the list row a scan actually reads.
fn msg(id: &str, mailbox: &str, from: &str, subject: &str, received: Option<&str>) -> MailListRow {
    let mut message = Message::new(
        MessageId::try_from(id).unwrap(),
        Memberships::of_one(MailboxId::try_from(mailbox).unwrap()),
    );
    message.envelope.from = vec![EmailAddress::new(from)];
    message.envelope.subject = Some(subject.to_owned());
    message.received_at = received.map(|raw| raw.parse().unwrap());
    MailListRow::project(&AccountId::try_from("acct").unwrap(), &message)
}

const OWNER: &str = "me@example.com";

#[test]
fn first_run_seeds_the_high_water_and_reports_nothing() {
    let messages = vec![
        msg(
            "m1",
            "inbox",
            "a@x.com",
            "One",
            Some("2026-06-01T09:00:00Z"),
        ),
        msg(
            "m2",
            "inbox",
            "b@x.com",
            "Two",
            Some("2026-06-01T10:00:00Z"),
        ),
    ];
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), None, None);
    assert!(scan.previews.is_empty(), "a first run reports nothing");
    assert_eq!(
        scan.high_water,
        Some("2026-06-01T10:00:00Z".parse().unwrap()),
        "the seed is the newest inbound message",
    );
}

#[test]
fn reports_only_messages_newer_than_the_mark_newest_first() {
    let messages = vec![
        msg(
            "old",
            "inbox",
            "a@x.com",
            "Old",
            Some("2026-06-01T08:00:00Z"),
        ),
        msg(
            "new1",
            "inbox",
            "b@x.com",
            "New one",
            Some("2026-06-01T11:00:00Z"),
        ),
        msg(
            "new2",
            "inbox",
            "c@x.com",
            "New two",
            Some("2026-06-01T10:00:00Z"),
        ),
    ];
    let mark = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, None);
    let subjects: Vec<&str> = scan
        .previews
        .iter()
        .map(|preview| preview.subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        ["New one", "New two"],
        "newest first, older excluded"
    );
    assert_eq!(
        scan.high_water,
        Some("2026-06-01T11:00:00Z".parse().unwrap())
    );
}

#[test]
fn the_owners_own_sent_mail_is_excluded() {
    let messages = vec![
        msg(
            "mine",
            "inbox",
            OWNER,
            "My reply",
            Some("2026-06-01T12:00:00Z"),
        ),
        msg(
            "theirs",
            "inbox",
            "them@x.com",
            "Their mail",
            Some("2026-06-01T11:00:00Z"),
        ),
    ];
    let mark = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, None);
    let subjects: Vec<&str> = scan
        .previews
        .iter()
        .map(|preview| preview.subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        ["Their mail"],
        "the owner's own message never notifies"
    );
}

#[test]
fn messages_outside_the_inbox_are_excluded() {
    let messages = vec![
        msg(
            "arch",
            "archive",
            "a@x.com",
            "Archived",
            Some("2026-06-01T12:00:00Z"),
        ),
        msg(
            "in",
            "inbox",
            "b@x.com",
            "Inbox",
            Some("2026-06-01T11:00:00Z"),
        ),
    ];
    let mark = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, None);
    let subjects: Vec<&str> = scan
        .previews
        .iter()
        .map(|preview| preview.subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        ["Inbox"],
        "only the resolved Inbox folder notifies"
    );
}

#[test]
fn undated_messages_are_never_reported() {
    let messages = vec![msg("nodate", "inbox", "a@x.com", "No date", None)];
    let mark = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, None);
    assert!(
        scan.previews.is_empty(),
        "an undated message can't be ordered; skip it"
    );
    assert_eq!(scan.high_water, None);
}

#[test]
fn an_empty_inbox_has_no_high_water() {
    let scan = newly_arrived(&[], "inbox", Some(OWNER), None, None);
    assert!(scan.previews.is_empty());
    assert_eq!(
        scan.high_water, None,
        "the caller seeds an empty inbox to now"
    );
}

#[test]
fn a_preview_carries_the_sender_name_subject_snippet_and_key() {
    let mut message = Message::new(
        MessageId::try_from("k1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.from = vec![EmailAddress::named("Jane Doe", "jane@x.com")];
    message.envelope.subject = Some("Q3 report".to_owned());
    message.preview = Some("The numbers you asked for are attached.".to_owned());
    message.received_at = Some("2026-06-01T10:00:00Z".parse().unwrap());
    let preview = preview_of(&MailListRow::project(
        &AccountId::try_from("acct").unwrap(),
        &message,
    ));
    assert_eq!(preview.sender, "jane@x.com");
    assert_eq!(preview.sender_name.as_deref(), Some("Jane Doe"));
    assert_eq!(preview.subject, "Q3 report");
    assert_eq!(
        preview.preview, "The numbers you asked for are attached.",
        "the notification says how the message begins, from the snippet the list row already shows"
    );
    assert_eq!(preview.received, "2026-06-01T10:00:00Z");
    assert_eq!(preview.message_key, "k1");
}

#[test]
fn a_message_whose_body_has_not_been_fetched_previews_as_empty() {
    // An IMAP account's snippet is computed by the body sync, so a message reported by the pass
    // that committed its headers has none yet. Empty rather than absent: a host drops the line
    // and shows sender and subject, which is what every client did before there was a snippet.
    let mut message = Message::new(
        MessageId::try_from("k2").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.from = vec![EmailAddress::new("jane@x.com")];
    message.envelope.subject = Some("No body yet".to_owned());
    message.received_at = Some("2026-06-01T10:00:00Z".parse().unwrap());
    let preview = preview_of(&MailListRow::project(
        &AccountId::try_from("acct").unwrap(),
        &message,
    ));
    assert!(preview.preview.is_empty());
}

#[test]
fn the_launch_catch_up_is_marked_seen_rather_than_reported() {
    // Two messages past the mark: one that landed while the app was closed, one that arrived
    // after it opened. A desktop scan floors its reports at the session start.
    let messages = vec![
        msg(
            "while-closed",
            "inbox",
            "a@x.com",
            "Overnight",
            Some("2026-06-01T03:00:00Z"),
        ),
        msg(
            "while-open",
            "inbox",
            "b@x.com",
            "Just now",
            Some("2026-06-01T09:30:00Z"),
        ),
    ];
    let mark = Some("2026-05-31T18:00:00Z".parse().unwrap());
    let session_start = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, session_start);
    let subjects: Vec<&str> = scan
        .previews
        .iter()
        .map(|preview| preview.subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        ["Just now"],
        "mail the launch sync brought in is already on screen; only what arrived since notifies"
    );
    assert_eq!(
        scan.high_water,
        Some("2026-06-01T09:30:00Z".parse().unwrap()),
        "the withheld message still advances the mark, so a later pass cannot announce it",
    );
}

#[test]
fn a_session_floor_below_the_mark_changes_nothing() {
    // A long-running desktop: the session started before mail the previous pass already
    // reported, so the mark is the later of the two and stays in charge.
    let messages = vec![
        msg(
            "reported",
            "inbox",
            "a@x.com",
            "Already seen",
            Some("2026-06-01T10:00:00Z"),
        ),
        msg(
            "fresh",
            "inbox",
            "b@x.com",
            "New",
            Some("2026-06-01T11:00:00Z"),
        ),
    ];
    let mark = Some("2026-06-01T10:30:00Z".parse().unwrap());
    let session_start = Some("2026-06-01T08:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark, session_start);
    let subjects: Vec<&str> = scan
        .previews
        .iter()
        .map(|preview| preview.subject.as_str())
        .collect();
    assert_eq!(subjects, ["New"]);
}

#[test]
fn a_first_run_reports_nothing_whatever_the_session_floor_says() {
    let messages = vec![msg(
        "m1",
        "inbox",
        "a@x.com",
        "One",
        Some("2026-06-01T12:00:00Z"),
    )];
    let session_start = Some("2026-06-01T09:00:00Z".parse().unwrap());
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), None, session_start);
    assert!(
        scan.previews.is_empty(),
        "seeding an account outranks the session floor; neither notifies"
    );
    assert_eq!(
        scan.high_water,
        Some("2026-06-01T12:00:00Z".parse().unwrap())
    );
}
