use engine_core::ids::AccountId;

use crate::{AccountConfig, Capability, load_str};

/// A mailbox that signs in with a password, with its calendar and contacts on another site that
/// takes the same password, as a provider whose DAV server knows nothing of its sign-in has it.
const PASSWORD: &str = r#"
[imap]
addr = "imap.mail.example:993"
server_name = "imap.mail.example"
username = "alice@mail.example"
password = "pw"

[caldav]
base_url = "https://dav.mail.example"
username = "alice@mail.example"
password = "pw"
"#;

/// The grant a sign-in at `mail.example` issues: mail scopes only, as Soverin's does.
const GRANT: &str = r#"
[oauth]
client_id = "client-abc"
refresh_token = "rt"
authorize_endpoint = "https://auth.mail.example/authorize"
token_endpoint = "https://auth.mail.example/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "imap", "smtp"]
issuer = "https://auth.mail.example"
"#;

fn password_account() -> AccountConfig {
    load_str(PASSWORD).unwrap()
}

fn signed_in(account: &AccountConfig) -> AccountConfig {
    let grant: AccountConfig = load_str(&format!("{PASSWORD}{GRANT}")).unwrap();
    account.with_grant(grant.oauth.unwrap())
}

fn uses(account: &AccountConfig) -> Vec<Capability> {
    account.capabilities().iter().collect()
}

#[test]
fn a_password_reaches_whatever_takes_it() {
    assert_eq!(password_account().uses_its_grant_does_not_reach(), []);
}

#[test]
fn a_grant_without_calendar_scopes_reaches_neither_calendar_nor_contacts() {
    assert_eq!(
        signed_in(&password_account()).uses_its_grant_does_not_reach(),
        [Capability::Calendar, Capability::Contacts]
    );
}

/// The calendar account keeps the password the servers took, the logins they had, and names the
/// mailbox it came from as the one it sends through.
#[test]
fn the_calendar_account_keeps_the_servers_and_the_password_that_worked() {
    let before = password_account();
    let calendar = before
        .calendar_account_for(&[Capability::Calendar, Capability::Contacts])
        .unwrap();

    assert!(calendar.imap.is_none() && calendar.smtp.is_none() && calendar.oauth.is_none());
    let caldav = calendar.caldav.as_ref().unwrap();
    assert_eq!(caldav.base_url, "https://dav.mail.example");
    assert_eq!(caldav.password.as_ref().unwrap().expose(), "pw");
    assert!(
        calendar.carddav.is_none(),
        "contacts stay beside the calendar"
    );
    assert_eq!(
        uses(&calendar),
        [Capability::Calendar, Capability::Contacts]
    );
    assert_eq!(
        calendar.shape.links.mail.as_ref().map(AccountId::as_str),
        Some("alice@mail.example@imap.mail.example")
    );
    assert_eq!(
        calendar.account_id().unwrap().as_str(),
        "alice@mail.example@dav:dav.mail.example"
    );
}

#[test]
fn contacts_moved_alone_take_the_calendars_server_as_their_own() {
    let contacts = password_account()
        .calendar_account_for(&[Capability::Contacts])
        .unwrap();
    assert!(contacts.caldav.is_none());
    assert_eq!(
        contacts.carddav.as_ref().unwrap().base_url,
        "https://dav.mail.example"
    );
    assert_eq!(uses(&contacts), [Capability::Contacts]);
}

#[test]
fn nothing_to_move_is_no_account() {
    assert!(
        password_account()
            .calendar_account_for(&[Capability::Mail])
            .is_none()
    );
}

/// The mailbox is left signing in alone: no calendar server, no password, linked to the account
/// its calendar and contacts moved to.
#[test]
fn the_mailbox_keeps_mail_and_links_to_where_the_rest_went() {
    let moved_to = AccountId::try_from("alice@mail.example@dav:dav.mail.example").unwrap();
    let mailbox = signed_in(&password_account()).without_uses(
        &[Capability::Calendar, Capability::Contacts],
        Some(&moved_to),
    );

    assert!(mailbox.caldav.is_none() && mailbox.carddav.is_none());
    assert_eq!(uses(&mailbox), [Capability::Mail]);
    assert_eq!(mailbox.shape.links.calendar.as_ref(), Some(&moved_to));
    assert_eq!(mailbox.shape.links.contacts.as_ref(), Some(&moved_to));
    assert_eq!(mailbox.uses_its_grant_does_not_reach(), []);
}

#[test]
fn a_calendar_moved_alone_leaves_its_server_to_the_contacts_that_stay() {
    let moved_to = AccountId::try_from("alice@mail.example@dav:dav.mail.example").unwrap();
    let mailbox = password_account().without_uses(&[Capability::Calendar], Some(&moved_to));
    assert!(mailbox.caldav.is_none());
    assert_eq!(
        mailbox.carddav.as_ref().unwrap().base_url,
        "https://dav.mail.example"
    );
    assert_eq!(uses(&mailbox), [Capability::Mail, Capability::Contacts]);
    assert!(mailbox.shape.links.contacts.is_none());
}

#[test]
fn a_typed_login_replaces_the_stored_one_on_every_server() {
    let calendar = password_account()
        .calendar_account_for(&[Capability::Calendar, Capability::Contacts])
        .unwrap()
        .with_dav_login("alice", "dav-pw");
    let caldav = calendar.caldav.unwrap();
    assert_eq!(caldav.username, "alice");
    assert_eq!(caldav.password.unwrap().expose(), "dav-pw");
}

#[test]
fn uses_stopped_rather_than_moved_link_nowhere() {
    let mailbox = signed_in(&password_account())
        .without_uses(&[Capability::Calendar, Capability::Contacts], None);
    assert_eq!(uses(&mailbox), [Capability::Mail]);
    assert!(mailbox.shape.links.is_empty());
}

/// The grant a sign-in at the calendar server's own provider issues: calendar and contacts
/// scopes from an issuer on the calendar server's site.
const DAV_GRANT: &str = r#"
[oauth]
client_id = "client-dav"
refresh_token = "rt"
authorize_endpoint = "https://dav.mail.example/oauth/authorize"
token_endpoint = "https://dav.mail.example/oauth/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "urn:ietf:params:oauth:scope:calendars", "urn:ietf:params:oauth:scope:contacts"]
authorization_server = "https://dav.mail.example"
"#;

#[test]
fn a_calendar_account_signed_in_at_its_own_provider_keeps_no_password() {
    let grant: AccountConfig = load_str(&format!("{PASSWORD}{DAV_GRANT}")).unwrap();
    let calendar = password_account()
        .calendar_account_for(&[Capability::Calendar, Capability::Contacts])
        .unwrap()
        .with_dav_grant(grant.oauth.unwrap());

    assert!(calendar.is_oauth());
    assert!(calendar.caldav.as_ref().unwrap().password.is_none());
    assert_eq!(calendar.uses_its_grant_does_not_reach(), []);
    assert_eq!(
        uses(&calendar),
        [Capability::Calendar, Capability::Contacts]
    );
}
