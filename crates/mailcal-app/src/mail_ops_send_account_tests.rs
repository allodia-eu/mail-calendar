//! Tests for which account a new message is written from when nothing on screen names one, and
//! for Settings → Composing's record of it: both read only the accounts used for mail. A child of
//! [`super`] (the send-from tests), reusing its `new_mail` and the parent's `ThreadProvider`.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Draft, EmailAddress, Engine, TimeZoneId};
use mailcal_viewmodel::{SendAccountOption, SendAccountSetting};

use super::{SilentObserver, ThreadProvider, new_mail};
use crate::{
    Account, App, ComposeOrigin, ComposePrefill, CompositionId, Intent, SendStatus, Telemetry,
    TimeZoneInit,
};

/// One account of a test app: its id, address and whether it is used for mail. An account not
/// used for mail has no mail provider at all, as a calendar-only account has none.
struct Spec(&'static str, &'static str, bool);

/// What one account's provider submitted.
type Outbox = Arc<Mutex<Vec<Draft>>>;

/// An app over `specs`, in the unified view, each account used for mail over a provider of its
/// own, with every account's outbox in order (one not used for mail never fills).
fn app_of(specs: &[Spec]) -> (Arc<App<ThreadProvider>>, Vec<Outbox>) {
    let mut accounts = Vec::new();
    let mut logs = Vec::new();
    for Spec(id, email, uses_mail) in specs {
        let provider = ThreadProvider::with(Vec::new());
        logs.push(provider.submissions());
        accounts.push(Account {
            id: AccountId::try_from(*id).unwrap(),
            providers: if *uses_mail {
                vec![provider].into()
            } else {
                Vec::new().into()
            },
            calendar_provider: None,
            meetings: None,
            contact_providers: Vec::new(),
            identity: EmailAddress::new(*email),
            dialled: true,
            uses_mail: *uses_mail,
        });
    }
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(SilentObserver),
        Telemetry::off(),
    );
    (Arc::new(app), logs)
}

/// A calendar-only account stored first, then one used for mail.
fn calendar_first() -> Arc<App<ThreadProvider>> {
    app_of(&[
        Spec("cal", "cal@allodia.local", false),
        Spec("mail", "mail@allodia.local", true),
    ])
    .0
}

async fn opened_from(
    app: &App<ThreadProvider>,
    origin: ComposeOrigin,
    prefill: Option<ComposePrefill>,
) -> String {
    let scratch = tempfile::tempdir().expect("a staging directory");
    let composition = CompositionId::new("composer-send-account").unwrap();
    app.open_composer(
        composition,
        origin,
        prefill,
        scratch.path().to_str().unwrap(),
    )
    .await
    .expect("the composer opens")
    .account
    .as_str()
    .to_owned()
}

fn id(raw: &str) -> AccountId {
    AccountId::try_from(raw).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_new_message_never_comes_from_an_account_not_used_for_mail() {
    let app = calendar_first();
    // The first account sends nothing, so the first that does is the answer.
    assert_eq!(
        opened_from(&app, ComposeOrigin::New { account: None }, None).await,
        "mail"
    );

    // Neither a stored default nor a named account makes it one.
    app.set_default_send_account(Some("cal".to_owned())).await;
    assert_eq!(
        opened_from(&app, ComposeOrigin::New { account: None }, None).await,
        "mail"
    );
    assert_eq!(
        opened_from(
            &app,
            ComposeOrigin::New {
                account: Some(id("cal"))
            },
            None
        )
        .await,
        "mail"
    );
    let prefill = ComposePrefill {
        account: Some(id("cal")),
        ..ComposePrefill::default()
    };
    assert_eq!(
        opened_from(&app, ComposeOrigin::New { account: None }, Some(prefill)).await,
        "mail"
    );
}

#[tokio::test(start_paused = true)]
async fn a_send_naming_no_account_goes_out_from_one_used_for_mail() {
    let (app, outboxes) = app_of(&[
        Spec("cal", "cal@allodia.local", false),
        Spec("mail", "mail@allodia.local", true),
    ]);

    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(new_mail(None)).await }
    });
    // Either outcome ends the wait, so a send from the wrong account fails here, not hangs.
    for _ in 0..10_000 {
        if matches!(app.send_status(), SendStatus::Sent | SendStatus::Failed) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(app.send_status(), SendStatus::Sent);
    task.await.unwrap();

    let sent = outboxes[1].lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].from.email, "mail@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn settings_offers_a_choice_only_between_two_or_more_accounts_used_for_mail() {
    let (none, _) = app_of(&[Spec("cal", "cal@allodia.local", false)]);
    assert_eq!(none.send_account_setting(), SendAccountSetting::NoAccount);

    assert_eq!(
        calendar_first().send_account_setting(),
        SendAccountSetting::Only {
            email: "mail@allodia.local".to_owned()
        }
    );

    let (two, _) = app_of(&[
        Spec("work", "work@allodia.local", true),
        Spec("cal", "cal@allodia.local", false),
        Spec("home", "home@allodia.local", true),
    ]);
    let choose = |selected: &str| SendAccountSetting::Choose {
        accounts: vec![
            SendAccountOption {
                id: "work".to_owned(),
                email: "work@allodia.local".to_owned(),
            },
            SendAccountOption {
                id: "home".to_owned(),
                email: "home@allodia.local".to_owned(),
            },
        ],
        selected: selected.to_owned(),
    };
    // Nothing chosen yet: the first account used for mail, which is what a new message takes.
    assert_eq!(two.send_account_setting(), choose("work"));
    two.dispatch(Intent::SetDefaultSendAccount(Some("home".to_owned())))
        .await;
    assert_eq!(two.send_account_setting(), choose("home"));
    // A stored choice that names no account used for mail reads as the one a message would take.
    two.dispatch(Intent::SetDefaultSendAccount(Some("cal".to_owned())))
        .await;
    assert_eq!(two.send_account_setting(), choose("work"));
    two.dispatch(Intent::SetDefaultSendAccount(Some("gone".to_owned())))
        .await;
    assert_eq!(two.send_account_setting(), choose("work"));
}

#[tokio::test(start_paused = true)]
async fn a_folder_on_screen_outranks_the_default_send_account() {
    let (app, _) = app_of(&[
        Spec("work", "work@allodia.local", true),
        Spec("home", "home@allodia.local", true),
    ]);
    app.dispatch(Intent::SetDefaultSendAccount(Some("home".to_owned())))
        .await;
    app.dispatch(Intent::SelectFolder {
        folder: crate::FolderRef::from_parts("work", "inbox".to_owned()).unwrap(),
    })
    .await;

    assert_eq!(
        opened_from(&app, ComposeOrigin::New { account: None }, None).await,
        "work"
    );
}

/// An assistant may write from another account than the one on screen: a prefill naming an
/// account that sends is the one the composer opens on.
#[tokio::test(start_paused = true)]
async fn a_prefill_names_the_account_to_write_from() {
    let (app, _) = app_of(&[
        Spec("work", "work@allodia.local", true),
        Spec("home", "home@allodia.local", true),
    ]);
    let prefill = ComposePrefill {
        account: Some(id("home")),
        ..ComposePrefill::default()
    };
    assert_eq!(
        opened_from(&app, ComposeOrigin::New { account: None }, Some(prefill)).await,
        "home"
    );
}
