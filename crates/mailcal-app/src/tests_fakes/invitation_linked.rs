//! A mail account without a calendar of its own, linked to a calendar account without mail: the
//! shape a mailbox and a calendar at different servers take (`docs/accounts.md` rule 12).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use engine_api::{AccountId, CalendarUserAddresses, EmailAddress, Engine, TimeZoneId};
use mailcal_account::AccountLinks;

use super::{InvitationFake, RecordingObserver};
use crate::{Account, App, Surface, Telemetry, TimeZoneInit};

/// The calendar account the mail account `acct-a` links to.
pub(crate) const CALENDAR_ACCOUNT: &str = "cal-b";

impl InvitationFake {
    /// A calendar server that schedules only as `addresses`.
    pub(crate) fn scheduling_as(mut self, addresses: &[&str]) -> Self {
        self.calendar_users =
            CalendarUserAddresses::Known(addresses.iter().map(|&a| a.to_owned()).collect());
        self
    }

    /// A transport that can also submit an iMIP message of its own, as SMTP beside a calendar
    /// can.
    pub(crate) fn sending_imip(mut self) -> Self {
        self.caps = self.caps.with_scheduling_submission();
        self
    }

    /// A calendar server that names only `addresses` and may schedule as others, as Graph and
    /// Google do.
    pub(crate) fn scheduling_as_at_least(mut self, addresses: &[&str]) -> Self {
        self.calendar_users =
            CalendarUserAddresses::Including(addresses.iter().map(|&a| a.to_owned()).collect());
        self
    }
}

/// The second mailbox [`shared_calendar_app`] files into the calendar, and the one the calendar
/// sends through.
pub(crate) const OTHER_MAILBOX: &str = "acct-c";

/// An app over `acct-a`, whose identity is `me@` and whose mail is `mail`, and the calendar
/// account `cal-b`, whose calendar is `calendar` and which signs in as `login@dav.test.local`.
/// With `linked`, the two are linked both ways, as Settings links them.
pub(crate) fn linked_invitation_app(
    mail: InvitationFake,
    calendar: InvitationFake,
    linked: bool,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
    prefs_path: Option<std::path::PathBuf>,
) -> App<InvitationFake> {
    let links = linked.then(|| {
        BTreeMap::from([
            ("acct-a".to_owned(), files_into_calendar()),
            (CALENDAR_ACCOUNT.to_owned(), sends_through("acct-a")),
        ])
    });
    let accounts = vec![
        mailbox("acct-a", "me@test.local", mail),
        calendar_account(calendar),
    ];
    app_over(accounts, links, surfaces, prefs_path)
}

/// [`linked_invitation_app`] linked, with a second mailbox `acct-c` (`other@`, mail `other`)
/// filing into the same calendar, which sends through `acct-c`.
pub(crate) fn shared_calendar_app(
    mail: InvitationFake,
    other: InvitationFake,
    calendar: InvitationFake,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
    prefs_path: Option<std::path::PathBuf>,
) -> App<InvitationFake> {
    let links = BTreeMap::from([
        ("acct-a".to_owned(), files_into_calendar()),
        (OTHER_MAILBOX.to_owned(), files_into_calendar()),
        (CALENDAR_ACCOUNT.to_owned(), sends_through(OTHER_MAILBOX)),
    ]);
    let accounts = vec![
        mailbox("acct-a", "me@test.local", mail),
        mailbox(OTHER_MAILBOX, "other@test.local", other),
        calendar_account(calendar),
    ];
    app_over(accounts, Some(links), surfaces, prefs_path)
}

fn mailbox(id: &str, identity: &str, mail: InvitationFake) -> Account<InvitationFake> {
    Account {
        id: AccountId::try_from(id).unwrap(),
        providers: vec![mail].into(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new(identity),
        dialled: true,
        uses_mail: true,
    }
}

fn calendar_account(calendar: InvitationFake) -> Account<InvitationFake> {
    Account {
        id: AccountId::try_from(CALENDAR_ACCOUNT).unwrap(),
        providers: Vec::new().into(),
        calendar_providers: vec![calendar],
        contact_providers: Vec::new(),
        identity: EmailAddress::new("login@dav.test.local"),
        dialled: true,
        uses_mail: false,
    }
}

fn files_into_calendar() -> AccountLinks {
    AccountLinks {
        calendar: Some(AccountId::try_from(CALENDAR_ACCOUNT).unwrap()),
        ..AccountLinks::default()
    }
}

fn sends_through(mail: &str) -> AccountLinks {
    AccountLinks {
        mail: Some(AccountId::try_from(mail).unwrap()),
        ..AccountLinks::default()
    }
}

fn app_over(
    accounts: Vec<Account<InvitationFake>>,
    links: Option<BTreeMap<String, AccountLinks>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
    prefs_path: Option<std::path::PathBuf>,
) -> App<InvitationFake> {
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    );
    if let Some(links) = links {
        app.set_links_source(Arc::new(move || links.clone()));
    }
    app
}
