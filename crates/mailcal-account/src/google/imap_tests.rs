//! The Gmail IMAP side's decisions: when it opens, when it gives way to the API, which sign-in
//! it dials with, and when its mail cannot be reached.

use engine_core::error::FailureClass;
use engine_provider::ProviderError;
use provider_imap::ImapError;

use super::{GmailImapSlot, GmailMailRoute, Opened, REFUSALS_BEFORE_API, Side, decide};

fn failed(class: FailureClass) -> engine_provider::ProviderResult<Opened> {
    Err(ProviderError::new(class, "no"))
}

/// A `NO` Gmail calls temporary, as the engine hands it on.
fn refused_for_now() -> ProviderError {
    ProviderError::from(ImapError::No("[UNAVAILABLE] Try again later".to_owned()))
}

/// A connection that died under a read, as the engine hands it on.
fn lost() -> ProviderError {
    ProviderError::from(ImapError::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "broken pipe",
    )))
}

#[test]
fn a_lasting_no_moves_the_account_to_the_api() {
    let (side, answer) = decide(failed(FailureClass::InvalidState), 0);
    assert!(matches!(side, Side::Api));
    assert!(matches!(answer, Ok(None)));
}

#[test]
fn a_server_without_the_gmail_extension_moves_it_at_once() {
    let (side, answer) = decide(Ok(Opened::NotGmail), 0);
    assert!(matches!(side, Side::Api));
    assert!(matches!(answer, Ok(None)));
}

/// A hidden All Mail is a lasting `NO`; one Gmail calls temporary is a server short of something
/// for now, and moving the account would hand an IMAP cursor to the API pass.
#[test]
fn a_refusal_gmail_calls_temporary_fails_the_read_and_moves_nothing() {
    let (side, answer) = decide(Err(refused_for_now()), 0);
    assert!(matches!(side, Side::Unknown(0)));
    assert_eq!(answer.unwrap_err().class(), FailureClass::Retryable);
}

#[test]
fn a_refused_login_moves_it_only_once_the_refusal_has_repeated() {
    let mut refused = 0;
    for _ in 1..REFUSALS_BEFORE_API {
        let (side, answer) = decide(failed(FailureClass::Authentication), refused);
        let Side::Unknown(count) = side else {
            panic!("one refusal may be a connection limit, not IMAP turned off");
        };
        assert_eq!(answer.unwrap_err().class(), FailureClass::Authentication);
        refused = count;
    }
    let (side, answer) = decide(failed(FailureClass::Authentication), refused);
    assert!(matches!(side, Side::Api));
    assert!(matches!(answer, Ok(None)));
}

#[test]
fn a_failure_that_may_pass_is_tried_again_and_counts_no_refusal() {
    for class in [
        FailureClass::Retryable,
        FailureClass::RateLimited,
        FailureClass::Permanent,
        FailureClass::Conflict,
    ] {
        let (side, answer) = decide(failed(class), 1);
        assert!(matches!(side, Side::Unknown(1)), "{class:?}");
        assert_eq!(answer.unwrap_err().class(), class);
    }
}

/// A token source of its own, as each sign-in builds: signing in again builds a new one
/// beside the old one, which stays alive until the account's old providers are replaced.
fn sign_in() -> std::sync::Arc<crate::GraphTokenSource> {
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    let provider = OAuthProviderConfig::google(
        "google-client",
        None,
        "com.googleusercontent.apps.google-client:/oauth2redirect",
        GOOGLE_SCOPES,
    );
    crate::GraphTokenSource::from_parts(
        OAuthClient::new(provider).unwrap(),
        engine_core::ids::AccountId::try_from("alice@gmail.com@mail.google.com").unwrap(),
        "refresh".to_owned(),
        None,
        "google",
        crate::CredentialOrigin::FreshSignIn,
    )
}

