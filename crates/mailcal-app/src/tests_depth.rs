//! Sync-depth lifecycle tests: new-account defaults, widening/narrowing, and account deletion.

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    ids::{MailboxId, MessageId},
    mail::{Mailbox, MailboxRole, Message},
    membership::Memberships,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
    time::{CalendarDate, UtcDateTime},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, Provider, ProviderResult, ScopeSync,
};
use mailcal_account::SyncDepth;

use super::{Account, App, AppObserver, Surface, Telemetry, TimeZoneInit};

struct SilentObserver;

impl AppObserver for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

struct WindowProvider {
    messages: Vec<Message>,
    caps: Capabilities,
}

impl WindowProvider {
    fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            caps: Capabilities::none().with_mail(),
        }
    }
}

#[async_trait::async_trait]
impl Provider for WindowProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Mailbox,
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Email,
        }
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("mbox-2"),
            ));
        }
        let mut inbox = Mailbox::new(MailboxId::try_from("inbox").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let present = [inbox.id.key().clone()].into();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![inbox], present),
            SyncState::new("mbox-1"),
        ))
    }

    fn stream_email<'a>(
        &'a self,
        _account: &AccountId,
        cursor: Option<&'a SyncState>,
        window: SyncWindow,
        _fetch_batch: usize,
        _chunk_size: usize,
    ) -> EmailStream<'a> {
        if cursor.is_some() {
            return Box::pin(futures::stream::iter(vec![Ok(EmailChunk::additive(
                Vec::new(),
                Vec::new(),
                None,
                SyncState::new("email-2"),
            ))]));
        }
        let messages: Vec<Message> = self
            .messages
            .iter()
            .filter(|message| in_window(message, window))
            .cloned()
            .collect();
        let present = messages.iter().map(|m| m.id.key().clone()).collect();
        Box::pin(futures::stream::iter(vec![Ok(EmailChunk::reconcile_last(
            messages,
            present,
            None,
            SyncState::new("email-1"),
        ))]))
    }
}

/// What a new account should start at on the target running the suite.
fn expected_default_depth() -> u16 {
    crate::form_factor::FormFactor::current().default_sync_depth_months()
}

/// The fixture keys that fall inside that default: the fixture is 1, 4 and 7 months old, so a
/// three-month default admits only the first and a six-month one the first two. "Old" is
/// outside either, which is the part that makes these assertions mean something on any target.
fn expected_default_keys() -> Vec<&'static str> {
    if expected_default_depth() >= 4 {
        vec!["middle", "recent"]
    } else {
        vec!["recent"]
    }
}

#[tokio::test]
async fn adding_a_new_account_takes_its_platform_default_and_syncs_only_that_window() {
    let app = test_app(Vec::new());

    app.add_account(account("acct", window_provider())).await;

    // The depth itself is the form factor's to decide (and is asserted there for both values);
    // what matters here is that a new account *takes* it, and syncs no wider than it.
    assert_eq!(account_depth(&app, "acct").await, expected_default_depth());
    assert_eq!(stored_keys(&app, "acct").await, expected_default_keys());
}

#[tokio::test]
async fn widening_sync_depth_clears_cursors_and_backfills_the_new_window() {
    let app = test_app(vec![account("acct", window_provider())]);
    app.dispatch(super::Intent::RefreshMail).await;
    assert_eq!(stored_keys(&app, "acct").await, vec!["recent"]);

    app.update_account_sync_depth("acct", 6).await;

    assert_eq!(account_depth(&app, "acct").await, 6);
    assert_eq!(stored_keys(&app, "acct").await, vec!["middle", "recent"]);
}

#[tokio::test]
async fn narrowing_sync_depth_clears_cursors_and_removes_out_of_window_mail() {
    let app = test_app(vec![account("acct", window_provider())]);
    app.set_account_sync_depth("acct", 6).await;
    app.dispatch(super::Intent::RefreshMail).await;
    assert_eq!(stored_keys(&app, "acct").await, vec!["middle", "recent"]);

    app.update_account_sync_depth("acct", 3).await;

    assert_eq!(account_depth(&app, "acct").await, 3);
    assert_eq!(stored_keys(&app, "acct").await, vec!["recent"]);
}

