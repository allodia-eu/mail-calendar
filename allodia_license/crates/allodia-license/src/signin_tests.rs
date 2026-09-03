// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! What can be wrong here while a sign-in still looks like it is working: a registration request
//! that quietly lands on the sign-in page, a `prompt` sent to a server that never offered one, and
//! a handle stored by an older build that no longer parses, which would strand someone mid-flow,
//! after the browser has already issued a code that cannot be obtained twice.
//!
//! The flow itself is not unit-tested: building a [`SignIn`] needs a client registration this
//! build may not carry, and the round trip is verified against the production service instead
//! (`entitlement.md`). What is testable without one is every decision made *about* the request,
//! and those are the ones that differ per server.

use super::*;

fn advertised(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn creating_an_account_asks_for_the_registration_page() {
    assert!(sends_create(
        Prompt::Create,
        &advertised(&["login", "create", "none"])
    ));
}

#[test]
fn signing_in_never_asks_for_it_however_much_the_server_offers() {
    assert!(!sends_create(
        Prompt::SignIn,
        &advertised(&["login", "create", "none"])
    ));
}

#[test]
fn a_server_that_does_not_advertise_create_is_not_sent_one() {
    // The fallback is deliberate and costs a click, not the flow: the ordinary sign-in page is
    // where someone registers anyway. Sending it regardless risks the request being refused.
    assert!(!sends_create(
        Prompt::Create,
        &advertised(&["login", "none"])
    ));
    assert!(!sends_create(Prompt::Create, &[]));
}

#[test]
fn the_prompt_is_appended_to_parameters_the_request_already_has() {
    let url = append_prompt_create("https://as.example.com/authorize?client_id=x&state=y");
    assert_eq!(
        url,
        "https://as.example.com/authorize?client_id=x&state=y&prompt=create"
    );
}

#[test]
fn a_handle_written_before_these_fields_existed_still_parses() {
    // The handle crosses a browser round trip, so an app updated mid-flow reads back one the
    // previous build wrote. Failing here would strand the person after the code was issued.
    let older = r#"{
        "authorize_endpoint": "https://as.example.com/authorize",
        "token_endpoint": "https://as.example.com/token",
        "resource": null,
        "userinfo_endpoint": "https://as.example.com/userinfo"
    }"#;
    let endpoints: Endpoints = serde_json::from_str(older).unwrap();
    assert_eq!(endpoints.token_endpoint, "https://as.example.com/token");
    assert!(endpoints.end_session_endpoint.is_none());
    assert!(endpoints.prompt_values_supported.is_empty());
    assert!(endpoints.scopes.is_empty());
}

/// A build that reaches a deployment predating a scope loses the feature, not the sign-in.
///
/// This is the whole point of the intersection. Asking for a scope a server has not advertised is
/// refused outright by enough of them that the alternative is a client which cannot sign in at all
/// until the server catches up: a far worse failure than no account sync, and one that
/// would land on everybody rather than on the feature.
#[test]
fn a_scope_the_service_does_not_offer_is_not_asked_for() {
    let today = advertised(&["openid", "profile", "email", "offline_access"]);
    let asked = scopes_for(&today);
    assert!(!asked.iter().any(|scope| scope.starts_with("mailcal:")));
    assert!(asked.iter().any(|scope| scope == "offline_access"));
}

#[test]
fn a_scope_the_service_offers_is_asked_for() {
    let tomorrow = advertised(&[
        "openid",
        "profile",
        "email",
        "offline_access",
        "mailcal:entitlement:read",
        "mailcal:accounts:read",
        "mailcal:accounts:write",
    ]);
    let asked = scopes_for(&tomorrow);
    for scope in SCOPES {
        assert!(
            asked.iter().any(|got| got == scope),
            "{scope} not asked for"
        );
    }
}

/// The four that gate the sign-in itself are sent whether or not the service lists them.
///
/// A service that simply under-advertises would otherwise produce a sign-in that succeeds and then
/// cannot say who signed in, or one that expires within the hour with no way back, both of which
/// read as the app being broken rather than as the server being terse.
#[test]
fn the_load_bearing_scopes_are_sent_even_to_a_service_that_lists_none() {
    let asked = scopes_for(&[]);
    for scope in ["openid", "profile", "email", "offline_access"] {
        assert!(asked.iter().any(|got| got == scope), "{scope} was dropped");
    }
    assert!(!asked.iter().any(|scope| scope.starts_with("mailcal:")));
}

#[test]
fn a_handle_round_trips_what_the_server_said_about_logout_and_prompts() {
    let endpoints = Endpoints {
        authorize_endpoint: "https://as.example.com/authorize".to_owned(),
        token_endpoint: "https://as.example.com/token".to_owned(),
        resource: Some("https://api.example.com/".to_owned()),
        userinfo_endpoint: None,
        end_session_endpoint: Some("https://as.example.com/end-session".to_owned()),
        prompt_values_supported: advertised(&["create"]),
        scopes: advertised(&["openid", "offline_access"]),
        issuer: Some("https://as.example.com".to_owned()),
    };
    let text = serde_json::to_string(&endpoints).unwrap();
    let read: Endpoints = serde_json::from_str(&text).unwrap();
    assert_eq!(read, endpoints);
}

#[test]
fn the_account_page_follows_the_host_this_build_points_at() {
    // Derived rather than written out, so a development build reaches its own service's page.
    let url = account_url();
    assert!(
        url.starts_with(&host()),
        "{url} should sit under {}",
        host()
    );
    assert!(
        url.ends_with("/account"),
        "{url} should be the account page"
    );
}
