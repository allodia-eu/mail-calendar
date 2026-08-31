use crate::{MailtoPrefill, parse_mailto};

fn parse(uri: &str) -> MailtoPrefill {
    parse_mailto(uri).expect("a mailto URI")
}

#[test]
fn a_non_mailto_uri_is_not_parsed() {
    // The client asks this before opening anything, so an `https:`/`tel:`/junk intent that
    // reached the activity by mistake must be a clean "not mine", never a blank composer.
    assert!(parse_mailto("https://allodia.eu").is_none());
    assert!(parse_mailto("tel:+3112345678").is_none());
    assert!(parse_mailto("").is_none());
    assert!(parse_mailto("alice@example.test").is_none());
}

#[test]
fn the_scheme_is_case_insensitive_and_may_be_padded() {
    assert_eq!(parse("MAILTO:alice@example.test").to, "alice@example.test");
    assert_eq!(
        parse("  mailto:alice@example.test  ").to,
        "alice@example.test"
    );
}

#[test]
fn a_bare_mailto_opens_a_blank_composer() {
    // `mailto:` with nothing after it is a legal link. It means "open a composer", so it
    // parses successfully with every field empty rather than being rejected.
    let prefill = parse("mailto:");
    assert!(prefill.is_empty());
    assert_eq!(prefill.to, "");
}

#[test]
fn the_path_carries_one_or_more_recipients() {
    assert_eq!(parse("mailto:alice@example.test").to, "alice@example.test");
    assert_eq!(
        parse("mailto:alice@example.test,bob@example.test").to,
        "alice@example.test, bob@example.test",
    );
}

#[test]
fn every_honoured_field_is_decoded() {
    let prefill = parse(
        "mailto:alice@example.test?cc=carol@example.test&bcc=dan@example.test\
         &subject=Quarterly%20report&body=Hi%20Alice",
    );
    assert_eq!(prefill.to, "alice@example.test");
    assert_eq!(prefill.cc, "carol@example.test");
    assert_eq!(prefill.bcc, "dan@example.test");
    assert_eq!(prefill.subject, "Quarterly report");
    assert_eq!(prefill.body, "Hi Alice");
}

#[test]
fn recipients_accumulate_from_the_path_and_the_query() {
    // RFC 6068 allows `to` as a header field as well as in the path, including the common
    // `mailto:?to=…` form that has no path at all. Both sources combine, in order.
    let prefill =
        parse("mailto:alice@example.test?to=bob@example.test&cc=c@example.test&cc=d@example.test");
    assert_eq!(prefill.to, "alice@example.test, bob@example.test");
    assert_eq!(prefill.cc, "c@example.test, d@example.test");

    assert_eq!(
        parse("mailto:?to=solo@example.test").to,
        "solo@example.test"
    );
}

#[test]
fn header_names_are_case_insensitive() {
    let prefill = parse("mailto:a@example.test?SUBJECT=Hi&Body=There&CC=c@example.test");
    assert_eq!(prefill.subject, "Hi");
    assert_eq!(prefill.body, "There");
    assert_eq!(prefill.cc, "c@example.test");
}

#[test]
fn a_plus_is_a_literal_plus_not_a_space() {
    // The whole reason this is hand-decoded: `mailto:` is percent-encoded (RFC 6068 §2), not
    // form-encoded. A generic query parser turns `a+b` into `a b` and quietly mangles both
    // plus-addressed recipients and any subject containing a plus.
    let prefill = parse("mailto:alice+newsletter@example.test?subject=C%2B%2B%20vs%20Rust");
    assert_eq!(prefill.to, "alice+newsletter@example.test");
    // The encoded pluses decode; the spaces came from `%20`, which is the only way RFC 6068
    // spells a space.
    assert_eq!(prefill.subject, "C++ vs Rust");
    // And a raw `+` stays a `+`, form-encoding would have made these three spaces.
    assert_eq!(
        parse("mailto:a@example.test?subject=a+b+c").subject,
        "a+b+c"
    );
}

#[test]
fn only_the_five_safe_headers_are_honoured() {
    // RFC 6068 §6.1: a URI may name any header. Honouring `from` would let a link forge who
    // the message appears to come from; `reply-to` redirects the answer; `in-reply-to` grafts
    // it onto someone else's thread. All are dropped, and the safe fields still come through.
    let prefill = parse(
        "mailto:alice@example.test?from=spoof@evil.test&reply-to=spoof@evil.test\
         &in-reply-to=%3Cx@evil.test%3E&content-type=text/html&x-priority=1&subject=Real",
    );
    assert_eq!(prefill.to, "alice@example.test");
    assert_eq!(prefill.subject, "Real");
    assert_eq!(prefill.cc, "");
    assert_eq!(prefill.bcc, "");
    assert_eq!(prefill.body, "");
}

