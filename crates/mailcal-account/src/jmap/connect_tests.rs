//! A JMAP account's providers over its credential source: an OAuth account's provider is built
//! once and asks the account's token source on every request, and a stored secret signs in as
//! the account.

use std::sync::{Arc, atomic::Ordering};

use engine_core::{
    error::FailureClass,
    ids::{AccountId, SharedAccountId},
};
use engine_provider::{
    Credential, CredentialProtocol, CredentialRequest, CredentialSubject, Origin,
};

use super::*;
use crate::{
    InsecureCause,
    graph::test_support::{
        dead_grant_token_endpoint, flaky_token_endpoint, ratcheting_token_endpoint,
        refused_token_endpoint, source_at,
    },
    imap_credentials::NO_TOKEN_SOURCE,
    jmap::{
        NO_STORED_SECRET,
        fake_server::{BOOK, FakeJmap, SHARED, account_id},
    },
    load_jmap_str,
};

/// An account signed in through its provider, at `base_url`.
fn oauth_account(base_url: &str) -> JmapAccountConfig {
    load_jmap_str(&format!(
        r#"
[jmap]
email = "you@example.net"
base_url = "{base_url}"

[jmap.oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://jmap-oauth"
scopes = ["offline_access"]
"#
    ))
    .unwrap()
}

/// An account that stores `secret` (`password = "…"` or `token = "…"`), at `base_url`.
fn stored_account(base_url: &str, secret: &str) -> JmapAccountConfig {
    load_jmap_str(&format!(
        "[jmap]\nemail = \"you@example.net\"\nbase_url = \"{base_url}\"\n{secret}\n"
    ))
    .unwrap()
}

/// The id a call names; JMAP answers for the account the session names, whatever it is.
fn any_account() -> AccountId {
    AccountId::try_from("you@example.net").unwrap()
}

fn presented(server: &FakeJmap) -> Vec<String> {
    server
        .seen()
        .into_iter()
        .map(|seen| seen.authorization)
        .collect()
}

#[tokio::test]
async fn an_oauth_account_presents_its_current_token_and_after_a_refusal_the_replacement() {
    let server = FakeJmap::start();
    let (endpoint, minted) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let provider =
        connect_jmap_calendar_provider(&oauth_account(server.url()), Some(&tokens), None)
            .await
            .unwrap();

    provider.sync_calendars(&any_account(), None).await.unwrap();
    server.revoke("Bearer AT-1");
    provider
        .sync_calendars(&any_account(), None)
        .await
        .expect("the replacement is accepted");
    provider.sync_calendars(&any_account(), None).await.unwrap();

    assert_eq!(
        presented(&server),
        [
            "Bearer AT-1",
            "Bearer AT-1",
            "Bearer AT-1",
            "Bearer AT-2",
            "Bearer AT-2"
        ],
        "the session, a request, the refused one, its replay, and the next request"
    );
    assert_eq!(
        server.discoveries(),
        1,
        "the provider is built once and never discovers its session again"
    );
    assert_eq!(minted.load(Ordering::SeqCst), 2);
}

/// Connecting proves the grant works: a dead one is "sign in again" now, not at the first sync.
#[tokio::test]
async fn a_dead_grant_fails_the_connect_as_a_refused_sign_in_and_sends_nothing() {
    let server = FakeJmap::start();
    let (endpoint, _) = dead_grant_token_endpoint();
    let tokens = source_at(endpoint, None);

    let Err(err) =
        connect_jmap_mail_providers(&oauth_account(server.url()), Some(&tokens), None).await
    else {
        panic!("a dead grant connected");
    };
    assert!(matches!(err, AccountError::SigninRejected(_)), "{err:?}");
    assert!(server.seen().is_empty(), "{:?}", server.seen());
}

/// A token endpoint that cannot be reached says nothing about the grant, so it never asks the
/// person to sign in again.
#[tokio::test]
async fn an_unreachable_token_endpoint_fails_the_connect_without_a_refused_sign_in() {
    for endpoint in [refused_token_endpoint(), flaky_token_endpoint(usize::MAX).0] {
        let server = FakeJmap::start();
        let tokens = source_at(endpoint, None);

        let Err(err) =
            connect_jmap_mail_providers(&oauth_account(server.url()), Some(&tokens), None).await
        else {
            panic!("connected without a token");
        };
        assert!(matches!(err, AccountError::Jmap(_)), "{err:?}");
        assert!(server.seen().is_empty(), "{:?}", server.seen());
    }
}

/// One refusal is worth one replacement: a refused replacement is the account's sign-in failing,
/// and asking for a third token would be one more refresh the server answers the same way.
#[tokio::test]
async fn a_refused_replacement_is_an_authentication_failure_after_one_refresh() {
    let server = FakeJmap::start();
    let (endpoint, minted) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let provider =
        connect_jmap_calendar_provider(&oauth_account(server.url()), Some(&tokens), None)
            .await
            .unwrap();

    server.revoke("Bearer AT-1");
    server.revoke("Bearer AT-2");
    let err = provider
        .sync_calendars(&any_account(), None)
        .await
        .unwrap_err();

    assert_eq!(err.class(), FailureClass::Authentication, "{err}");
    assert_eq!(minted.load(Ordering::SeqCst), 2);
    assert_eq!(
        presented(&server),
        ["Bearer AT-1", "Bearer AT-1", "Bearer AT-2"]
    );
}

/// A JMAP adapter advertises no write destination until it is bound to a book, so losing the
/// binding would turn the account's contacts read-only.
#[tokio::test]
async fn the_contact_book_stays_bound_after_a_replaced_token() {
    let server = FakeJmap::start();
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let providers = connect_jmap_contact_providers(&oauth_account(server.url()), Some(&tokens))
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one contacts provider");
    };

    server.revoke("Bearer AT-1");
    provider
        .sync_address_books(&any_account(), None)
        .await
        .unwrap();

    let destination = provider.contact_destination().expect("a write destination");
    assert_eq!(destination.address_book.as_str(), BOOK);
    assert_eq!(presented(&server).last().unwrap(), "Bearer AT-2");
}

