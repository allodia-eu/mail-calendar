//! A write of one of the assistant's three files that fails is main's settings-not-saved notice,
//! whose Try again writes it (`docs/ai.md`, "Storage"): the `[ai]` preferences, the style library
//! and the drafted replies' log. Clients say nothing about it on the surface that changed it.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use mailcal_account::StoredWritingStyle;

use crate::{
    AiEndpointIntent, AiEndpointResult, AiIntent, Intent, JurisdictionClass, LogLevel, MailcalApp,
    NoticeAction, NoticeIntent, NoticeKind, NoticeScreens, NoticeSeverity,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

fn app(name: &str) -> (Arc<MailcalApp>, PathBuf) {
    let (tx, _rx) = mpsc::channel();
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
    // A first launch saves the display zone behind the caller; settled before a test stages a
    // refusal beside the file, or the two race.
    app.app.settle_preferences();
    (app, dir)
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

/// A write of `file` is refused while this lives: it writes beside the file and renames over it,
/// and a directory in the way refuses that.
fn refuse_writes(dir: &Path, file: &str) -> PathBuf {
    let staged = dir.join(format!("{file}.saving"));
    std::fs::create_dir_all(&staged).unwrap();
    staged
}

/// The notice is raised as main raises it, and its Try again, once the disk takes the write,
/// writes `file` and takes the notice away.
fn the_notice_retries(app: &MailcalApp, dir: &Path, file: &str, staged: &Path) -> String {
    assert!(eventually(|| app.app.preferences_not_saved()), "{file}");
    let notice = app
        .notices()
        .notices
        .into_iter()
        .find(|notice| notice.kind == NoticeKind::SettingsNotSaved)
        .unwrap_or_else(|| panic!("{file}: the failed write is the notice"));
    assert_eq!(notice.severity, NoticeSeverity::Degraded);
    assert_eq!(notice.screens, NoticeScreens::Everywhere);
    assert_eq!(notice.action, Some(NoticeAction::Retry));

    std::fs::remove_dir(staged).unwrap();
    app.dispatch(Intent::Notices {
        intent: NoticeIntent::Retry { id: notice.id },
    });
    assert!(eventually(|| !app.app.preferences_not_saved()), "{file}");
    assert!(eventually(|| app
        .notices()
        .notices
        .iter()
        .all(|notice| notice.kind != NoticeKind::SettingsNotSaved)));
    std::fs::read_to_string(dir.join(file)).unwrap()
}

#[test]
fn an_endpoint_save_the_preferences_refused_is_the_notice_and_try_again_writes_it() {
    let (app, dir) = app("ai-notice-preferences");
    let staged = refuse_writes(&dir, "preferences.toml");
    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: "https://models.example.eu/v1".to_owned(),
                model: "local-model".to_owned(),
                declared: Some(JurisdictionClass::EuNative),
                key: String::new(),
            },
        },
    });
    assert!(eventually(|| app
        .ai_endpoint_form()
        .and_then(|form| form.outcome)
        .is_some_and(
            |outcome| outcome.result == AiEndpointResult::NotWritten
        )));
    let written = the_notice_retries(&app, &dir, "preferences.toml", &staged);
    assert!(
        written.contains("models.example.eu"),
        "the [ai] table: {written}"
    );
}

#[test]
fn a_library_the_disk_refused_is_the_notice_and_try_again_writes_it() {
    let (app, dir) = app("ai-notice-styles");
    let staged = refuse_writes(&dir, "writing_styles.toml");
    let id = app.app.add_writing_style(
        None,
        StoredWritingStyle {
            name: "Work".to_owned(),
            ..StoredWritingStyle::default()
        },
    );
    let written = the_notice_retries(&app, &dir, "writing_styles.toml", &staged);
    assert!(written.contains(id.as_str()), "{written}");
}

#[test]
fn a_drafted_reply_log_the_disk_refused_is_the_notice_and_try_again_writes_it() {
    let (app, dir) = app("ai-notice-drafted");
    let staged = refuse_writes(&dir, "drafted_replies.toml");
    app.app.note_drafted_reply(
        "acct-1",
        engine_api::MessageIdHeader::new("<drafted@example.eu>").unwrap(),
    );
    let written = the_notice_retries(&app, &dir, "drafted_replies.toml", &staged);
    assert!(written.contains("drafted@example.eu"), "{written}");
}
