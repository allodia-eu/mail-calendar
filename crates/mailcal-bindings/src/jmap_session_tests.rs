//! What a JMAP session leaves out: read as not offered, never switched on, filled by a link, and
//! the choice setup offers once the account has signed in. No server listens on these ports, so
//! each session is recorded by hand, as a dial would.

use std::sync::{Arc, mpsc};

use engine_api::Capabilities as Offered;
use mailcal_account::Capability;

use super::JmapSession;
use crate::{
    AccountCapability, AccountEntry, CapabilityState, LogLevel, MailcalApp, MailcalError,
    SetupChoice,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const JMAP: &str = r#"
[jmap]
email = "alice@example.org"
base_url = "http://127.0.0.1:1"
password = "pw"
"#;

const CALENDAR: &str = r#"
[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice"
password = "pw"
"#;

const IMAP: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "bob@example.org"
password = "pw"
"#;

fn app(name: &str, configs: &[&str]) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs.iter().map(|config| (*config).to_owned()).collect(),
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots")
}

fn jmap_entry(app: &MailcalApp) -> AccountEntry {
    app.accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.kind == crate::AccountKind::Jmap)
        .expect("listed")
}

fn state(entry: &AccountEntry, capability: AccountCapability) -> CapabilityState {
    entry
        .uses
        .iter()
        .find(|row| row.capability == capability)
        .expect("listed")
        .state
}

/// Records `offered` as the JMAP account's session, as its dial would.
fn session(app: &MailcalApp, offered: Offered) -> String {
    let id = jmap_entry(app).id;
    app.registry
        .jmap_session(&id)
        .expect("a JMAP entry")
        .record(offered);
    id
}

#[test]
fn a_session_lacks_what_its_account_does_not_list() {
    let session = JmapSession::default();
    assert!(session.lacks().is_empty(), "nothing is known before a dial");

    session.record(Offered::none().with_mail().with_contacts());
    let lacks: Vec<Capability> = session.lacks().iter().collect();
    assert_eq!(lacks, [Capability::Calendar]);

    // A dial taken from the entry records into the entry.
    let dial = session.clone();
    dial.record(Offered::none().with_calendars());
    let lacks: Vec<Capability> = session.lacks().iter().collect();
    assert_eq!(lacks, [Capability::Mail, Capability::Contacts]);
}

#[test]
fn a_use_the_session_lacks_is_not_offered_and_cannot_be_switched_on() {
    let app = app("jmap-not-offered", &[JMAP]);
    let before = jmap_entry(&app);
    assert_eq!(
        state(&before, AccountCapability::Calendar),
        CapabilityState::On,
        "before a dial a use reads as chosen"
    );

    let id = session(&app, Offered::none().with_mail().with_contacts());
    let entry = jmap_entry(&app);
    assert_eq!(state(&entry, AccountCapability::Mail), CapabilityState::On);
    assert_eq!(
        state(&entry, AccountCapability::Calendar),
        CapabilityState::NotOffered
    );
    assert_eq!(
        state(&entry, AccountCapability::Contacts),
        CapabilityState::On
    );

    assert!(matches!(
        app.set_account_capability(id.clone(), AccountCapability::Calendar, true),
        Err(MailcalError::Config(_))
    ));
    app.set_account_capability(id.clone(), AccountCapability::Contacts, false)
        .expect("contacts off");
    // The calendar is still chosen, but a server without one cannot be all the account is for.
    assert!(matches!(
        app.set_account_capability(id, AccountCapability::Mail, false),
        Err(MailcalError::Config(_))
    ));
}

#[test]
fn a_jmap_account_without_a_calendar_can_link_one() {
    let app = app("jmap-links-calendar", &[JMAP, CALENDAR]);
    assert!(
        jmap_entry(&app).link_candidates.calendar.is_empty(),
        "an account with a calendar of its own links none"
    );

    session(&app, Offered::none().with_mail().with_contacts());
    let offered: Vec<String> = jmap_entry(&app)
        .link_candidates
        .calendar
        .into_iter()
        .map(|candidate| candidate.address)
        .collect();
    assert_eq!(offered, ["alice"]);
}

#[test]
fn setup_offers_what_the_session_offers_once_signed_in() {
    let app = app("jmap-signed-in-choices", &[JMAP, IMAP]);
    let id = session(&app, Offered::none().with_mail().with_contacts());
    let on = |capability| SetupChoice {
        capability,
        on: true,
        server_found: true,
    };
    assert_eq!(
        app.signed_in_setup_choices(id.clone()),
        [on(AccountCapability::Mail), on(AccountCapability::Contacts)]
    );

    session(&app, Offered::none().with_mail());
    assert!(
        app.signed_in_setup_choices(id).is_empty(),
        "one use is not a choice"
    );

    let imap = app
        .accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.kind == crate::AccountKind::Imap)
        .expect("listed");
    assert!(
        app.signed_in_setup_choices(imap.id).is_empty(),
        "a standards account chose before it connected"
    );
}
