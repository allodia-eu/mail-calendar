//! Tests for the one verdict a connect failure carries out of this crate: whether the server
//! **refused the account's credential**, which raises the "sign in again" prompt, or merely could
//! not be reached, which badges an outage (`docs/provider-oauth.md` rule 12).
//!
//! Both directions are load-bearing. A verdict that reads only an OAuth-shaped variant tells a user
//! whose IMAP password stopped working that their *server* is unreachable, on every launch, and
//! never points at the field that would fix it. A verdict that fires on any refusal at all is the
//! opposite failure: servers do refuse a valid credential, so a refusal a sibling login contradicts
//! must move nothing.

use engine_core::error::FailureClass;
use provider_imap::ImapError;
use provider_jmap::JmapError;

use super::AccountError;

/// What Dovecot answers a refused `LOGIN`, verbatim.
fn refused_login() -> ImapError {
    ImapError::Auth("[AUTHENTICATIONFAILED] Authentication failed.".to_owned())
}

/// A JMAP HTTP status failure. Built variant-first because the engine's own constructor is
/// crate-private.
fn jmap_status(status: u16) -> JmapError {
    JmapError::Status {
        status,
        body: "{}".to_owned(),
    }
}

#[test]
fn a_refused_first_imap_login_is_a_rejected_signin() {
    assert!(matches!(
        AccountError::from_first_imap_login(refused_login()),
        AccountError::SigninRejected(_)
    ));
}

#[test]
fn a_rejected_signin_still_carries_what_the_server_said() {
    // The prompt names no cause, so this string is all a support log has left of the refusal.
    let rendered = AccountError::from_first_imap_login(refused_login()).to_string();
    assert!(
        rendered.starts_with("sign-in rejected:"),
        "not rendered as a refusal: {rendered}"
    );
    assert!(
        rendered.contains("AUTHENTICATIONFAILED"),
        "the server's own words were dropped: {rendered}"
    );
}

#[test]
fn a_first_login_that_never_got_an_answer_is_not_a_rejected_signin() {
    // A timeout is the outage this prompt must not swallow: the credential is unproven either way.
    let err = ImapError::Io(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out",
    ));
    assert!(matches!(
        AccountError::from_first_imap_login(err),
        AccountError::Imap(_)
    ));
}

#[test]
fn a_first_login_refused_on_other_grounds_keeps_its_own_variant() {
    for err in [
        ImapError::No("SELECT nonexistent".to_owned()),
        ImapError::Bad("bad command".to_owned()),
        ImapError::Protocol("not a FETCH".to_owned()),
    ] {
        let class = err.failure_class();
        assert!(
            matches!(
                AccountError::from_first_imap_login(err),
                AccountError::Imap(_)
            ),
            "a {class:?} failure was read as a refused credential"
        );
    }
}

/// The corroboration half of rule 12, at the type level: the folder loop of an IMAP dial converts
/// with plain `?`, and that conversion may never produce the verdict: the INBOX login
/// authenticated seconds earlier with the same password, so the refusal is the server contradicting
/// itself and a prompt over it is a false prompt.
#[test]
fn a_refusal_converted_the_ordinary_way_is_never_a_rejected_signin() {
    assert!(matches!(
        AccountError::from(refused_login()),
        AccountError::Imap(_)
    ));
}

#[test]
fn a_jmap_401_is_a_rejected_signin() {
    assert_eq!(
        jmap_status(401).failure_class(),
        FailureClass::Authentication,
        "the engine no longer classifies a 401 as an authentication failure"
    );
    assert!(matches!(
        AccountError::from_jmap_connect(&jmap_status(401)),
        AccountError::SigninRejected(_)
    ));
}

#[test]
fn a_jmap_connect_that_failed_some_other_way_is_not_a_rejected_signin() {
    for err in [jmap_status(503), JmapError::Session("no apiUrl".to_owned())] {
        let class = err.failure_class();
        assert!(
            matches!(AccountError::from_jmap_connect(&err), AccountError::Jmap(_)),
            "a {class:?} failure was read as a refused credential"
        );
    }
}