/// Signing in again replaces the account's providers only after the new ones have connected,
/// so the old ones, and the slot they hold, are still alive when the new ones ask for theirs.
/// A slot that kept the old sign-in would dial every new IMAP connection with a grant the
/// user just replaced, which is the grant that was refused.
#[test]
fn signing_in_again_gives_the_account_a_slot_with_the_new_sign_in() {
    let tls = crate::tls::tls_with(&[]).unwrap();
    let address = "signed-in-again@gmail.com";
    let old = sign_in();
    let replaced = GmailImapSlot::shared(address, &old, &tls);

    let new = sign_in();
    let slot = GmailImapSlot::shared(address, &new, &tls);
    assert!(
        std::sync::Arc::ptr_eq(&slot.tokens, &new),
        "the slot dials IMAP with the sign-in it was given, not the one it replaced",
    );
    assert!(std::sync::Arc::ptr_eq(&replaced.tokens, &old));

    // The account's other providers, built from the same sign-in, share the new slot.
    let again = GmailImapSlot::shared(address, &new, &tls);
    assert!(std::sync::Arc::ptr_eq(&slot, &again));
}

/// Each core in the process draws its own Settings, so each hears a route change, and one
/// that has gone stops being asked.
#[test]
fn every_listener_hears_a_route_change_until_it_is_gone() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let gone = Arc::new(AtomicBool::new(false));
    let (heard, alive) = (Arc::clone(&first), Arc::clone(&gone));
    super::on_gmail_mail_route_change(move || {
        heard.fetch_add(1, Ordering::SeqCst);
        !alive.load(Ordering::SeqCst)
    });
    let heard = Arc::clone(&second);
    super::on_gmail_mail_route_change(move || {
        heard.fetch_add(1, Ordering::SeqCst);
        true
    });

    super::route_changed();
    assert_eq!(first.load(Ordering::SeqCst), 1);
    assert_eq!(second.load(Ordering::SeqCst), 1);

    gone.store(true, Ordering::SeqCst);
    super::route_changed();
    super::route_changed();
    assert_eq!(
        first.load(Ordering::SeqCst),
        2,
        "a listener that is gone is asked once more"
    );
    assert_eq!(second.load(Ordering::SeqCst), 3);
}

/// A network that lets HTTPS through and blocks IMAP fails every mail read while the account's
/// API calls succeed; two in a row say so, and the first read that gets through takes it back.
#[test]
fn two_reads_that_cannot_reach_imap_make_its_mail_unreachable_until_one_does() {
    let tls = crate::tls::tls_with(&[]).unwrap();
    let address = "unreached@gmail.com";
    let slot = GmailImapSlot::shared(address, &sign_in(), &tls);

    slot.note_read(Err(&lost()));
    assert_eq!(
        slot.route(),
        GmailMailRoute::Undecided,
        "one blip is not an outage"
    );
    slot.note_read(Err(&lost()));
    assert_eq!(slot.route(), GmailMailRoute::Unreachable);
    assert_eq!(
        super::gmail_mail_route(address),
        GmailMailRoute::Unreachable
    );

    slot.note_read(Ok(()));
    assert_eq!(slot.route(), GmailMailRoute::Undecided);

    // A server that answered, even with a refusal, ends the run.
    slot.note_read(Err(&lost()));
    slot.note_read(Err(&ProviderError::authentication("no")));
    slot.note_read(Err(&lost()));
    assert_eq!(slot.route(), GmailMailRoute::Undecided);
}

/// A refusal Gmail calls temporary is a failure that may pass, like a lost connection, but the
/// network carried it, so it is no sign that IMAP is blocked.
#[test]
fn a_refusal_gmail_calls_temporary_is_no_sign_its_mail_is_unreachable() {
    let tls = crate::tls::tls_with(&[]).unwrap();
    let slot = GmailImapSlot::shared("refused-for-now@gmail.com", &sign_in(), &tls);

    slot.note_read(Err(&refused_for_now()));
    slot.note_read(Err(&refused_for_now()));
    assert_eq!(slot.route(), GmailMailRoute::Undecided);

    slot.note_read(Err(&lost()));
    slot.note_read(Err(&refused_for_now()));
    slot.note_read(Err(&lost()));
    assert_eq!(slot.route(), GmailMailRoute::Undecided);
}

/// An account on the API reads nothing over IMAP; its outage is the account's own, which the
/// sync already reports.
#[test]
fn reads_over_the_api_never_make_its_mail_unreachable() {
    let tls = crate::tls::tls_with(&[]).unwrap();
    let slot = GmailImapSlot::shared("on-the-api@gmail.com", &sign_in(), &tls);
    slot.route.store(
        GmailMailRoute::Api as u8,
        std::sync::atomic::Ordering::Release,
    );
    for _ in 0..3 {
        slot.note_read(Err(&lost()));
    }
    assert_eq!(slot.route(), GmailMailRoute::Api);
}
