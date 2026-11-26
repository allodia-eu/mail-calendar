//! Which source an IMAP account's connections ask: the account's own token source when it signs
//! in with OAuth, its stored password otherwise, both under the account's id.

use std::sync::Arc;

use engine_provider::{Credential, CredentialProtocol, CredentialRequest, Origin};

use super::*;
use crate::graph::test_support::{ratcheting_token_endpoint, source_for};

const OAUTH: &str = "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\n\
     [oauth]\nclient_id=\"c\"\nrefresh_token=\"r\"\n\
     authorize_endpoint=\"https://h/a\"\ntoken_endpoint=\"https://h/t\"\n\
     redirect_uri=\"x://y\"\nscopes=[]\n";

const PASSWORD: &str =
    "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"hunter2\"\n";

#[test]
fn an_oauth_account_without_its_token_source_is_refused() {
    let account: AccountConfig = toml::from_str(OAUTH).expect("valid config");
    assert!(matches!(
        imap_credential_source(&account, None),
        Err(AccountError::MissingCredential(NO_TOKEN_SOURCE))
    ));
}

/// The mailbox asks the very source the account's other adapters ask, so a token one of them saw
/// refused is replaced once for all of them.
#[test]
fn an_oauth_account_dials_with_its_own_token_source() {
    let account: AccountConfig = toml::from_str(OAUTH).expect("valid config");
    let (endpoint, _) = ratcheting_token_endpoint("r");
    let tokens = source_for(account.account_id().unwrap(), endpoint, None);

    let source = imap_credential_source(&account, Some(&tokens)).unwrap();
    assert!(
        std::ptr::addr_eq(Arc::as_ptr(&source), Arc::as_ptr(&tokens)),
        "the account's one source, not a wrapper around it"
    );
}

#[tokio::test]
async fn a_password_account_presents_its_password_under_the_account_id() {
    let account: AccountConfig = toml::from_str(PASSWORD).expect("valid config");
    let source = imap_credential_source(&account, None).unwrap();

    assert_eq!(
        source.subject(),
        &CredentialSubject::new(account.account_id().unwrap().as_str())
    );
    let origin = Origin::new("imaps", "h", 993);
    let request = CredentialRequest::new(CredentialProtocol::Imap, &origin);
    let credential = source.credential(&request).await.unwrap();
    assert!(matches!(
        &credential,
        Credential::Password { username, password } if username == "u" && password.expose() == "hunter2"
    ));
    assert!(
        source
            .replace(&credential, &request)
            .await
            .unwrap()
            .is_none(),
        "the same password would get the same answer"
    );
}
