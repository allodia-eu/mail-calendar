//! Discovery on an OAuth account's calendar and contacts leading off the grant's hosts, through
//! the real connect: the engine refuses to take the token there, and the use needs a sign-in of
//! its own (`docs/mail-oauth.md` rule 10).

use crate::{
    AccountError, InsecureCause, connect_caldav, connect_carddav_contact_providers,
    graph::test_support::{mock_token_endpoint, source_at},
    http_fake::{self, Reply},
    load_str,
};

/// A server whose every answer names the user's principal on a host the grant does not list.
fn naming_a_principal_elsewhere() -> (String, http_fake::Received) {
    http_fake::serve_with(|_| {
        Reply::Status(
            "207 Multi-Status",
            r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/dav/</d:href>
    <d:propstat>
      <d:prop>
        <d:current-user-principal><d:href>https://dav.example.org/principals/you/</d:href></d:current-user-principal>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#
                .to_owned(),
        )
    })
}

/// An OAuth account whose grant lists only the server at `base`, its calendar and contacts there.
fn signed_in_at(base: &str) -> crate::AccountConfig {
    load_str(&format!(
        r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"

[caldav]
base_url = "{base}/dav/"
username = "you@example.net"

[oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "urn:ietf:params:oauth:scope:mail", "urn:ietf:params:oauth:scope:calendars", "urn:ietf:params:oauth:scope:contacts"]
authorization_server = "https://auth.example.net"
protected_resources = ["{base}"]
"#
    ))
    .unwrap()
}

#[tokio::test]
async fn a_principal_off_the_grants_hosts_needs_a_sign_in_of_its_own() {
    let (base, _) = naming_a_principal_elsewhere();
    let account = signed_in_at(&base);
    let (endpoint, _) = mock_token_endpoint(vec![http_fake::minted(1), http_fake::minted(2)]);
    let tokens = source_at(endpoint, None);

    let calendar = connect_caldav(&account, Some(&tokens)).await.err();
    let contacts = connect_carddav_contact_providers(&account, Some(&tokens))
        .await
        .err();

    assert!(
        matches!(calendar, Some(AccountError::DavNeedsOwnSignIn("calendar"))),
        "{calendar:?}"
    );
    assert!(
        matches!(contacts, Some(AccountError::DavNeedsOwnSignIn("contacts"))),
        "{contacts:?}"
    );
}

/// The engine sends no token over plaintext to a server off this device, so an OAuth calendar at
/// a plaintext address is a server reached only over plaintext, not one that needs a sign-in of
/// its own: a password would be refused the same way.
#[tokio::test]
async fn a_grants_calendar_at_a_plaintext_address_is_an_insecure_server() {
    let account = signed_in_at("http://dav.example.net");
    let (endpoint, _) = mock_token_endpoint(vec![http_fake::minted(1)]);
    let tokens = source_at(endpoint, None);

    let calendar = connect_caldav(&account, Some(&tokens)).await.err();
    let contacts = connect_carddav_contact_providers(&account, Some(&tokens))
        .await
        .err();

    assert!(
        matches!(
            calendar,
            Some(AccountError::InsecureServer {
                cause: InsecureCause::PlaintextAddress,
                ..
            })
        ),
        "{calendar:?}"
    );
    assert!(
        matches!(
            contacts,
            Some(AccountError::InsecureServer {
                cause: InsecureCause::PlaintextAddress,
                ..
            })
        ),
        "{contacts:?}"
    );
}
