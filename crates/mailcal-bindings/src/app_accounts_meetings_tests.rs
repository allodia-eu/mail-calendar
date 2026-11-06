//! Switching video meetings on and off as an account's use (`docs/accounts.md` rule 19).

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use mailcal_account::Capability;

use crate::{
    AccountCapability, CapabilityState, MailcalError,
    account_registry::{AccountRegistry, UseChange},
};

fn microsoft(
    granted: &[&str],
    affiliation: Option<engine_api::Affiliation>,
    uses: &str,
) -> Arc<AccountRegistry> {
    let config = mailcal_account::MicrosoftConfig {
        email: "alice@work.example".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: Vec::new(),
        refresh_token: mailcal_account::Secret::new("refresh".to_owned()),
        granted_scopes: Some(granted.iter().map(|&scope| scope.to_owned()).collect()),
        affiliation,
        shape: mailcal_account::AccountShape::read(&format!("capabilities = {uses}")).unwrap(),
    };
    let id = config.account_id().unwrap();
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .unwrap();
    let registry = AccountRegistry::new();
    registry
        .pre_register(
            id.as_str().to_owned(),
            crate::ConnectedAccount::Microsoft { config, tokens },
        )
        .commit();
    registry
}

const MICROSOFT_ID: &str = "alice@work.example@graph.microsoft.com";
const CALENDAR: &str = r#"["calendar"]"#;

fn dav(nextcloud: bool) -> Arc<AccountRegistry> {
    let mut toml = r#"
[caldav]
base_url = "http://127.0.0.1:1/remote.php/dav/"
username = "f3a9c1"
password = "app-password"
"#
    .to_owned();
    if nextcloud {
        toml.push_str("\n[nextcloud]\nserver = \"http://127.0.0.1:1\"\n");
    }
    let config = mailcal_account::load_str(&toml).unwrap();
    assert_eq!(config.account_id().unwrap().as_str(), DAV_ID);
    let registry = AccountRegistry::new();
    registry
        .pre_register(
            DAV_ID.to_owned(),
            crate::ConnectedAccount::imap_account(config, None),
        )
        .commit();
    registry
}

const DAV_ID: &str = "f3a9c1@dav:127.0.0.1:1";

/// The state Settings → Accounts shows for `id`'s video meetings, `None` when it lists none.
fn meetings(registry: &AccountRegistry) -> Option<CapabilityState> {
    crate::accounts_view::entries(&registry.facts(), &BTreeSet::new(), &BTreeMap::new())[0]
        .uses
        .iter()
        .find(|row| row.capability == AccountCapability::Meetings)
        .map(|row| row.state)
}

#[test]
fn teams_meetings_ask_a_work_account_for_their_permission_before_they_switch_on() {
    let registry = microsoft(&["Calendars.ReadWrite"], None, CALENDAR);
    assert_eq!(meetings(&registry), Some(CapabilityState::Off));
    let asked = registry.set_use(MICROSOFT_ID, Capability::Meetings, true);
    assert!(matches!(asked, Ok(UseChange::NeedsConsent)));
    assert_eq!(meetings(&registry), Some(CapabilityState::Off));

    let granted = microsoft(
        &["Calendars.ReadWrite", "OnlineMeetings.ReadWrite"],
        None,
        CALENDAR,
    );
    let on = granted.set_use(MICROSOFT_ID, Capability::Meetings, true);
    assert!(matches!(on, Ok(UseChange::Changed { .. })));
    assert_eq!(meetings(&granted), Some(CapabilityState::On));
}

#[test]
fn a_withheld_permission_leaves_the_use_needing_it() {
    // Chosen, and the grant came back without it: an administrator withheld it.
    let registry = microsoft(
        &["Calendars.ReadWrite"],
        None,
        r#"["calendar", "meetings"]"#,
    );
    assert_eq!(meetings(&registry), Some(CapabilityState::NeedsPermission));
}