#[test]
fn an_encoded_ampersand_cannot_inject_another_field() {
    // The query is split into fields BEFORE decoding. Were it the other way round, the `%26`
    // below would become a real `&` and hand the link a Bcc recipient the user never saw in
    // the composer; silent carbon-copying is exactly the abuse this ordering prevents.
    let prefill = parse("mailto:alice@example.test?subject=Invoice%26bcc%3Dsnoop@evil.test");
    assert_eq!(prefill.subject, "Invoice&bcc=snoop@evil.test");
    assert_eq!(prefill.bcc, "");
}

#[test]
fn crlf_in_a_subject_cannot_inject_a_header() {
    // %0D%0A in a subject is the classic header-injection payload: unfiltered it would end the
    // Subject header and start a Bcc one when the draft is assembled.
    let prefill = parse("mailto:alice@example.test?subject=Hi%0D%0ABcc:%20snoop@evil.test");
    assert!(!prefill.subject.contains('\r'));
    assert!(!prefill.subject.contains('\n'));
    assert_eq!(prefill.subject, "HiBcc: snoop@evil.test");
    assert_eq!(prefill.bcc, "");
}

#[test]
fn an_address_carrying_a_newline_or_brackets_is_dropped() {
    // A recipient field is single-line by construction, so anything that could break out of it
    // is not "cleaned up" into a different address; it is discarded, and the good ones remain.
    let prefill = parse(
        "mailto:alice@example.test,bad%0D%0Ax@evil.test,%3Cbob@example.test%3E,\
         no-at-sign,bob@example.test",
    );
    assert_eq!(prefill.to, "alice@example.test, bob@example.test");
}

#[test]
fn a_body_keeps_its_line_breaks_and_normalizes_them() {
    // Newlines are content in a body (unlike a subject), so they survive, but CRLF, CR, and
    // LF all collapse to one `\n` so the client seeds the same paragraphs either way.
    let prefill = parse("mailto:a@example.test?body=One%0D%0ATwo%0AThree%0DFour%00");
    assert_eq!(prefill.body, "One\nTwo\nThree\nFour");
}

#[test]
fn a_repeated_subject_or_body_keeps_the_first() {
    // Appending `&subject=…` to a link someone else wrote must not silently replace what the
    // user was shown by the original.
    let prefill = parse("mailto:a@example.test?subject=Real&subject=Evil&body=First&body=Second");
    assert_eq!(prefill.subject, "Real");
    assert_eq!(prefill.body, "First");
}

#[test]
fn non_ascii_and_malformed_encoding_survive() {
    // UTF-8 percent-encoding is the RFC 6068 norm for a non-ASCII subject.
    assert_eq!(
        parse("mailto:a@example.test?subject=%C3%A9%C3%A9n").subject,
        "één"
    );
    // A stray `%` is kept verbatim, and an invalid UTF-8 byte becomes U+FFFD, rather than
    // either one throwing the whole link away.
    assert_eq!(
        parse("mailto:a@example.test?subject=100%%20done").subject,
        "100% done"
    );
    assert!(
        parse("mailto:a@example.test?subject=%FF")
            .subject
            .contains('\u{FFFD}')
    );
}

#[test]
fn a_field_without_a_value_is_skipped() {
    let prefill = parse("mailto:a@example.test?subject&cc=&body=Hi");
    assert_eq!(prefill.subject, "");
    assert_eq!(prefill.cc, "");
    assert_eq!(prefill.body, "Hi");
}

#[test]
fn the_debug_impl_redacts_the_message() {
    // A `mailto:` URI is entirely content; recipients, subject, and body. Its Debug must stay
    // safe to put in a diagnostic log (docs/logging.md), so it reports lengths, never values.
    let prefill = parse("mailto:alice@example.test?subject=Secret&body=Confidential");
    let rendered = format!("{prefill:?}");
    assert!(!rendered.contains("alice"));
    assert!(!rendered.contains("Secret"));
    assert!(!rendered.contains("Confidential"));
    assert!(rendered.contains("subject_len: 6"));
}
