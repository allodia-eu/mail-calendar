//! Asking an account's calendar server which addresses it schedules as.

use std::sync::Arc;

use engine_api::{AccountId, CalendarUserAddresses, EmailAddress, Engine, TimeZoneId};

use crate::{Account, App, AppObserver, Surface, Telemetry, TimeZoneInit};

struct SilentObserver;

impl AppObserver for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

/// A calendar provider whose server answers with `answer`.
#[derive(Debug)]
struct CalendarServer {
    answer: CalendarUserAddresses,
}

#[async_trait::async_trait]
impl engine_provider::Provider for CalendarServer {
    fn connection_info(&self) -> engine_provider::ConnectionInfo {
        engine_provider::ConnectionInfo::new(engine_provider::Capabilities::none().with_calendars())
    }
}

impl engine_api::MailboxWrites for CalendarServer {}

#[async_trait::async_trait]
impl engine_provider::CalendarWrites for CalendarServer {
    async fn calendar_user_addresses(
        &self,
        _account: &AccountId,
    ) -> engine_provider::ProviderResult<CalendarUserAddresses> {
        Ok(self.answer.clone())
    }
}

fn app(calendar: Option<CalendarServer>) -> Arc<App<CalendarServer>> {
    Arc::new(App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("alice@dav:cloud.example").unwrap(),
            providers: Vec::new(),
            calendar_providers: calendar.into_iter().collect(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("alice@cloud.example"),
            dialled: true,
            uses_mail: false,
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(SilentObserver),
        Telemetry::off(None),
    ))
}

#[tokio::test]
async fn the_calendar_server_says_which_addresses_it_schedules_as() {
    let app = app(Some(CalendarServer {
        answer: CalendarUserAddresses::Known(vec!["alice@example.org".to_owned()]),
    }));
    let addresses = app
        .calendar_user_addresses(&AccountId::try_from("alice@dav:cloud.example").unwrap())
        .await
        .expect("a calendar to ask");
    assert!(addresses.recognises("Alice@Example.org"));
    assert!(!addresses.recognises("bob@example.org"));
}

#[tokio::test]
async fn an_account_without_a_calendar_has_nobody_to_ask() {
    let app = app(None);
    assert!(
        app.calendar_user_addresses(&AccountId::try_from("alice@dav:cloud.example").unwrap())
            .await
            .is_none()
    );
}