#[test]
fn a_personal_microsoft_account_has_no_teams_meetings_to_switch_on() {
    let registry = microsoft(
        &["Calendars.ReadWrite", "OnlineMeetings.ReadWrite"],
        Some(engine_api::Affiliation::Personal),
        CALENDAR,
    );
    assert_eq!(meetings(&registry), None);
    let refused = registry.set_use(MICROSOFT_ID, Capability::Meetings, true);
    assert!(matches!(refused, Err(MailcalError::Config(_))));
}

#[test]
fn a_nextcloud_account_makes_talk_meetings_until_switched_off_and_never_loses_its_last_use() {
    let registry = dav(true);
    // On without being asked: Talk uses the account's own app password.
    assert_eq!(meetings(&registry), Some(CapabilityState::On));
    let off = registry.set_use(DAV_ID, Capability::Meetings, false);
    assert!(matches!(off, Ok(UseChange::Changed { .. })));
    assert_eq!(meetings(&registry), Some(CapabilityState::Off));
    let on = registry.set_use(DAV_ID, Capability::Meetings, true);
    assert!(matches!(on, Ok(UseChange::Changed { .. })));
    assert_eq!(meetings(&registry), Some(CapabilityState::On));
    // Meetings are no use to keep an account for: calendar and contacts are.
    let calendar_off = registry.set_use(DAV_ID, Capability::Calendar, false);
    assert!(matches!(calendar_off, Ok(UseChange::Changed { .. })));
    let contacts_off = registry.set_use(DAV_ID, Capability::Contacts, false);
    assert!(matches!(contacts_off, Err(MailcalError::Config(_))));
    let off = registry.set_use(DAV_ID, Capability::Meetings, false);
    assert!(matches!(off, Ok(UseChange::Changed { .. })));
}

#[test]
fn a_nextcloud_server_without_talk_reads_as_not_offered_until_a_connect_finds_it() {
    let registry = dav(true);
    let talk = registry.talk_offer(DAV_ID).unwrap();
    talk.record(false);
    assert_eq!(meetings(&registry), Some(CapabilityState::NotOffered));
    assert!(
        registry
            .set_use(DAV_ID, Capability::Meetings, true)
            .is_err()
    );
    talk.record(true);
    assert_eq!(meetings(&registry), Some(CapabilityState::On));
}

#[test]
fn an_account_no_nextcloud_sign_in_made_offers_no_meetings() {
    let registry = dav(false);
    assert_eq!(meetings(&registry), None);
    let refused = registry.set_use(DAV_ID, Capability::Meetings, true);
    assert!(matches!(refused, Err(MailcalError::Config(_))));
}

/// What Settings → Videoconferencing lists for `registry`'s accounts.
fn overview(registry: &AccountRegistry) -> Vec<(String, String, CapabilityState)> {
    let entries =
        crate::accounts_view::entries(&registry.facts(), &BTreeSet::new(), &BTreeMap::new());
    crate::accounts_view::meeting_accounts(&entries)
        .into_iter()
        .map(|row| (row.service, row.account.address, row.state))
        .collect()
}

#[test]
fn videoconferencing_lists_each_account_that_makes_meetings_in_its_state() {
    let work = microsoft(&["Calendars.ReadWrite"], None, CALENDAR);
    assert_eq!(
        overview(&work),
        [(
            "Microsoft Teams".to_owned(),
            "alice@work.example".to_owned(),
            CapabilityState::Off
        )]
    );
    let cloud = dav(true);
    let row = overview(&cloud).remove(0);
    assert_eq!(
        (row.0.as_str(), row.2),
        ("Nextcloud Talk", CapabilityState::On)
    );
    cloud.talk_offer(DAV_ID).unwrap().record(false);
    assert_eq!(overview(&cloud)[0].2, CapabilityState::NotOffered);
    assert!(overview(&dav(false)).is_empty());
    let personal = microsoft(&[], Some(engine_api::Affiliation::Personal), CALENDAR);
    assert!(overview(&personal).is_empty());
}