#[tokio::test]
async fn a_shared_mailbox_stays_bound_after_a_replaced_token() {
    let server = FakeJmap::start();
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let shared = SharedAccountId::try_from(SHARED).unwrap();
    let provider =
        connect_jmap_calendar_provider(&oauth_account(server.url()), Some(&tokens), Some(&shared))
            .await
            .unwrap();

    server.revoke("Bearer AT-1");
    provider.sync_calendars(&any_account(), None).await.unwrap();

    let calls: Vec<String> = server.seen().iter().filter_map(account_id).collect();
    assert_eq!(calls, [SHARED, SHARED], "the refused call and its replay");
}

/// The source every adapter of the account shares, not a wrapper around it.
#[test]
fn an_oauth_account_asks_its_own_token_source() {
    let account = oauth_account("https://mail.example.net");
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);

    let id = account.account_id().unwrap();
    let source = account.credential_source(&id, Some(&tokens)).unwrap();
    assert!(std::ptr::addr_eq(
        Arc::as_ptr(&source),
        Arc::as_ptr(&tokens)
    ));
}

#[test]
fn an_oauth_account_without_its_token_source_is_refused() {
    let account = oauth_account("https://mail.example.net");
    let id = account.account_id().unwrap();
    assert!(matches!(
        account.credential_source(&id, None),
        Err(AccountError::MissingCredential(NO_TOKEN_SOURCE))
    ));
}

/// Signing in as the account's id, as its other adapters do, so everything the engine keys by
/// subject is the account's.
#[tokio::test]
async fn a_stored_password_signs_in_as_the_account_and_is_never_replaced() {
    let account = stored_account("https://mail.example.net", "password = \"hunter2\"");
    let id = account.account_id().unwrap();
    let source = account.credential_source(&id, None).unwrap();

    assert_eq!(source.subject(), &CredentialSubject::new(id.as_str()));
    let origin = Origin::new("https", "mail.example.net", 443);
    let request = CredentialRequest::new(CredentialProtocol::Http, &origin);
    let credential = source.credential(&request).await.unwrap();
    assert!(matches!(
        &credential,
        Credential::Password { username, password }
            if username == "you@example.net" && password.expose() == "hunter2"
    ));
    assert!(
        source
            .replace(&credential, &request)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_stored_api_token_signs_in_as_the_account() {
    let account = stored_account("https://mail.example.net", "token = \"api-token\"");
    let id = account.account_id().unwrap();
    let source = account.credential_source(&id, None).unwrap();

    assert_eq!(source.subject(), &CredentialSubject::new(id.as_str()));
    let origin = Origin::new("https", "mail.example.net", 443);
    let request = CredentialRequest::new(CredentialProtocol::Http, &origin);
    assert!(matches!(
        source.credential(&request).await.unwrap(),
        Credential::Bearer { token, .. } if token.expose() == "api-token"
    ));
}

#[tokio::test]
async fn a_stored_password_connects_with_it() {
    let server = FakeJmap::start();
    let account = stored_account(server.url(), "password = \"hunter2\"");

    connect_jmap_mail_providers(&account, None, None)
        .await
        .unwrap();

    assert_eq!(
        presented(&server),
        ["Basic eW91QGV4YW1wbGUubmV0Omh1bnRlcjI="]
    );
}

/// A login that cannot succeed is never sent: a provider may count it towards a lockout.
#[tokio::test]
async fn an_account_that_stores_no_secret_is_refused_before_anything_is_sent() {
    let server = FakeJmap::start();
    let account = stored_account(server.url(), "");

    let Err(err) = connect_jmap_mail_providers(&account, None, None).await else {
        panic!("an account with no secret connected");
    };
    assert!(
        matches!(err, AccountError::MissingCredential(NO_STORED_SECRET)),
        "{err:?}"
    );
    assert!(server.seen().is_empty(), "{:?}", server.seen());
}

/// A stored API token a header cannot carry is refused before anything is sent; it never goes out
/// as stored, so the person is asked to sign in again, as after a refused one.
#[tokio::test]
async fn a_stored_token_a_header_cannot_carry_is_a_rejected_signin() {
    let server = FakeJmap::start();
    let account = stored_account(server.url(), "token = \"api token\"");

    let Err(err) = connect_jmap_mail_providers(&account, None, None).await else {
        panic!("a token a header cannot carry connected");
    };
    assert!(matches!(err, AccountError::SigninRejected(_)), "{err:?}");
    assert!(server.seen().is_empty(), "{:?}", server.seen());
}

/// No sign-in goes over plaintext to a server off this device, so the connect says the server is
/// not reached securely, and nothing is sent.
#[tokio::test]
async fn a_plaintext_server_off_this_device_is_an_insecure_server() {
    let account = stored_account("http://mail.example.invalid", "password = \"hunter2\"");

    let Err(err) = connect_jmap_mail_providers(&account, None, None).await else {
        panic!("a plaintext server connected");
    };
    assert!(
        matches!(
            err,
            AccountError::InsecureServer {
                cause: InsecureCause::PlaintextAddress,
                ..
            }
        ),
        "{err:?}"
    );
}
