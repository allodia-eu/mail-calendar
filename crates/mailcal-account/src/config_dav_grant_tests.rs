//! Where an OAuth account's grant may go when it reaches for its calendar and contacts
//! (`docs/mail-oauth.md` rule 10). Every refusal is decided before a token is minted.

use std::sync::{Arc, atomic::Ordering};

use engine_provider::{
    Credential, CredentialProtocol, CredentialRequest, CredentialSource, CredentialSubject, Origin,
};
use mailcal_oauth::DavUse;

use crate::{
    AccountError,
    config_dav::dav_credential_source,
    graph::test_support::{ratcheting_token_endpoint, source_at},
    load_str,
};

/// An HTTPS origin on the default port.
fn https(host: &str) -> Origin {
    Origin::new("https", host, 443)
}

/// What `source` presents to `origin`.
async fn presented(source: &dyn CredentialSource, origin: &Origin) -> Credential {
    source
        .credential(&CredentialRequest::new(CredentialProtocol::Http, origin))
        .await
        .unwrap()
}

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

/// The source for `account`'s calendar, and how many tokens were minted while it was built.
fn calendar_auth(
    account: &crate::AccountConfig,
    dav_use: DavUse,
) -> (Result<Arc<dyn CredentialSource>, AccountError>, usize) {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let auth = dav_credential_source(
        account,
        account.caldav_endpoint().unwrap(),
        dav_use,
        Some(&tokens),
    );
    (auth, hits.load(Ordering::SeqCst))
}

#[tokio::test]
async fn a_grant_reaches_a_calendar_its_issuer_covers_as_bearer_under_the_issuers_host_rule() {
    let account = signed_in(FULL, "https://dav.example.net/dav", "");
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars);
    let auth = auth.expect("the issuer's own domain, with the calendar scope asked for");
    assert_eq!(minted, 0, "the token is asked per request, not at connect");
    let origin = https("dav.example.net");
    assert!(matches!(
        presented(auth.as_ref(), &origin).await,
        Credential::Bearer { token, .. } if token.expose() == "AT-1"
    ));
    assert!(auth.permits(&origin));
    assert!(
        auth.permits(&https("d1.caldav.example.net")),
        "a redirect within the issuer's domain"
    );
    assert!(
        !auth.permits(&https("caldav.example.org")),
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
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars);
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
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars);
    assert!(matches!(
        auth,
        Err(AccountError::DavNeedsOwnSignIn("calendar"))
    ));
    assert_eq!(minted, 0);
}

#[tokio::test]
async fn a_calendar_at_an_ipv6_literal_is_judged_by_its_whole_address() {
    let account = signed_in(
        FULL,
        "https://[::1]/dav",
        r#"protected_resources = ["https://[::1]/dav"]"#,
    );
    let (auth, _) = calendar_auth(&account, DavUse::Calendars);
    assert!(auth.is_ok(), "a listed IPv6 host, with no port after it");
}

#[tokio::test]
async fn a_scope_for_one_use_does_not_open_the_other() {
    let account = signed_in(
        r#""offline_access", "urn:ietf:params:oauth:scope:mail", "urn:ietf:params:oauth:scope:calendars""#,
        "https://dav.example.net",
        "",
    );
    let (auth, _) = calendar_auth(&account, DavUse::Contacts);
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
    let (auth, _) = calendar_auth(&account, DavUse::Calendars);
    assert!(
        matches!(auth, Err(AccountError::DavNeedsOwnSignIn("calendar"))),
        "on the issuer's domain but not in its list"
    );

    let listed = signed_in(
        FULL,
        "https://calendar.example.com/dav",
        r#"protected_resources = ["https://calendar.example.com", "imap.example.net"]"#,
    );
    let (auth, _) = calendar_auth(&listed, DavUse::Calendars);
    assert!(
        auth.expect("listed")
            .permits(&https("calendar.example.com"))
    );
}

/// The calendar signs in with the mailbox's own login, so it is the same subject as the mailbox.
#[tokio::test]
async fn a_calendar_on_the_mailboxs_login_presents_it_under_the_account_id_with_no_host_rule() {
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
    let auth = dav_credential_source(
        &account,
        account.caldav_endpoint().unwrap(),
        DavUse::Calendars,
        None,
    )
    .unwrap();
    let origin = https("dav.example.net");
    assert!(matches!(
        presented(auth.as_ref(), &origin).await,
        Credential::Password { password, .. } if password.expose() == "pw"
    ));
    assert_eq!(
        auth.subject(),
        &CredentialSubject::new(account.account_id().unwrap().as_str())
    );
    assert!(auth.permits(&https("elsewhere.example.org")));
}

/// Each login is a subject of its own: a calendar and an address book that sign in with their own
/// stored logins share no subject with the mailbox or with each other.
#[test]
fn a_dav_login_of_its_own_is_a_subject_of_its_own() {
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
password = "calendar-app-password"

[carddav]
base_url = "https://cards.example.net"
username = "cards-login"
password = "pw"
"#,
    )
    .unwrap();
    let source = |endpoint, dav_use| {
        dav_credential_source(&account, endpoint, dav_use, None)
            .unwrap()
            .subject()
            .clone()
    };
    let mailbox = CredentialSubject::new(account.account_id().unwrap().as_str());
    let calendar = source(account.caldav_endpoint().unwrap(), DavUse::Calendars);
    let contacts = source(account.carddav_endpoint().unwrap(), DavUse::Contacts);

    assert_ne!(calendar, mailbox, "another password is another login");
    assert_ne!(contacts, mailbox, "another username is another login");
    assert_ne!(calendar, contacts);
}

/// Whether a token may travel over plaintext is the engine's rule, which refuses it on the
/// connect: a calendar at a plaintext address on the issuer's hosts is one the grant reaches, so
/// it is not told it needs a sign-in of its own.
#[tokio::test]
async fn a_plaintext_address_on_the_issuers_hosts_is_one_the_grant_reaches() {
    let account = signed_in(FULL, "http://dav.example.net/dav", "");
    let (auth, minted) = calendar_auth(&account, DavUse::Calendars);
    assert!(auth.is_ok(), "{:?}", auth.err());
    assert_eq!(minted, 0);
}

/// Stalwart's list names its HTTPS origins and its host name bare, for IMAP and SMTP. A calendar
/// on that host is one the list admits only when an HTTPS entry matches its URL, port included,
/// and a redirect only to a host an HTTPS entry names.
#[tokio::test]
async fn a_listed_host_admits_only_the_servers_the_list_matches() {
    let listed = r#"protected_resources = ["https://dav.example.net", "imap.example.net"]"#;

    let account = signed_in(FULL, "https://dav.example.net/dav", listed);
    let (auth, _) = calendar_auth(&account, DavUse::Calendars);
    let auth = auth.expect("an HTTPS entry covers the calendar");
    assert!(auth.permits(&https("dav.example.net")));
    assert!(
        !auth.permits(&https("imap.example.net")),
        "a bare entry names the IMAP host, not a DAV hop to it"
    );
    assert!(
        !auth.permits(&Origin::new("https", "dav.example.net", 8443)),
        "a hop to another port of the listed host is another server"
    );

    for elsewhere in [
        "https://dav.example.net:8443/dav",
        "https://imap.example.net/dav",
    ] {
        let account = signed_in(FULL, elsewhere, listed);
        let (auth, minted) = calendar_auth(&account, DavUse::Calendars);
        assert!(
            matches!(auth, Err(AccountError::DavNeedsOwnSignIn("calendar"))),
            "{elsewhere} is another server than any the list names"
        );
        assert_eq!(minted, 0, "refused before a token is minted");
    }
}
