//! What preparing a stored account builds before anything dials.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::AccountId;
use engine_provider::{CredentialSource, CredentialSubject};
use mailcal_account::{CredentialOrigin, TokenSink};
use mailcal_oauth::GrantedScopes;

use super::prepare_stored_account;
use crate::ConnectedAccount;

/// A sink that keeps nothing: nothing here refreshes.
struct NoSink;

#[async_trait]
impl TokenSink for NoSink {
    async fn refresh_token_rotated(&self, _account: &AccountId, _new_refresh_token: &str) {}

    async fn scopes_granted(&self, _account: &AccountId, _granted: &GrantedScopes) {}
}

/// An OAuth JMAP account whose id is pinned, so it differs from the one its settings derive.
const PINNED_OAUTH_JMAP: &str = r#"
id = "pinned-jmap-account"

[jmap]
email = "you@example.net"
base_url = "https://mail.example.net"

[jmap.oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://jmap-oauth"
scopes = ["offline_access"]
"#;

/// The token source every adapter of the account asks signs in as the account's id, the subject
/// a stored secret signs in under too, so everything the engine keys by subject is the account's.
#[test]
fn an_oauth_jmap_accounts_token_source_signs_in_as_the_account() {
    let sink: Arc<dyn TokenSink> = Arc::new(NoSink);
    let prepared = prepare_stored_account(PINNED_OAUTH_JMAP, &sink, CredentialOrigin::Stored)
        .expect("a well-formed config prepares");
    let ConnectedAccount::Jmap {
        config,
        tokens: Some(tokens),
        ..
    } = &prepared.connected
    else {
        panic!("an OAuth JMAP account with its token source");
    };

    let id = config.account_id().expect("an id");
    assert_eq!(id.as_str(), "pinned-jmap-account");
    assert_eq!(tokens.subject(), &CredentialSubject::new(id.as_str()));
}
