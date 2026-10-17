//! Tests for scope selection and the registration request.

use super::*;

fn metadata(scopes: &[&str]) -> AuthServerMetadata {
    AuthServerMetadata {
        issuer: "https://as.example.com".to_owned(),
        authorization_endpoint: "https://as.example.com/authorize".to_owned(),
        token_endpoint: "https://as.example.com/token".to_owned(),
        registration_endpoint: Some("https://as.example.com/register".to_owned()),
        revocation_endpoint: None,
        userinfo_endpoint: None,
        scopes_supported: scopes.iter().map(|s| (*s).to_owned()).collect(),
        end_session_endpoint: None,
        prompt_values_supported: Vec::new(),
        issuer_parameter_supported: false,
        protected_resources: Vec::new(),
    }
}

/// What an account over `protocol` asks a server advertising `scopes` for.
fn select(scopes: &[&str], protocol: MailProtocol) -> Vec<String> {
    select_scopes(&metadata(scopes), protocol)
}

/// Whether an account over `protocol` can use a server advertising `scopes` for mail.
fn usable(scopes: &[&str], protocol: MailProtocol) -> bool {
    let meta = metadata(scopes);
    grants_mail_access(&meta, &select_scopes(&meta, protocol), protocol)
}

#[test]
fn the_software_id_is_a_fixed_uuid() {
    // It identifies the *software*, not an install, so it must never be generated per device
    // or per run: a server keying on it would then see every install as different software.
    assert_eq!(SOFTWARE_ID, "df16a4a8-25e5-428c-907a-a789a3a7b52e");
    assert_eq!(SOFTWARE_ID.len(), 36);
}

#[test]
fn offline_access_is_always_requested() {
    // Without it the server issues no refresh token and the account breaks an hour after
    // setup: the single most expensive thing to get wrong here.
    assert!(select(&[], MailProtocol::Imap).contains(&OFFLINE_ACCESS.to_owned()));
    assert_eq!(
        select(&["urn:ietf:params:oauth:scope:mail"], MailProtocol::Imap)[0],
        OFFLINE_ACCESS
    );
}

#[test]
fn only_the_capabilities_we_use_are_requested() {
    // The shape a real JMAP server advertises. Mail, calendars and contacts are all
    // exercised by the app; admin is not, and an over-broad consent screen is a
    // user-visible harm; we cannot honestly ask for a permission we never use.
    let selected = select(
        &[
            "urn:ietf:params:oauth:scope:mail",
            "urn:ietf:params:oauth:scope:contacts",
            "urn:ietf:params:oauth:scope:calendars",
            "https://example.com/auth/admin",
            "offline_access",
        ],
        MailProtocol::Jmap,
    );
    assert_eq!(
        selected,
        vec![
            "offline_access",
            "urn:ietf:params:oauth:scope:mail",
            "urn:ietf:params:oauth:scope:contacts",
            "urn:ietf:params:oauth:scope:calendars",
        ]
    );
    assert!(
        !selected.iter().any(|scope| scope.ends_with("admin")),
        "a capability the app never exercises must not reach the consent screen"
    );
}

#[test]
fn bare_and_path_style_scope_names_are_recognised_too() {
    // The match is on the last segment, so the URN form, a bare word and a URL form all
    // work: this is the "generic from the standards" rule, not one provider's spelling.
    assert!(select(&["mail"], MailProtocol::Imap).contains(&"mail".to_owned()));
    assert!(
        select(&["https://example.com/auth/calendar"], MailProtocol::Imap)
            .contains(&"https://example.com/auth/calendar".to_owned())
    );
}

#[test]
fn offline_access_is_never_duplicated_when_the_server_advertises_it() {
    let selected = select(&["offline_access", "mail"], MailProtocol::Imap);
    assert_eq!(
        selected.iter().filter(|s| *s == OFFLINE_ACCESS).count(),
        1,
        "a duplicated scope is rejected outright by some servers: {selected:?}"
    );
}

#[test]
fn a_server_offering_no_mail_scope_is_not_usable() {
    // A token good for calendars alone would surface much later as a permanently empty
    // mailbox. Catch it at discovery and fall back to the manual secret instead.
    for protocol in [MailProtocol::Imap, MailProtocol::Jmap] {
        assert!(!usable(
            &["urn:ietf:params:oauth:scope:calendars"],
            protocol
        ));
        assert!(usable(&["urn:ietf:params:oauth:scope:mail"], protocol));
    }
}

#[test]
fn a_server_advertising_no_scopes_at_all_is_allowed_through() {
    // RFC 6749 §3.3: with no scope advertised the server applies its own default grant.
    // That is legal and common on small self-hosted servers, so it must not be mistaken
    // for "no mail access".
    assert!(usable(&[], MailProtocol::Imap));
    assert!(usable(&[], MailProtocol::Jmap));
}

