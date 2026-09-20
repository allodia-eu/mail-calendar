//! Tests for the **sender name**: the `Name` in `Name <address>`, which is what a recipient
//! actually reads. Until it existed, every IMAP account sent as a bare address.
//!
//! Asserted here rather than in `mailcal-account` because this is where the two halves meet:
//! the stored preference and the `Draft` the outbox submits. A test of the preference alone
//! would pass just as well against a send path that never read it, which is the shape of the
//! bug.
//!
//! A child of [`super`] (the rich reply/forward tests), reusing its `ThreadProvider` and
//! two-account fixtures; its own file to keep each module under the 500-line limit.

use std::sync::Arc;

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use mailcal_account::{Preferences, save_preferences};

use super::{SilentObserver, ThreadProvider, dispatch_until, from_account::new_mail};
use crate::{Account, App, SendStatus, Telemetry, TimeZoneInit};

/// A one-account app over `prefs`, so a test can set a name before the app boots and prove
/// the value is read from disk rather than only from the setter that wrote it.
fn app_with_prefs(
    prefs_path: Option<std::path::PathBuf>,
) -> (
    Arc<App<ThreadProvider>>,
    Arc<std::sync::Mutex<Vec<engine_api::Draft>>>,
) {
    let provider = ThreadProvider::with(Vec::new());
    let outbox = provider.submissions();
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![provider],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path,
        },
        None,
        Arc::new(SilentObserver),
        Telemetry::off(None),
    );
    (Arc::new(app), outbox)
}

/// A fresh preferences file of its own, so one test's writes cannot reach another's.
fn scratch_prefs(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-sender-name-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("preferences.toml");
    save_preferences(&path, &Preferences::default()).unwrap();
    path
}

#[tokio::test(start_paused = true)]
async fn an_account_with_no_name_still_sends_as_a_bare_address() {
    // The state every account starts in, and the one that must stay valid: a name is
    // something the user supplies, never something the app invents from the address.
    let (app, outbox) = app_with_prefs(None);

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    let draft = outbox.lock().unwrap()[0].clone();
    assert_eq!(draft.from.email, "me@allodia.local");
    assert_eq!(draft.from.name, None);
}

#[tokio::test(start_paused = true)]
async fn a_sent_message_carries_the_name_the_user_set() {
    let (app, outbox) = app_with_prefs(Some(scratch_prefs("send")));
    app.set_account_sender_name("acct-1", "Ada Lovelace").await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    let draft = outbox.lock().unwrap()[0].clone();
    assert_eq!(draft.from.name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        draft.from.email, "me@allodia.local",
        "a name is added to the address, never instead of it"
    );
}

#[tokio::test(start_paused = true)]
async fn a_name_set_in_an_earlier_session_is_read_back_at_boot() {
    // The persisted half. An app that only knew the name because it had just been told
    // would send correctly today and as a bare address tomorrow.
    let path = scratch_prefs("reload");
    let (first, _) = app_with_prefs(Some(path.clone()));
    first
        .set_account_sender_name("acct-1", "Ada Lovelace")
        .await;
    drop(first);

    let (app, outbox) = app_with_prefs(Some(path));
    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    let draft = outbox.lock().unwrap()[0].clone();
    assert_eq!(draft.from.name.as_deref(), Some("Ada Lovelace"));
}

#[tokio::test(start_paused = true)]
async fn a_pasted_header_cannot_reach_the_wire_through_the_name() {
    // The engine's assembler refuses a control character too, but by then the user has a
    // mailbox that will not send and no idea why. Sanitising on store is what keeps anyone
    // from reaching that.
    let (app, outbox) = app_with_prefs(Some(scratch_prefs("injection")));
    app.set_account_sender_name("acct-1", "Ada\r\nBcc: eve@example.com")
        .await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    let draft = outbox.lock().unwrap()[0].clone();
    let name = draft.from.name.expect("the name survives, flattened");
    assert!(!name.contains('\r') && !name.contains('\n'), "{name:?}");
    assert_eq!(draft.bcc, Vec::new(), "the paste added no recipient");
}

#[tokio::test(start_paused = true)]
async fn clearing_the_name_goes_back_to_a_bare_address() {
    let (app, outbox) = app_with_prefs(Some(scratch_prefs("clear")));
    app.set_account_sender_name("acct-1", "Ada Lovelace").await;
    app.set_account_sender_name("acct-1", "").await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    assert_eq!(outbox.lock().unwrap()[0].from.name, None);
}

#[tokio::test]
async fn the_account_row_carries_the_name_beside_the_address() {
    // What every client renders. `email` stays the address so no existing label breaks, and
    // `name` is empty rather than a copy of the address when nobody has set one.
    let (app, _) = app_with_prefs(Some(scratch_prefs("row")));
    // The rows are published by a rebuild, not by construction.
    app.prime_snapshot().await;

    let before = app.mailbox_list().accounts;
    assert_eq!(before[0].email, "me@allodia.local");
    assert_eq!(before[0].name, "");

    app.set_account_sender_name("acct-1", "Ada Lovelace").await;

    let after = app.mailbox_list().accounts;
    assert_eq!(after[0].name, "Ada Lovelace");
    assert_eq!(after[0].email, "me@allodia.local");
}