#[tokio::test]
async fn narrowing_sync_depth_removes_out_of_window_mail_with_the_network_down() {
    // Depth is what the user gets to say about how much mail is on *their device*, so it
    // must hold without a server. The re-snapshot reaches the same state when it can; if
    // it were the only path, "keep three months" would mean nothing until connectivity
    // returned, and the space would not come back at all.
    let app = test_app(vec![account("acct", window_provider())]);
    app.set_account_sync_depth("acct", 6).await;
    app.dispatch(super::Intent::RefreshMail).await;
    assert_eq!(stored_keys(&app, "acct").await, vec!["middle", "recent"]);
    app.dispatch(super::Intent::ReportNetworkReachable(false))
        .await;

    app.update_account_sync_depth("acct", 3).await;

    assert_eq!(stored_keys(&app, "acct").await, vec!["recent"]);
}

#[tokio::test]
async fn widening_sync_depth_with_the_network_down_keeps_the_mail_it_has() {
    // The prune runs only on a narrowing. A widen offline can fetch nothing, but it must
    // not mistake "I could not reach the wider window" for "drop what I already hold".
    let app = test_app(vec![account("acct", window_provider())]);
    app.dispatch(super::Intent::RefreshMail).await;
    app.dispatch(super::Intent::ReportNetworkReachable(false))
        .await;

    app.update_account_sync_depth("acct", 6).await;

    assert_eq!(account_depth(&app, "acct").await, 6);
    assert_eq!(stored_keys(&app, "acct").await, vec!["recent"]);
}

#[tokio::test]
async fn removing_an_account_forgets_engine_data_and_sync_settings() {
    let app = test_app(vec![account("acct", window_provider())]);
    app.set_account_sync_depth("acct", 6).await;
    app.dispatch(super::Intent::RefreshMail).await;
    assert_eq!(stored_keys(&app, "acct").await, vec!["middle", "recent"]);

    app.remove_account(&AccountId::try_from("acct").unwrap())
        .await;

    assert!(stored_keys(&app, "acct").await.is_empty());
    app.add_account(account("acct", window_provider())).await;
    assert_eq!(account_depth(&app, "acct").await, expected_default_depth());
    assert_eq!(stored_keys(&app, "acct").await, expected_default_keys());
}

fn test_app(accounts: Vec<Account<WindowProvider>>) -> App<WindowProvider> {
    App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::off(None),
    )
}

fn account(id: &str, provider: WindowProvider) -> Account<WindowProvider> {
    Account {
        id: AccountId::try_from(id).unwrap(),
        providers: vec![provider],
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new(format!("me@{id}.local")),
    }
}

fn window_provider() -> WindowProvider {
    WindowProvider::new(vec![
        dated_message("recent", "Recent mail", 1),
        dated_message("middle", "Middle mail", 4),
        dated_message("old", "Old mail", 7),
    ])
}

fn dated_message(key: &str, subject: &str, months_ago: u16) -> Message {
    let date = SyncDepth::Months(months_ago)
        .cutoff(time::OffsetDateTime::now_utc().date())
        .unwrap();
    let mut message = Message::new(
        MessageId::try_from(key).unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.subject = Some(subject.to_owned());
    message.received_at =
        Some(UtcDateTime::new(date.year(), u8::from(date.month()), date.day(), 12, 0, 0).unwrap());
    message
}

fn in_window(message: &Message, window: SyncWindow) -> bool {
    let Some(floor) = window.floor() else {
        return true;
    };
    message.received_at.is_none_or(|received| {
        CalendarDate::new(received.year(), received.month(), received.day())
            .is_ok_and(|date| date >= floor)
    })
}

async fn stored_keys(app: &App<WindowProvider>, account: &str) -> Vec<String> {
    let account = AccountId::try_from(account).unwrap();
    let mut keys: Vec<String> = app
        .engine
        .messages(&account)
        .await
        .unwrap()
        .into_iter()
        .map(|message| message.id.key().as_str().to_owned())
        .collect();
    keys.sort();
    keys
}

async fn account_depth(app: &App<WindowProvider>, account: &str) -> u16 {
    app.sync_settings()
        .await
        .accounts
        .into_iter()
        .find(|row| row.account_id == account)
        .map_or(0, |row| row.sync_depth_months)
}
