//! Preferences as a host reaches them: changed by intent, read back at once, saved behind, and a
//! save that failed said as a notice whose Try again writes it.

use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    Appearance, Intent, LogLevel, MailcalApp, NoticeAction, NoticeIntent, NoticeKind,
    NoticeScreens, NoticeSeverity, PreferenceIntent, Surface,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

fn app(name: &str) -> (Arc<MailcalApp>, PathBuf, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let dir = crate::tests::temp_data_dir(name);
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots");
    // A first launch saves the display zone behind the caller; written before a test stages
    // its own file beside it, or the two race.
    app.app.settle_preferences();
    (app, dir, rx)
}

fn set(app: &MailcalApp, intent: PreferenceIntent) {
    app.dispatch(Intent::Preferences { intent });
}

fn eventually(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    done()
}

/// A change is what the next read says before `dispatch` returns, and is in the file once the
/// host has said it is closing.
#[test]
fn a_change_is_read_at_once_and_on_disk_when_the_app_closes() {
    let (app, dir, _rx) = app("preferences-at-once");
    set(
        &app,
        PreferenceIntent::SetAppearance {
            appearance: Appearance::Dark,
        },
    );
    set(&app, PreferenceIntent::SetCalendarVisibleHours { hours: 9 });
    let settings = app.display_settings();
    assert_eq!(settings.appearance, Appearance::Dark);
    assert_eq!(settings.visible_hours, 9);
    assert_eq!(settings.visible_hours_choices, [6, 8, 9, 12, 16, 24]);

    app.finish_saving_settings();
    let stored = mailcal_account::load_preferences(mailcal_account::preferences_path(&dir));
    assert_eq!(stored.appearance, mailcal_account::Appearance::Dark);
    assert_eq!(stored.calendar_visible_hours, 9);
}

/// ⚠️ A save that fails is a notice, never silence: the change holds, the notice offers Try
/// again, and Try again writes it and the notice goes.
#[test]
fn a_failed_save_is_a_notice_whose_try_again_writes_it() {
    let (app, dir, rx) = app("preferences-failed");
    // The save writes beside the file and renames over it; a directory in the way refuses it.
    let staged = dir.join("preferences.toml.saving");
    std::fs::create_dir_all(&staged).unwrap();
    set(
        &app,
        PreferenceIntent::SetAppearance {
            appearance: Appearance::Light,
        },
    );
    assert!(eventually(|| app.app.preferences_not_saved()));
    assert_eq!(app.display_settings().appearance, Appearance::Light);
    assert!(eventually(|| rx
        .try_iter()
        .any(|s| matches!(s, Surface::Notices))));

    let notices = app.notices().notices;
    let notice = notices
        .iter()
        .find(|notice| notice.kind == NoticeKind::SettingsNotSaved)
        .expect("the failed save is a notice");
    assert_eq!(notice.severity, NoticeSeverity::Degraded);
    assert_eq!(notice.screens, NoticeScreens::Everywhere);
    assert_eq!(notice.action, Some(NoticeAction::Retry));
    assert_eq!(notice.title.key, "notice_settings_not_saved");
    let id = notice.id.clone();

    std::fs::remove_dir(&staged).unwrap();
    app.dispatch(Intent::Notices {
        intent: NoticeIntent::Retry { id },
    });
    assert!(eventually(|| !app.app.preferences_not_saved()));
    assert!(eventually(|| app
        .notices()
        .notices
        .iter()
        .all(|notice| notice.kind != NoticeKind::SettingsNotSaved)));
    let stored = mailcal_account::load_preferences(mailcal_account::preferences_path(&dir));
    assert_eq!(stored.appearance, mailcal_account::Appearance::Light);
}

/// A Try again that fails again says so on the notice.
#[test]
fn a_try_again_that_fails_again_is_said_on_the_notice() {
    let (app, dir, _rx) = app("preferences-failed-twice");
    std::fs::create_dir_all(dir.join("preferences.toml.saving")).unwrap();
    set(
        &app,
        PreferenceIntent::SetDarkMessageBackground { dark: false },
    );
    assert!(eventually(|| app.app.preferences_not_saved()));
    let id = app
        .notices()
        .notices
        .into_iter()
        .find(|notice| notice.kind == NoticeKind::SettingsNotSaved)
        .expect("the failed save is a notice")
        .id;
    app.dispatch(Intent::Notices {
        intent: NoticeIntent::Retry { id: id.clone() },
    });
    assert!(eventually(|| app
        .notices()
        .notices
        .iter()
        .any(|notice| notice.id == id && notice.remedy_failed)));
}

/// The analytics decision raises Settings like every other preference, so no client repeats it
/// into a copy of its own.
#[test]
fn the_analytics_decision_signals_settings() {
    let (app, _dir, rx) = app("preferences-analytics");
    while rx.try_recv().is_ok() {}
    set(
        &app,
        PreferenceIntent::SetAnalyticsConsent { enabled: false },
    );
    assert!(
        rx.try_iter()
            .any(|surface| matches!(surface, Surface::Settings))
    );
    let consent = app.analytics_consent();
    assert!(consent.asked && !consent.enabled);

    // The decision in force, again, changes nothing and says nothing.
    set(
        &app,
        PreferenceIntent::SetAnalyticsConsent { enabled: false },
    );
    assert!(
        rx.try_iter()
            .all(|surface| !matches!(surface, Surface::Settings))
    );
}
