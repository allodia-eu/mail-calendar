//! The account's token source as the engine asks it: who it signs in as, what it presents, what
//! it replaces, and how a failed refresh is classified.

use std::sync::atomic::Ordering;

use engine_core::{error::FailureClass, ids::AccountId};
use engine_provider::{
    Credential, CredentialProtocol, CredentialRequest, CredentialSource, CredentialSubject, Origin,
};

use crate::graph::test_support::{
    SIGN_IN_NAME, dead_grant_token_endpoint, mock_token_endpoint, ratcheting_token_endpoint,
    refused_token_endpoint, source_at, source_for,
};

fn imap() -> Origin {
    Origin::new("imaps", "imap.example.net", 993)
}

fn request(origin: &Origin) -> CredentialRequest<'_> {
    CredentialRequest::new(CredentialProtocol::Imap, origin)
}

fn token(credential: &Credential) -> &str {
    match credential {
        Credential::Bearer { token, .. } => token.expose(),
        _ => panic!("expected a token credential"),
    }
}

#[tokio::test]
async fn a_dial_presents_the_current_token_under_the_name_the_account_signs_in_as() {
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let source = source_at(endpoint, None);
    let origin = imap();

    let credential = source.credential(&request(&origin)).await.unwrap();
    assert_eq!(credential.username(), Some(SIGN_IN_NAME));
    assert_eq!(token(&credential), "AT-1");
}

#[test]
fn the_source_signs_in_as_the_account_it_was_built_for() {
    let account = AccountId::try_from("subject@example.net@graph.microsoft.com").unwrap();
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let source = source_for(account.clone(), endpoint, None);

    assert_eq!(
        source.subject(),
        &CredentialSubject::new(account.as_str()),
        "the engine keys everything authenticated by the account id"
    );
}

/// The server's refusal is the one piece of news the cached expiry cannot know, so a
/// replacement refreshes although the token has most of its hour left.
#[tokio::test]
async fn a_refused_token_is_replaced_with_a_fresh_one() {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let source = source_at(endpoint, None);
    let origin = imap();

    let refused = source.credential(&request(&origin)).await.unwrap();
    let replaced = source
        .replace(&refused, &request(&origin))
        .await
        .unwrap()
        .expect("a replacement");
    assert_eq!(token(&replaced), "AT-2");
    assert_eq!(replaced.username(), Some(SIGN_IN_NAME));
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/// A token endpoint may answer a refresh with the token it issued last; handing that back would
/// only have the server refuse it a second time.
#[tokio::test]
async fn a_refresh_that_answers_the_refused_token_again_is_no_replacement() {
    let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT-SAME"}"#;
    let (endpoint, hits) = mock_token_endpoint(vec![body.to_owned(); 2]);
    let source = source_at(endpoint, None);
    let origin = imap();

    let refused = source.credential(&request(&origin)).await.unwrap();
    assert_eq!(token(&refused), "AT-SAME");
    assert!(
        source
            .replace(&refused, &request(&origin))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2, "the refresh was asked");
}

#[tokio::test]
async fn anything_but_a_token_is_never_replaced() {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let source = source_at(endpoint, None);
    let origin = imap();

    let password = Credential::password("you@example.net", "hunter2");
    assert!(
        source
            .replace(&password, &request(&origin))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0, "nothing was refreshed");
}

/// A dead grant must become "sign in again", never a retry: retrying presents a refresh
/// token the server has already refused. The token endpoint's own words stay out of the error.
#[tokio::test]
async fn a_dead_grant_is_an_authentication_failure_without_the_servers_text() {
    let (endpoint, _) = dead_grant_token_endpoint();
    let source = source_at(endpoint, None);
    let origin = imap();

    let err = source
        .credential(&request(&origin))
        .await
        .expect_err("the grant is dead");
    assert_eq!(err.class(), FailureClass::Authentication);
    let detail = err.to_string();
    assert!(
        !detail.contains("invalid_grant") && !detail.contains("revoked"),
        "{detail}"
    );
}

/// A token endpoint out of reach says nothing about the account, so nobody is asked to sign in.
#[tokio::test]
async fn an_unreachable_token_endpoint_is_retryable() {
    let source = source_at(refused_token_endpoint(), None);
    let origin = imap();

    let err = source
        .credential(&request(&origin))
        .await
        .expect_err("nothing answers");
    assert_eq!(err.class(), FailureClass::Retryable);
}
