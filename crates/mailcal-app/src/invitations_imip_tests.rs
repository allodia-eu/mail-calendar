//! Tests for the piece of the client-iMIP route that is decidable without a provider: the
//! subject the reply falls back to.
//!
//! The route itself is exercised end to end in `tests_invitation_rsvp.rs`, over a fake that
//! records both the document stored and the message sent; because the point of this route is
//! that *both* happen, and a test that could only see one of them would pass while the
//! organiser was still waiting.

use engine_api::Message;
use engine_core::{
    ids::{MailboxId, MessageId},
    membership::Memberships,
};

use super::reply_subject;

fn message_titled(subject: Option<&str>) -> Message {
    let mut message = Message::new(
        MessageId::try_from("m-1").expect("a message id"),
        Memberships::of_one(MailboxId::try_from("INBOX").expect("a mailbox")),
    );
    message.envelope.subject = subject.map(str::to_owned);
    message
}

#[test]
fn the_fallback_subject_quotes_the_invitation_back() {
    // Deliberately not "Accepted: …": the core has no locale, and announcing the answer in a
    // language the user does not speak is worse than not announcing it. The answer itself
    // travels in the iTIP part, which is what the organiser's client actually reads.
    assert_eq!(
        reply_subject(&message_titled(Some("Sprint planning"))),
        "Re: Sprint planning"
    );
}

#[test]
fn an_already_prefixed_subject_is_not_prefixed_twice() {
    // Answering a re-sent invitation would otherwise produce "Re: Re: Re: Sprint planning".
    for existing in ["Re: Sprint planning", "RE: Sprint planning", "re:Sprint"] {
        let subject = reply_subject(&message_titled(Some(existing)));
        assert_eq!(subject, existing, "{existing:?} was prefixed again");
    }
}

#[test]
fn a_subjectless_invitation_still_gets_a_subject() {
    // An empty `Subject:` header is legal and some schedulers emit one. A draft with no subject
    // at all is more likely to be filtered than one with a bare `Re:`.
    assert_eq!(reply_subject(&message_titled(None)), "Re:");
    assert_eq!(reply_subject(&message_titled(Some(""))), "Re:");
}