#[tokio::test]
async fn removing_an_account_forgets_the_name_it_sent_under() {
    // A re-added id must not inherit a name the user chose for a different mailbox: it is
    // what recipients see, so an inherited one misrepresents the sender.
    let path = scratch_prefs("removal");
    let (app, _) = app_with_prefs(Some(path.clone()));
    app.set_account_sender_name("acct-1", "Ada Lovelace").await;

    app.remove_account(&AccountId::try_from("acct-1").unwrap())
        .await;

    assert_eq!(
        mailcal_account::load_preferences(&path).sender_name_of("acct-1"),
        None
    );
}

/// A provider that reports one send-as identity, so the setup flow's "does this account still
/// need a name" question has a server answer to read. `name` is the display name the server
/// holds; `None` is a provider that knows the address and no name for it.
#[derive(Debug)]
struct IdentityProvider {
    name: Option<String>,
}

#[async_trait::async_trait]
impl engine_provider::Provider for IdentityProvider {
    fn connection_info(&self) -> engine_provider::ConnectionInfo {
        engine_provider::ConnectionInfo::new(
            engine_provider::Capabilities::none()
                .with_mail()
                .with_sender_identities(engine_provider::IdentityControls::Writable),
        )
    }

    async fn sender_identities(
        &self,
        _account: &AccountId,
    ) -> engine_provider::ProviderResult<Vec<engine_provider::SenderIdentity>> {
        let mut address = EmailAddress::new("me@allodia.local");
        address.name = self.name.clone();
        Ok(vec![engine_provider::SenderIdentity::new(
            engine_provider::SenderIdentityId::new("send-as-1"),
            address,
        )])
    }
}

impl engine_provider::CalendarWrites for IdentityProvider {}

/// A one-account app whose provider holds `name` as the server's own display name.
fn app_with_identity(
    name: Option<&str>,
    prefs_path: std::path::PathBuf,
) -> Arc<App<IdentityProvider>> {
    Arc::new(App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![IdentityProvider {
                name: name.map(str::to_owned),
            }],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: Some(prefs_path),
        },
        None,
        Arc::new(SilentObserver),
        Telemetry::off(None),
    ))
}

#[tokio::test]
async fn a_name_the_provider_already_holds_is_adopted_and_not_asked_for() {
    // The step the user would have seen here has a pre-filled answer and nothing to decide.
    // Adopting rather than merely skipping is the half that matters: the `From` reads the
    // stored name, so a skip that stored nothing sends as a bare address while Gmail's own
    // client shows a name.
    let app = app_with_identity(Some("Ada Lovelace"), scratch_prefs("adopt"));
    let account = AccountId::try_from("acct-1").unwrap();

    assert!(!app.needs_sender_name(&account).await);
    assert_eq!(app.mailbox_list().accounts[0].name, "Ada Lovelace");
}

#[tokio::test]
async fn a_provider_holding_no_name_is_still_asked() {
    // The ordinary IMAP case, and the one the step exists for.
    let app = app_with_identity(None, scratch_prefs("ask"));
    let account = AccountId::try_from("acct-1").unwrap();

    assert!(app.needs_sender_name(&account).await);
    assert_eq!(
        app.sender_name("acct-1"),
        None,
        "asking is not the same as storing an empty name"
    );
}

#[tokio::test]
async fn a_provider_name_of_nothing_but_control_characters_is_not_a_name() {
    // Sanitising decides what a name is, so a server answer that survives as empty leaves
    // the account in the state that still needs asking rather than in a named one.
    let app = app_with_identity(Some("\u{7}\u{7}"), scratch_prefs("blank"));
    let account = AccountId::try_from("acct-1").unwrap();

    assert!(app.needs_sender_name(&account).await);
}

#[tokio::test]
async fn a_name_the_user_already_set_is_never_replaced_by_the_provider_s() {
    // The user's own answer outranks the server's, and the question is not asked again.
    let app = app_with_identity(Some("Directory Name"), scratch_prefs("prefer-stored"));
    let account = AccountId::try_from("acct-1").unwrap();
    app.set_account_sender_name("acct-1", "Ada Lovelace").await;

    assert!(!app.needs_sender_name(&account).await);
    assert_eq!(app.mailbox_list().accounts[0].name, "Ada Lovelace");
}

#[tokio::test]
async fn a_suggestion_is_sanitised_before_it_reaches_the_field() {
    // The provider's answer is offered, not only stored, so it passes the same rule: a field
    // prefilled with control characters shows a name that sanitises back to nothing on save.
    let app = app_with_identity(
        Some("Ada\r\nBcc: eve@example.com"),
        scratch_prefs("suggest"),
    );
    let account = AccountId::try_from("acct-1").unwrap();

    let suggestion = app.suggested_sender_name(&account).await;

    assert!(
        !suggestion.contains('\r') && !suggestion.contains('\n'),
        "{suggestion:?}"
    );
}
