//! Asking an account's calendar server which addresses it schedules as.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
    asked: Arc<AtomicUsize>,
}

impl CalendarServer {
    fn answering(answer: CalendarUserAddresses) -> Self {
        Self {
            answer,
            asked: Arc::default(),
        }
    }
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
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(self.answer.clone())
    }
}

fn account(calendar: Option<CalendarServer>) -> Account<CalendarServer> {
    Account {
        id: AccountId::try_from("alice@dav:cloud.example").unwrap(),
        providers: Vec::new(),
        calendar_providers: calendar.into_iter().collect(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("alice@cloud.example"),
        dialled: true,
        uses_mail: false,
    }
}

fn app(calendar: Option<CalendarServer>) -> Arc<App<CalendarServer>> {
    Arc::new(App::new(
        Engine::open_in_memory().unwrap(),
        vec![account(calendar)],
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
    let app = app(Some(CalendarServer::answering(
        CalendarUserAddresses::Known(vec!["alice@example.org".to_owned()]),
    )));
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

#[tokio::test]
async fn the_server_is_asked_once_per_connection() {
    let server = CalendarServer::answering(CalendarUserAddresses::Known(vec![
        "alice@example.org".to_owned(),
    ]));
    let asked = Arc::clone(&server.asked);
    let app = app(Some(server));
    let id = AccountId::try_from("alice@dav:cloud.example").unwrap();
    app.calendar_user_addresses(&id).await;
    app.calendar_user_addresses(&id).await;
    assert_eq!(asked.load(Ordering::SeqCst), 1);

    let again = CalendarServer::answering(CalendarUserAddresses::NotEnabled);
    let asked_again = Arc::clone(&again.asked);
    app.add_account_deferred(account(Some(again))).await;
    assert_eq!(
        app.calendar_user_addresses(&id).await,
        Some(CalendarUserAddresses::NotEnabled),
        "an account connected again is asked again"
    );
    assert_eq!(asked_again.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_server_that_could_not_say_is_asked_again() {
    let server = CalendarServer::answering(CalendarUserAddresses::Unknown);
    let asked = Arc::clone(&server.asked);
    let app = app(Some(server));
    let id = AccountId::try_from("alice@dav:cloud.example").unwrap();
    app.calendar_user_addresses(&id).await;
    app.calendar_user_addresses(&id).await;
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}
