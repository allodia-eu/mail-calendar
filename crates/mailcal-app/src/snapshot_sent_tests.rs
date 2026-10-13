//! The "Sent" badge a conversation draws on a message: the owner sent it, which a draft, written
//! from the owner's address but never sent, is not.
//!
//! A child of [`super`].

use engine_api::EmailAddress;
use engine_core::{
    ids::{AccountId, MailboxId, MessageId},
    mail::{Keyword, Message, SystemKeyword},
    membership::Memberships,
};

use super::*;

const OWNER: &str = "me@example.com";

fn row(from: &str, draft: bool) -> MailListRow {
    let mut message = Message::new(
        MessageId::try_from("m1").unwrap(),
        Memberships::of_one(MailboxId::try_from("drafts").unwrap()),
    );
    message.envelope.from = vec![EmailAddress::new(from)];
    if draft {
        message
            .keywords
            .insert(Keyword::system(SystemKeyword::Draft));
    }
    MailListRow::project(&AccountId::try_from("acct").unwrap(), &message)
}

#[test]
fn the_owners_own_message_is_sent() {
    assert!(is_sent(&row(OWNER, false), Some(OWNER)));
    assert!(is_sent(&row("ME@example.com", false), Some(OWNER)));
}

#[test]
fn a_draft_from_the_owner_is_not_sent() {
    assert!(!is_sent(&row(OWNER, true), Some(OWNER)));
    // Still the owner's own mail, which is what keeps it out of new-mail notifications.
    assert!(is_outgoing(&row(OWNER, true), Some(OWNER)));
}

#[test]
fn someone_elses_message_is_not_sent() {
    assert!(!is_sent(&row("them@example.com", false), Some(OWNER)));
    assert!(!is_sent(&row(OWNER, false), None));
}
