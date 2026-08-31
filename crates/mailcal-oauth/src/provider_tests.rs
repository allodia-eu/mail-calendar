//! Authorization-URL shape and the per-provider scope sets.

use super::*;

fn microsoft() -> OAuthProviderConfig {
    OAuthProviderConfig::microsoft(
        "client-abc",
        "common",
        "eu.allodia.mailcal://oauth",
        MICROSOFT_GRAPH_SCOPES,
    )
}

#[test]
fn microsoft_endpoints_are_the_v2_authority_paths() {
    let cfg = microsoft();
    assert_eq!(
        cfg.authorize_endpoint,
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
    );
    assert_eq!(
        cfg.token_endpoint,
        "https://login.microsoftonline.com/common/oauth2/v2.0/token"
    );
}

#[test]
fn a_blank_tenant_falls_back_to_common() {
    let cfg = OAuthProviderConfig::microsoft("c", "  ", "r://x", &["openid"]);
    assert!(cfg.authorize_endpoint.contains("/common/"));
}

#[test]
fn a_specific_tenant_is_used_verbatim() {
    let cfg = OAuthProviderConfig::microsoft("c", "contoso.onmicrosoft.com", "r://x", &["openid"]);
    assert!(
        cfg.token_endpoint
            .contains("/contoso.onmicrosoft.com/oauth2/v2.0/token")
    );
}

#[test]
fn authorization_url_carries_pkce_state_and_percent_encoded_params() {
    let url = microsoft().authorization_url("state-xyz", "chal-123", None);
    let parsed = url::Url::parse(&url).unwrap();
    let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

    assert_eq!(q["client_id"], "client-abc");
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["redirect_uri"], "eu.allodia.mailcal://oauth");
    assert_eq!(q["state"], "state-xyz");
    assert_eq!(q["code_challenge"], "chal-123");
    assert_eq!(q["code_challenge_method"], "S256");
    // No hint ⇒ let the user pick their account.
    assert_eq!(q["prompt"], "select_account");
    assert!(!q.contains_key("login_hint"));
    // Scopes are space-joined; the space must be percent-encoded in the raw query.
    assert_eq!(q["scope"], MICROSOFT_GRAPH_SCOPES.join(" "));
    assert!(parsed.query().unwrap().contains("scope=offline_access"));
    assert!(!parsed.query().unwrap().contains("scope=offline_access "));
}

#[test]
fn a_discovered_resource_is_named_on_the_authorization_request() {
    // RFC 8707. A server may bind the issued code to the target, so the resource has to be on
    // the front-channel request as well as the exchange.
    let mut cfg = microsoft();
    cfg.resource = Some("https://api.example.com/jmap/session".to_owned());
    let url = cfg.authorization_url("s", "c", None);
    let parsed = url::Url::parse(&url).unwrap();
    let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(q["resource"], "https://api.example.com/jmap/session");
    // …and a provider that discovered none sends none.
    assert!(
        !microsoft()
            .authorization_url("s", "c", None)
            .contains("resource=")
    );
}

#[test]
fn a_login_hint_targets_that_account_instead_of_the_picker() {
    let url = microsoft().authorization_url("s", "c", Some("alice@example.com"));
    let parsed = url::Url::parse(&url).unwrap();
    let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    // The hint pre-fills/targets the account, and the "pick an account" prompt is gone.
    assert_eq!(q["login_hint"], "alice@example.com");
    assert!(!q.contains_key("prompt"));
    // A blank hint falls back to the picker, not an empty login_hint.
    let blank = microsoft().authorization_url("s", "c", Some("   "));
    assert!(!blank.contains("login_hint"));
    assert!(blank.contains("prompt=select_account"));
}

#[test]
fn requests_offline_access_so_a_refresh_token_is_issued() {
    // Without `offline_access` Microsoft issues no refresh token: the account
    // would break an hour after setup. Guard the default scope set.
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"offline_access"));
}

#[test]
fn requests_the_calendar_scope_so_graph_calendar_sync_works() {
    // Without `Calendars.ReadWrite` the Graph calendar read/sync + writes 403; a
    // Microsoft account would connect mail-only. Guard the scope is requested.
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/Calendars.ReadWrite"));
}

fn google() -> OAuthProviderConfig {
    OAuthProviderConfig::google(
        "google-client",
        None,
        "com.googleusercontent.apps.google-client:/oauth2redirect",
        GOOGLE_SCOPES,
    )
}

#[test]
fn google_uses_the_fixed_google_endpoints() {
    let cfg = google();
    assert_eq!(
        cfg.authorize_endpoint,
        "https://accounts.google.com/o/oauth2/v2/auth"
    );
    assert_eq!(cfg.token_endpoint, "https://oauth2.googleapis.com/token");
}

