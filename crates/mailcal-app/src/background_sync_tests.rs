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
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), None);
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
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark);
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
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark);
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
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark);
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
    let scan = newly_arrived(&messages, "inbox", Some(OWNER), mark);
    assert!(
        scan.previews.is_empty(),
        "an undated message can't be ordered; skip it"
    );
    assert_eq!(scan.high_water, None);
}

#[test]
fn an_empty_inbox_has_no_high_water() {
    let scan = newly_arrived(&[], "inbox", Some(OWNER), None);
    assert!(scan.previews.is_empty());
    assert_eq!(
        scan.high_water, None,
        "the caller seeds an empty inbox to now"
    );
}

#[test]
fn a_preview_carries_the_sender_name_subject_and_key() {
    let mut message = Message::new(
        MessageId::try_from("k1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.from = vec![EmailAddress::named("Jane Doe", "jane@x.com")];
    message.envelope.subject = Some("Q3 report".to_owned());
    message.received_at = Some("2026-06-01T10:00:00Z".parse().unwrap());
    let preview = preview_of(&MailListRow::project(
        &AccountId::try_from("acct").unwrap(),
        &message,
    ));
    assert_eq!(preview.sender, "jane@x.com");
    assert_eq!(preview.sender_name.as_deref(), Some("Jane Doe"));
    assert_eq!(preview.subject, "Q3 report");
    assert_eq!(preview.received, "2026-06-01T10:00:00Z");
    assert_eq!(preview.message_key, "k1");
}
