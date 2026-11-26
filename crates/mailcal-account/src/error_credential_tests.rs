//! What a first connect makes of a credential the engine would not present: nothing was sent, so
//! no server answered, and what it means depends on why and on what the account signs in with.

use engine_core::error::FailureClass;
use engine_provider::{
    Credential, CredentialFailure, CredentialProtocol, CredentialRequest, CredentialSource,
    CredentialSubject, Origin, ProviderError,
};
use mailcal_oauth::DavUse;
use provider_caldav::CalDavError;
use provider_imap::ImapError;
use provider_jmap::JmapError;

use super::{AccountError, SignsIn};
use crate::InsecureCause;

/// A credential that would leave TLS, as the engine refuses a plaintext address off loopback.
fn insecure() -> ProviderError {
    CredentialFailure::refused_origin("the credential would leave TLS").into_error()
}

/// A credential the server led to plaintext from an address the engine accepts, as a redirect
/// from HTTPS to plaintext does.
fn led_to_insecure() -> ProviderError {
    CredentialFailure::led_to_insecure(
        "a redirect would take the credential from HTTPS to plaintext",
    )
    .into_error()
}

/// An origin the source does not permit, as an issuer's rule refuses a host off its domain.
fn not_permitted() -> ProviderError {
    CredentialFailure::not_permitted("the credential source does not permit this origin")
        .into_error()
}

/// A credential the protocol cannot carry, as the engine refuses a Basic username holding `:` or
/// a password holding a line break.
fn unpresentable() -> ProviderError {
    CredentialFailure::unpresentable("the credential cannot be presented").into_error()
}

/// A source answering for another account: a fault in how the account was built, which a new
/// sign-in does not mend.
fn another_account() -> ProviderError {
    CredentialFailure::another_account("another account").into_error()
}

/// A source that fails as `failure` does, through the engine's own ask, which marks the failure
/// as the source's.
async fn source_failing(failure: ProviderError) -> ProviderError {
    #[derive(Debug)]
    struct Failing(Option<ProviderError>, CredentialSubject);
    #[async_trait::async_trait]
    impl CredentialSource for Failing {
        fn subject(&self) -> &CredentialSubject {
            &self.1
        }

        async fn credential(&self, _: &CredentialRequest<'_>) -> Result<Credential, ProviderError> {
            Err(self.0.as_ref().map_or_else(
                || ProviderError::retryable("asked twice"),
                |failure| failure.reworded(failure.detail()),
            ))
        }
    }
    let source = Failing(Some(failure), CredentialSubject::new("you@example.org"));
    let origin = Origin::new("https", "dav.example.org", 443);
    let request = CredentialRequest::new(CredentialProtocol::Http, &origin);
    engine_provider::with_credential(
        &source,
        &request,
        std::time::Duration::from_secs(5),
        |_: &ProviderError| false,
        |_| async { Ok::<(), ProviderError>(()) },
    )
    .await
    .expect_err("the source fails")
}

/// What each first connect makes of `failure`, for an account signing in `with`: IMAP, JMAP,
/// then the calendar and the address book.
fn verdicts(failure: &ProviderError, with: SignsIn) -> [AccountError; 4] {
    let dav = |failed| {
        let grant = (with == SignsIn::WithGrant).then_some(DavUse::Calendars);
        AccountError::from_first_dav_connect(
            CalDavError::Credential(failure.reworded(failure.detail())),
            failed,
            grant,
        )
    };
    [
        AccountError::from_first_imap_login(
            ImapError::Credential(failure.reworded(failure.detail())),
            with,
        ),
        AccountError::from_jmap_connect(
            &JmapError::Credential(failure.reworded(failure.detail())),
            with,
        ),
        dav(AccountError::CalDav),
        dav(AccountError::CardDav),
    ]
}

/// A server the sign-in would reach only over plaintext is no refused sign-in and no outage: no
/// new password or sign-in clears it, whatever the account signs in with.
#[test]
fn a_credential_that_would_leave_tls_is_an_insecure_server_on_every_protocol() {
    for with in [SignsIn::WithStoredSecret, SignsIn::WithGrant] {
        for verdict in verdicts(&insecure(), with) {
            assert!(
                matches!(verdict, AccountError::InsecureServer { .. }),
                "{with:?}: {verdict}"
            );
        }
    }
}

/// A stored secret the protocol cannot carry never goes out as stored, so it asks the person to
/// sign in again, as a refused password does, on every protocol.
#[test]
fn a_stored_secret_the_protocol_cannot_carry_is_a_rejected_signin() {
    for verdict in verdicts(&unpresentable(), SignsIn::WithStoredSecret) {
        assert!(
            matches!(verdict, AccountError::SigninRejected(_)),
            "{verdict}"
        );
    }
}