#[test]
fn google_authorization_url_forces_offline_access_and_consent() {
    // Without `access_type=offline` + `prompt=consent` Google issues no refresh token on a
    // repeat authorisation and the account breaks an hour later; guard both are present.
    let url = google().authorization_url("state-xyz", "chal-123", None);
    let parsed = url::Url::parse(&url).unwrap();
    let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

    assert_eq!(q["client_id"], "google-client");
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["access_type"], "offline");
    assert_eq!(q["prompt"], "consent");
    // Google is not sent Microsoft's `response_mode`, and with no hint there is no picker.
    assert!(!q.contains_key("response_mode"));
    assert!(!q.contains_key("login_hint"));
    assert_eq!(q["scope"], GOOGLE_SCOPES.join(" "));
}

#[test]
fn google_login_hint_targets_the_account_but_keeps_the_consent_prompt() {
    // Unlike Microsoft, a hint must NOT drop `prompt`: Google still needs `prompt=consent`
    // to reissue a refresh token, so both travel together.
    let url = google().authorization_url("s", "c", Some("alice@gmail.com"));
    let parsed = url::Url::parse(&url).unwrap();
    let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(q["login_hint"], "alice@gmail.com");
    assert_eq!(q["prompt"], "consent");
    // A blank hint is dropped, not sent empty.
    let blank = google().authorization_url("s", "c", Some("  "));
    assert!(!blank.contains("login_hint"));
}

#[test]
fn a_google_desktop_client_carries_its_non_confidential_secret() {
    // The iOS/Android google() helper is secretless; a Desktop client passes Some(secret),
    // which the token exchange must then send (Google rejects the PKCE exchange without it).
    assert!(google().client_secret.is_none());
    let desktop = OAuthProviderConfig::google(
        "desktop-client",
        Some("GOCSPX-not-a-real-secret".to_owned()),
        "http://127.0.0.1:0/",
        GOOGLE_SCOPES,
    );
    assert_eq!(
        desktop.client_secret.as_deref(),
        Some("GOCSPX-not-a-real-secret")
    );
}

/// The same three surfaces Microsoft's set covers: the user's own contacts, the addresses
/// Google collects on their behalf, and (on a Workspace domain) their colleagues, who are
/// where a profile photo actually comes from.
#[test]
fn google_requests_the_contact_scopes_so_saved_and_directory_people_resolve() {
    assert!(GOOGLE_SCOPES.contains(&"https://www.googleapis.com/auth/contacts"));
    assert!(GOOGLE_SCOPES.contains(&"https://www.googleapis.com/auth/contacts.other.readonly"));
    assert!(GOOGLE_SCOPES.contains(&"https://www.googleapis.com/auth/directory.readonly"));
}

#[test]
fn google_requests_full_gmail_and_calendar_scopes() {
    // Permanent `messages.delete` needs the full mail scope; calendar writes need the
    // calendar scope. Guard both so a Google account is never silently mail- or
    // calendar-only.
    assert!(GOOGLE_SCOPES.contains(&"https://mail.google.com/"));
    assert!(GOOGLE_SCOPES.contains(&"https://www.googleapis.com/auth/calendar"));
}

/// The photo a mail row draws comes from the directory rather than from a saved card for
/// most correspondents, so both scopes are load-bearing.
///
/// Contacts are asked for read **and write** deliberately, ahead of the editing feature:
/// widening later would force every Microsoft account through a second re-authentication,
/// which is worse than one broader prompt now. The read-only promise lives in the privacy
/// policy and in what the code does, not in the narrowness of the scope.
#[test]
fn requests_the_contact_scopes_so_saved_and_directory_people_resolve() {
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/Contacts.ReadWrite"));
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/User.ReadBasic.All"));
}

/// A scope the account cannot grant fails at **consent**, so it does not degrade one
/// capability; it stops the account being added. These two are tenant-wide reads a tenant
/// may put behind an administrator, and neither buys anything the set above does not already
/// cover, so requesting them would risk locking users out of setup for nothing.
#[test]
fn does_not_request_the_tenant_wide_permissions_an_admin_may_have_to_approve() {
    assert!(
        !MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/ProfilePhoto.Read.All"),
        "User.ReadBasic.All already grants the photo read"
    );
    assert!(
        !MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/OrgContact.Read.All"),
        "organizational contacts are a source the product does not read"
    );
}

#[test]
fn requests_the_mail_write_and_send_scopes_so_graph_actions_and_sending_work() {
    // `Mail.Read` is read-only: the mail write actions (mark-read/flag, move/archive,
    // delete) need `Mail.ReadWrite`, and submission needs the distinct `Mail.Send`
    // (`Mail.ReadWrite` does not grant send). Without them the advertised
    // mail_writes/submission capabilities 403 at runtime. Guard both are requested, and
    // that the redundant read-only scope (subsumed by `Mail.ReadWrite`) is not.
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/Mail.ReadWrite"));
    assert!(MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/Mail.Send"));
    assert!(!MICROSOFT_GRAPH_SCOPES.contains(&"https://graph.microsoft.com/Mail.Read"));
}
