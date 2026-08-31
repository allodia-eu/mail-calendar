//! Tests for the pieces of the client-iMIP route that are decidable without a provider: the
//! resource address an invitation is stored at, and the subject the reply falls back to.
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

use super::{encode_path_segment, event_href, reply_subject};

fn message_titled(subject: Option<&str>) -> Message {
    let mut message = Message::new(
        MessageId::try_from("m-1").expect("a message id"),
        Memberships::of_one(MailboxId::try_from("INBOX").expect("a mailbox")),
    );
    message.envelope.subject = subject.map(str::to_owned);
    message
}

#[test]
fn the_address_follows_the_convention_a_server_would_have_used() {
    // RFC 4791 §5.3.2 lets the client name the resource, and `<uid>.ics` is what everything
    // else picks. That is not cosmetic: it is what makes the create's `If-None-Match: *` worth
    // having, because a copy the server deposited for the same meeting is sitting at exactly
    // this address. A random name would always succeed, and always duplicate.
    let href = event_href("/calendars/alice/work/", "meeting-9").expect("an event address");
    assert_eq!(href.as_str(), "/calendars/alice/work/meeting-9.ics");
}

#[test]
fn a_collection_without_its_trailing_slash_still_yields_a_child() {
    // Concatenating onto `/work` would produce `/workmeeting-9.ics`: a sibling of the
    // collection rather than a resource inside it, which a server answers with a 404 or,
    // worse, accepts into a place nothing syncs.
    let href = event_href("/calendars/alice/work", "meeting-9").expect("an event address");
    assert_eq!(href.as_str(), "/calendars/alice/work/meeting-9.ics");
}

#[test]
fn an_opaque_uid_is_percent_encoded_rather_than_trusted() {
    // A `UID` is opaque and attacker-influenced: Google writes an `@`, and nothing in RFC 5545
    // forbids a `/`. Pasted into a path unencoded, that slash silently retargets the write into
    // another collection.
    assert_eq!(encode_path_segment("evt@example.org"), "evt%40example.org");
    assert_eq!(encode_path_segment("a/b"), "a%2Fb");
    assert_eq!(encode_path_segment("../secret"), "..%2Fsecret");
    // The unreserved set is left alone, because only for those characters are the literal byte
    // and its `%XX` form equivalent; encoding them would produce a *different* address from
    // the one a server minted for the same UID, and the guard would then never fire.
    assert_eq!(encode_path_segment("aZ0-._~"), "aZ0-._~");
}

#[test]
fn a_non_ascii_uid_is_encoded_byte_by_byte() {
    // Percent-encoding is defined over octets, not characters (RFC 3986 §2.1), so a two-byte
    // character becomes two escapes.
    assert_eq!(encode_path_segment("é"), "%C3%A9");
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
