//! Settings → Notifications: what a pass announces while new-mail notifications are off, and
//! after they are turned on again.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_core::mail::Message;

use crate::{Surface, tests_fakes as fakes};

const BUDGET: Duration = Duration::from_secs(5);

/// A message received `hours_ago`, inside the sync window whatever today is.
fn received(id: &str, hours_ago: i64) -> Message {
    let at = time::OffsetDateTime::now_utc() - time::Duration::hours(hours_ago);
    let mut message = fakes::message(id, "a", id);
    message.received_at = Some(
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            at.year(),
            u8::from(at.month()),
            at.day(),
            at.hour(),
            at.minute(),
            at.second()
        )
        .parse()
        .unwrap(),
    );
    message
}

fn announced(pass: &super::BackgroundNewMail) -> Vec<&str> {
    pass.accounts
        .iter()
        .flat_map(|account| account.messages.iter())
        .map(|preview| preview.message_key.as_str())
        .collect()
}

#[tokio::test]
async fn mail_that_arrives_while_notifications_are_off_is_never_announced() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = fakes::FakeProvider::with(vec![received("m1", 3)]);
    let late = provider.late_delivery();
    let app = fakes::app_with_prefs(
        vec![fakes::account("acct-1", provider)],
        scratch.path().join("preferences.toml"),
        &surfaces,
    );
    assert!(
        app.new_mail_notifications(),
        "on until someone turns them off"
    );
    assert_eq!(announced(&app.run_background_sync(BUDGET).await), [""; 0]);

    app.set_new_mail_notifications(false).await;
    assert!(matches!(
        surfaces.lock().unwrap().last(),
        Some(Surface::Settings)
    ));
    late.lock().unwrap().push(received("m2", 2));
    assert_eq!(announced(&app.run_background_sync(BUDGET).await), [""; 0]);

    app.set_new_mail_notifications(true).await;
    late.lock().unwrap().push(received("m3", 1));
    assert_eq!(
        announced(&app.run_background_sync(BUDGET).await),
        ["m3"],
        "m2 arrived while they were off, so it counts as seen",
    );
}

#[tokio::test]
async fn turning_notifications_off_survives_a_relaunch() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let path = scratch.path().join("preferences.toml");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = fakes::app_with_prefs(Vec::new(), path.clone(), &surfaces);
    app.set_new_mail_notifications(false).await;
    assert!(app.settle_preferences_within(Duration::from_secs(5)));
    drop(app);

    let reopened = fakes::app_with_prefs(Vec::new(), path, &surfaces);
    assert!(!reopened.new_mail_notifications());
}
