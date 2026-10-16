//! Where an OAuth account's grant may go when it reaches for its calendar and contacts
//! (`docs/mail-oauth.md` rule 10). Every refusal is decided before a token is minted.

use std::sync::atomic::Ordering;

use mailcal_oauth::DavUse;
use provider_caldav::Credentials;

use crate::{
    AccountError,
    config_dav::{DavAuth, dav_credentials},
    graph::test_support::{ratcheting_token_endpoint, source_at},
    load_str,
};

/// An OAuth IMAP account: `scopes` is what its sign-in asked for, `dav` its calendar server.
fn signed_in(scopes: &str, dav: &str, extra: &str) -> crate::AccountConfig {
    load_str(&format!(
        r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"

[caldav]
base_url = "{dav}"
username = "you@example.net"

[oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = [{scopes}]
authorization_server = "https://auth.example.net"
{extra}
"#
    ))
    .unwrap()
}

const FULL: &str = r#""offline_access", "urn:ietf:params:oauth:scope:mail", "urn:ietf:params:oauth:scope:calendars", "urn:ietf:params:oauth:scope:contacts""#;

/// The result for `account`'s calendar, and how many tokens were minted for it.
async fn calendar_auth(
    account: &crate::AccountConfig,
    dav_use: DavUse,
) -> (Result<DavAuth, AccountError>, usize) {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let auth = dav_credentials(
        account,
        account.caldav_endpoint().unwrap(),
        dav_use,
        Some(&tokens),
    )
    .await;
    (auth, hits.load(Ordering::SeqCst))
}

#[tokio::test]
async fn a_grant_reaches_a_calendar_its_issuer_covers_as_bearer_under_the_issuers_host_rule() {
    let account = signed_in(FULL, "https://dav.example.net/dav", "");
    let (auth, _) = calendar_auth(&account, DavUse::Calendars).await;
    let auth = auth.expect("the issuer's own domain, with the calendar scope asked for");
    assert!(matches!(auth.credentials, Credentials::Bearer(token) if token == "AT-1"));
    let hosts = auth
        .hosts
        .expect("a token goes with the issuer's host rule");
    assert!(hosts.permits("dav.example.net"));
    assert!(
        hosts.permits("d1.caldav.example.net"),
        "a redirect within the issuer's domain"
    );
    assert!(
        !hosts.permits("caldav.example.org"),
        "a redirect off the issuer's domain"
    );
}

#[tokio::test]
async fn a_grant_that_asked_for_no_calendar_scope_never_reaches_a_calendar() {
    // Soverin's shape: a mail-only grant in the earlier drafts' names, and a calendar server on
    // the issuer's own domain that would not take it anyway.
    let account = signed_in(
        r#""offline_access", "imap", "smtp""#,
        "https://caldav.example.net",
        "",
    );
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars).await;
    assert!(matches!(
        auth,
        Err(AccountError::DavNeedsOwnSignIn("calendar"))
    ));
    assert_eq!(
        minted, 0,
        "no token is minted for a server it may not reach"
    );
}

#[tokio::test]
async fn a_grant_never_reaches_a_server_outside_its_issuers_domain() {
    // The email domain's well-known may point at a different provider's server, which accepts
    // Bearer for its own sign-in; this account's token must not go there.
    let account = signed_in(FULL, "https://cloud.example.org/remote.php/dav", "");
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars).await;
    assert!(matches!(
        auth,
        Err(AccountError::DavNeedsOwnSignIn("calendar"))
    ));
    assert_eq!(minted, 0);
}

#[tokio::test]
async fn a_scope_for_one_use_does_not_open_the_other() {
    let account = signed_in(
        r#""offline_access", "urn:ietf:params:oauth:scope:mail", "urn:ietf:params:oauth:scope:calendars""#,
        "https://dav.example.net",
        "",
    );
    let (auth, _) = calendar_auth(&account, DavUse::Contacts).await;
    assert!(matches!(
        auth,
        Err(AccountError::DavNeedsOwnSignIn("contacts"))
    ));
}

#[tokio::test]
async fn an_issuers_list_replaces_its_domain() {
    let account = signed_in(
        FULL,
        "https://dav.example.net",
        r#"protected_resources = ["https://calendar.example.com", "imap.example.net"]"#,
    );
    let (auth, _) = calendar_auth(&account, DavUse::Calendars).await;
    assert!(
        matches!(auth, Err(AccountError::DavNeedsOwnSignIn("calendar"))),
        "on the issuer's domain but not in its list"
    );

    let listed = signed_in(
        FULL,
        "https://calendar.example.com/dav",
        r#"protected_resources = ["https://calendar.example.com", "imap.example.net"]"#,
    );
    let (auth, _) = calendar_auth(&listed, DavUse::Calendars).await;
    assert!(
        auth.expect("listed")
            .hosts
            .unwrap()
            .permits("calendar.example.com")
    );
}

#[tokio::test]
async fn a_password_account_presents_its_password_with_no_host_rule() {
    let account = load_str(
        r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"
password = "pw"

[caldav]
base_url = "https://dav.example.net"
username = "you@example.net"
password = "pw"
"#,
    )
    .unwrap();
    let auth = dav_credentials(
        &account,
        account.caldav_endpoint().unwrap(),
        DavUse::Calendars,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(auth.credentials, Credentials::Basic { password, .. } if password == "pw"));
    assert!(auth.hosts.is_none());
}
