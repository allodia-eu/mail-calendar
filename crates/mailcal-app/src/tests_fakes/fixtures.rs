//! The data fixtures the loop tests build cases out of: the [`Message`] builders (plain, provider
//! threaded, and unthreaded-with-headers) and the typed references. Split out of `tests_fakes.rs`
//! , which keeps the [`FakeProvider`](super::FakeProvider), the recording observer, and the
//! `account`/`app` builders; to stay under the 500-line limit.

use engine_core::{
    ids::{MailboxId, MessageId, MessageIdHeader, ThreadId},
    mail::Message,
    membership::Memberships,
};

use crate::{EventRef, FolderRef, Intent, MessageRef, ThreadRef};

pub(crate) fn message(id: &str, mailbox: &str, subject: &str) -> Message {
    let mut message = Message::new(
        MessageId::try_from(id).unwrap(),
        Memberships::of_one(MailboxId::try_from(mailbox).unwrap()),
    );
    message.envelope.subject = Some(subject.to_owned());
    message
}

/// Like [`message`], but on thread `thread`: so several messages (across folders) group into
/// one conversation. The id is **provider-assigned**: nothing derived it, which is what a fake
/// delivering an already-threaded message models.
pub(crate) fn threaded(id: &str, mailbox: &str, subject: &str, thread: &str) -> Message {
    let mut message = message(id, mailbox, subject);
    message.thread = Some(engine_core::mail::ThreadRef::provider_assigned(
        ThreadId::try_from(thread).unwrap(),
    ));
    message
}

/// Like [`message`], but carrying **no** thread id and the RFC 5322 threading headers instead;
/// the shape a provider without server-side threading (IMAP) delivers, so the engine has to
/// derive the thread from `owned` (`Message-ID`) and `references` (`References`).
pub(crate) fn unthreaded(
    id: &str,
    mailbox: &str,
    subject: &str,
    owned: &str,
    references: &[&str],
) -> Message {
    let mut message = message(id, mailbox, subject);
    message.envelope.message_id = vec![MessageIdHeader::new(owned).unwrap()];
    message.envelope.references = references
        .iter()
        .map(|r| MessageIdHeader::new(*r).unwrap())
        .collect();
    message
}

/// A typed message reference (account + provider key) for the loop tests.
pub(crate) fn msg(account: &str, key: &str) -> MessageRef {
    MessageRef::from_parts(account, key.to_owned()).unwrap()
}

/// A typed thread reference (account + thread id) for the loop tests.
pub(crate) fn thread_ref(account: &str, thread_id: &str) -> ThreadRef {
    ThreadRef::from_parts(account, thread_id.to_owned()).unwrap()
}

/// The intent that opens one account's folder: the only way to select one, since a folder key
/// is unique only within its account (`docs/folder-pane.md`, rule 14).
pub(crate) fn open_folder(account: &str, key: &str) -> Intent {
    Intent::SelectFolder {
        folder: FolderRef::from_parts(account, key.to_owned()).unwrap(),
    }
}

/// A typed event reference (account + provider key) for the loop tests.
pub(crate) fn evt(account: &str, key: &str) -> EventRef {
    EventRef::from_parts(account, key.to_owned()).unwrap()
}