/// What `soverin.net` advertises: the `-00`/`-01` draft names, without the later URN.
const EARLIER_DRAFT_IMAP_SERVER: &[&str] = &["imap", "smtp", "pop", "offline_access"];

#[test]
fn an_imap_account_signs_in_where_the_server_names_the_earlier_drafts_imap_and_smtp() {
    assert!(usable(EARLIER_DRAFT_IMAP_SERVER, MailProtocol::Imap));
    // POP is a protocol the app never speaks, so its permission stays off the consent screen.
    assert_eq!(
        select(EARLIER_DRAFT_IMAP_SERVER, MailProtocol::Imap),
        vec!["offline_access", "imap", "smtp"]
    );
}

#[test]
fn an_imap_account_needs_both_halves_of_the_earlier_drafts_mail() {
    // Reading over IMAP with a token SMTP refuses is an account that fails at its first send.
    assert!(!usable(&["imap", "offline_access"], MailProtocol::Imap));
    assert!(!usable(&["smtp", "offline_access"], MailProtocol::Imap));
}

#[test]
fn the_earlier_drafts_mail_scopes_belong_to_their_own_protocol() {
    // A JMAP account holding `imap` and `smtp` could open neither of its own endpoints, and an
    // IMAP account asking for `jmap` is asking for a permission it never uses.
    assert!(!usable(EARLIER_DRAFT_IMAP_SERVER, MailProtocol::Jmap));
    assert!(usable(&["jmap", "offline_access"], MailProtocol::Jmap));
    assert_eq!(
        select(&["jmap", "imap", "smtp"], MailProtocol::Jmap),
        vec!["offline_access", "jmap"]
    );
    assert_eq!(
        select(&["jmap", "imap", "smtp"], MailProtocol::Imap),
        vec!["offline_access", "imap", "smtp"]
    );
}

#[test]
fn the_standard_name_is_preferred_where_a_server_offers_both() {
    // One permission per kind of data: a consent screen listing mail twice reads as two asks.
    assert_eq!(
        select(
            &[
                "imap",
                "smtp",
                "caldav",
                "carddav",
                "urn:ietf:params:oauth:scope:mail",
                "urn:ietf:params:oauth:scope:calendars",
                "urn:ietf:params:oauth:scope:contacts",
            ],
            MailProtocol::Imap,
        ),
        vec![
            "offline_access",
            "urn:ietf:params:oauth:scope:mail",
            "urn:ietf:params:oauth:scope:calendars",
            "urn:ietf:params:oauth:scope:contacts",
        ]
    );
}

#[test]
fn the_earlier_drafts_calendar_and_contacts_scopes_are_asked_for_too() {
    // The account presents its grant to CalDAV and CardDAV as well, so a server naming them
    // the earlier way still gets asked for them.
    assert_eq!(
        select(&["imap", "smtp", "caldav", "carddav"], MailProtocol::Imap),
        vec!["offline_access", "imap", "smtp", "caldav", "carddav"]
    );
}

#[test]
fn an_earlier_draft_name_is_matched_whole() {
    // A vendor's URL-form scope that happens to end in a protocol name is not the draft's
    // scope and carries no promise about what it grants.
    assert!(!usable(
        &[
            "https://www.example.com/dev/protocol-imap",
            "https://www.example.com/dev/smtp"
        ],
        MailProtocol::Imap
    ));
}

#[test]
fn a_grant_reaches_a_dav_use_only_when_its_request_named_it() {
    let scopes = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let stalwart = scopes(&[
        "offline_access",
        "urn:ietf:params:oauth:scope:mail",
        "urn:ietf:params:oauth:scope:calendars",
    ]);
    assert!(requests_dav_use(&stalwart, DavUse::Calendars));
    assert!(!requests_dav_use(&stalwart, DavUse::Contacts));
    // Soverin's grant: mail only, in the earlier drafts' names.
    let soverin = scopes(&["offline_access", "imap", "smtp"]);
    assert!(!requests_dav_use(&soverin, DavUse::Calendars));
    assert!(!requests_dav_use(&soverin, DavUse::Contacts));
    let earlier = scopes(&["imap", "smtp", "caldav", "carddav"]);
    assert!(requests_dav_use(&earlier, DavUse::Calendars));
    assert!(requests_dav_use(&earlier, DavUse::Contacts));
}

#[test]
fn a_sign_in_that_named_no_scope_leaves_dav_to_the_servers_default() {
    // The issuer advertised no scopes, so the request carried `offline_access` alone and the
    // server applies its default grant; it said nothing that would refuse a calendar.
    let unnamed = vec![OFFLINE_ACCESS.to_owned()];
    assert!(requests_dav_use(&unnamed, DavUse::Calendars));
    assert!(requests_dav_use(&unnamed, DavUse::Contacts));
}