/// A grant's token is minted afresh, so a token the protocol cannot carry is no stored secret to
/// change, and a source answering for another account is no refusal at all.
#[test]
fn only_a_stored_secret_refused_for_its_form_is_a_rejected_signin() {
    for (failure, with) in [
        (unpresentable(), SignsIn::WithGrant),
        (another_account(), SignsIn::WithStoredSecret),
        (another_account(), SignsIn::WithGrant),
    ] {
        let [imap, jmap, calendar, contacts] = verdicts(&failure, with);
        assert!(matches!(imap, AccountError::Imap(_)), "{with:?}: {imap}");
        assert!(matches!(jmap, AccountError::Jmap(_)), "{with:?}: {jmap}");
        assert!(
            matches!(calendar, AccountError::CalDav(_)),
            "{with:?}: {calendar}"
        );
        assert!(
            matches!(contacts, AccountError::CardDav(_)),
            "{with:?}: {contacts}"
        );
    }
}

/// A server whose discovery leads where the account's grant may not take its token is one the
/// sign-in does not reach (`docs/mail-oauth.md` rule 10), as a server outside the issuer's hosts
/// is before anything is sent.
#[test]
fn a_hop_the_grant_may_not_reach_needs_a_sign_in_of_its_own() {
    let hop = || CalDavError::Credential(not_permitted());
    assert!(matches!(
        AccountError::from_first_dav_connect(hop(), AccountError::CalDav, Some(DavUse::Calendars),),
        AccountError::DavNeedsOwnSignIn("calendar")
    ));
    assert!(matches!(
        AccountError::from_first_dav_connect(hop(), AccountError::CardDav, Some(DavUse::Contacts),),
        AccountError::DavNeedsOwnSignIn("contacts")
    ));
}

/// An origin a policy refused is a sign-in of its own only for a grant's DAV server: elsewhere it
/// stays the family's own failure.
#[test]
fn an_origin_not_permitted_elsewhere_is_the_familys_own_failure() {
    let [imap, jmap, ..] = verdicts(&not_permitted(), SignsIn::WithGrant);
    assert!(matches!(imap, AccountError::Imap(_)), "{imap}");
    assert!(matches!(jmap, AccountError::Jmap(_)), "{jmap}");
    for verdict in verdicts(&not_permitted(), SignsIn::WithStoredSecret) {
        assert!(
            !matches!(
                verdict,
                AccountError::SigninRejected(_)
                    | AccountError::InsecureServer { .. }
                    | AccountError::DavNeedsOwnSignIn(_)
            ),
            "{verdict}"
        );
    }
}

/// A credential that could not be had asked no server, so no start is gone and nothing is
/// worth finding again.
#[test]
fn a_dav_credential_failure_is_no_gone_start() {
    for failure in [insecure(), not_permitted()] {
        assert_eq!(failure.class(), FailureClass::Permanent);
        let refused = || CalDavError::Credential(failure.reworded(failure.detail()));
        assert!(!AccountError::CalDav(refused()).is_gone());
        assert!(!AccountError::CardDav(refused()).is_gone());
    }
}

/// Why a server is reached only over plaintext is the engine's kind, on every protocol: the
/// account's own address is plaintext (`Insecure`), or the server led the credential there from
/// one the engine accepts (`LedToInsecure`).
#[test]
fn an_insecure_server_carries_the_engines_kind_as_its_cause() {
    let cause = |verdict: &AccountError| match verdict {
        AccountError::InsecureServer { cause, .. } => Some(*cause),
        _ => None,
    };
    for with in [SignsIn::WithStoredSecret, SignsIn::WithGrant] {
        for verdict in verdicts(&insecure(), with) {
            assert_eq!(
                cause(&verdict),
                Some(InsecureCause::PlaintextAddress),
                "{with:?}: {verdict}"
            );
        }
        for verdict in verdicts(&led_to_insecure(), with) {
            assert_eq!(
                cause(&verdict),
                Some(InsecureCause::LedToPlaintext),
                "{with:?}: {verdict}"
            );
        }
    }
}

/// A source that failed keeps its class's meaning, whatever the account signs in with: consent
/// that is gone is a refused sign-in, a source that cannot be reached for now is the family's own
/// failure, which a retry dials again.
#[tokio::test]
async fn a_source_that_failed_is_read_by_its_class() {
    let gone = source_failing(ProviderError::authentication("the grant is gone")).await;
    let down = source_failing(ProviderError::retryable("the token endpoint is down")).await;
    assert!(gone.is_credential_failure() && down.is_credential_failure());
    for with in [SignsIn::WithStoredSecret, SignsIn::WithGrant] {
        for verdict in verdicts(&gone, with) {
            assert!(
                matches!(verdict, AccountError::SigninRejected(_)),
                "{with:?}: {verdict}"
            );
        }
        let [imap, jmap, calendar, contacts] = verdicts(&down, with);
        assert!(matches!(imap, AccountError::Imap(_)), "{imap}");
        assert!(matches!(jmap, AccountError::Jmap(_)), "{jmap}");
        assert!(matches!(calendar, AccountError::CalDav(_)), "{calendar}");
        assert!(matches!(contacts, AccountError::CardDav(_)), "{contacts}");
    }
}
