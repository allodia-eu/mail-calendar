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
}

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
    let accounts = vec![
        Account {
            id: AccountId::try_from("acct-a").unwrap(),
            providers: vec![mail],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@test.local"),
            dialled: true,
            uses_mail: true,
        },
        Account {
            id: AccountId::try_from(CALENDAR_ACCOUNT).unwrap(),
            providers: Vec::new(),
            calendar_providers: vec![calendar],
            contact_providers: Vec::new(),
            identity: EmailAddress::new("login@dav.test.local"),
            dialled: true,
            uses_mail: false,
        },
    ];
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
    if linked {
        app.set_links_source(Arc::new(|| {
            BTreeMap::from([
                (
                    "acct-a".to_owned(),
                    AccountLinks {
                        calendar: Some(AccountId::try_from(CALENDAR_ACCOUNT).unwrap()),
                        ..AccountLinks::default()
                    },
                ),
                (
                    CALENDAR_ACCOUNT.to_owned(),
                    AccountLinks {
                        mail: Some(AccountId::try_from("acct-a").unwrap()),
                        ..AccountLinks::default()
                    },
                ),
            ])
        }));
    }
    app
}
