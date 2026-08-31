//! Action- and settings-surface tests for [`super::App`]: archiving (by role, by
//! conventional name, whole-thread, optimistic removal), the display-timezone state machine,
//! per-account sync settings (push/poll, folder caps), and the persisted message-grouping and
//! per-account sync-depth preferences. The shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use engine_provider::MailEdit;
use fakes::{
    FakeProvider, account, app, app_with_prefs, flat_subjects, message, msg, thread_ref, threaded,
};
use mailcal_viewmodel::ViewMode;

use super::{Intent, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn archive_moves_the_message_to_the_archive_folder() {
    let provider = FakeProvider::with_archive(vec![message("m1", "a", "Quarterly report")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // Sync first so the message + the Archive folder are in the store.
    app.dispatch(Intent::RefreshMail).await;
    surfaces.lock().unwrap().clear();

    // Archiving forwards a MoveTo the Archive-role mailbox for that key, then re-syncs.
    app.dispatch(Intent::Archive {
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1);
    match &edits[0] {
        MailEdit::MoveTo {
            target,
            destination,
        } => {
            assert_eq!(target.as_str(), "m1");
            assert_eq!(destination.key().as_str(), "archive");
        }
        other => panic!("expected a MoveTo edit, got {other:?}"),
    }
}

#[tokio::test]
async fn archive_thread_moves_the_received_side_but_never_the_sent_copies() {
    // A conversation with two received messages (Inbox) and two of the owner's own copies in
    // Sent. Archiving the whole thread must move ONLY the two received messages to Archive; the
    // Sent copies are never moved out of Sent (so the thread still shows both sides from Archive).
    let provider = FakeProvider::with_sent_and_archive(vec![
        threaded("r1", "a", "Re: report", "t"),
        threaded("r2", "a", "Re: report", "t"),
        threaded("s1", "sent", "Re: report", "t"),
        threaded("s2", "sent", "Re: report", "t"),
    ]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ArchiveThread {
        thread: thread_ref("acct-1", "t"),
    })
    .await;

    // Exactly the two received messages moved, both to Archive; neither Sent copy was touched.
    let edits = edits.lock().unwrap();
    let moved: Vec<&str> = edits
        .iter()
        .map(|edit| match edit {
            MailEdit::MoveTo {
                target,
                destination,
            } => {
                assert_eq!(destination.key().as_str(), "archive", "moves go to Archive");
                target.as_str()
            }
            other => panic!("expected only MoveTo edits, got {other:?}"),
        })
        .collect();
    assert_eq!(moved.len(), 2, "only the two received messages move");
    assert!(moved.contains(&"r1") && moved.contains(&"r2"));
    assert!(
        !moved.contains(&"s1") && !moved.contains(&"s2"),
        "a Sent copy is never moved out of Sent"
    );
}

#[tokio::test]
async fn archive_falls_back_to_a_conventional_folder_name_when_the_role_is_untagged() {
    // The server didn't tag the "Archieven" folder with `\Archive`, so it has no role; archive
    // would otherwise be a silent no-op. It must resolve the destination by its conventional name.
    let provider = FakeProvider::with_named_archive(vec![message("m1", "a", "Quarterly report")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Archive {
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1);
    match &edits[0] {
        MailEdit::MoveTo {
            target,
            destination,
        } => {
            assert_eq!(target.as_str(), "m1");
            assert_eq!(destination.key().as_str(), "archief");
        }
        other => panic!("expected a MoveTo edit to the named archive, got {other:?}"),
    }
}

#[tokio::test]
async fn archive_falls_back_to_all_mail_when_the_account_has_no_archive_folder() {
    // Gmail has no Archive folder in any form: no `\Archive` role, and no conventionally
    // named one either; archiving there means leaving the Inbox, and the message's home is
    // the `\All` "All Mail" mailbox. Without this fallback the role and name lookups both
    // miss, `move_to_role` returns before building an edit, and archive is a **silent
    // no-op** on every Gmail account (observed in production: the reading pane advanced to
    // the next message and the archived one stayed in the list).
    let provider = FakeProvider::with_all_mail(vec![message("m1", "INBOX", "Quarterly report")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Archive {
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1, "archive must reach the provider, not no-op");
    match &edits[0] {
        MailEdit::MoveTo {
            target,
            destination,
        } => {
            assert_eq!(target.as_str(), "m1");
            assert_eq!(destination.key().as_str(), "ALL_MAIL");
        }
        other => panic!("expected a MoveTo edit to All Mail, got {other:?}"),
    }
}

#[tokio::test]
async fn archive_optimistically_removes_the_row_even_when_the_resync_still_reports_it() {
    // The fake never actually moves the message; its store keeps returning it in the inbox;
    // mirroring the real-world lag where the post-edit re-sync hasn't observed the expunge yet.
    // The archived row must still leave the list immediately and stay gone (the bug: it lingered
    // until a manual refresh).
    let provider = FakeProvider::with_archive(vec![
        message("m1", "a", "Quarterly report"),
        message("m2", "a", "Lunch plans"),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Quarterly report")
    );

    app.dispatch(Intent::Archive {
        message: msg("acct-1", "m1"),
    })
    .await;

    let subjects = flat_subjects(&app.mailbox_list());
    assert!(
        !subjects.iter().any(|s| s == "Quarterly report"),
        "archived row should leave the list immediately, got {subjects:?}"
    );
    assert!(
        subjects.iter().any(|s| s == "Lunch plans"),
        "unaffected rows stay, got {subjects:?}"
    );
}

#[tokio::test]
async fn timezone_intents_drive_the_settings_surface_and_reorder_the_agenda() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    assert_eq!(app.timezone_settings().active, "Etc/UTC");
    assert_eq!(app.timezone_settings().pending_device, None);

    // The OS reports a move to Amsterdam: a pending change is raised + a Settings signal,
    // but the active zone stays put until the user accepts.
    app.dispatch(Intent::ReportDeviceTimeZone("Europe/Amsterdam".to_owned()))
        .await;
    assert_eq!(
        app.timezone_settings().pending_device.as_deref(),
        Some("Europe/Amsterdam")
    );
    assert_eq!(app.timezone_settings().active, "Etc/UTC");
    assert_eq!(*surfaces.lock().unwrap(), vec![Surface::Settings]);

    // Accepting adopts the zone and re-orders the agenda (Settings + Calendar signals).
    surfaces.lock().unwrap().clear();
    app.dispatch(Intent::AcceptTimeZoneChange).await;
    assert_eq!(app.timezone_settings().active, "Europe/Amsterdam");
    assert_eq!(app.timezone_settings().pending_device, None);
    let seen = surfaces.lock().unwrap().clone();
    assert!(seen.contains(&Surface::Settings));
    assert!(seen.contains(&Surface::Calendar));

    // The explicit selector switches the active zone (and re-orders) too.
    app.dispatch(Intent::SetTimeZone("America/New_York".to_owned()))
        .await;
    assert_eq!(app.timezone_settings().active, "America/New_York");

    // A bogus zone is ignored: the active zone is unchanged.
    app.dispatch(Intent::SetTimeZone("Totally/Bogus".to_owned()))
        .await;
    assert_eq!(app.timezone_settings().active, "America/New_York");
}

#[tokio::test]
async fn sync_settings_defaults_to_polling_without_idle_support() {
    // A server that doesn't advertise IDLE: the account defaults to interval polling
    // (30 min), the push option is gated off, and the folder list is still reported so a
    // client can render it. The shared limits (5 folders, the interval set) come through.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct", FakeProvider::with(vec![]))], &surfaces);
    app.dispatch(Intent::RefreshMail).await; // populate the engine's folder list

    let settings = app.sync_settings().await;
    assert_eq!(settings.max_push_folders, 5);
    assert_eq!(settings.poll_intervals, vec![15, 30, 60, 90, 120]);
    let row = &settings.accounts[0];
    assert!(!row.idle_supported, "the fake server advertises no IDLE");
    assert_eq!(row.strategy, mailcal_viewmodel::SyncStrategyKind::Poll);
    assert_eq!(row.poll_interval_mins, 30);
    assert!(row.folders.iter().any(|f| f.key == "a"), "INBOX is listed");
    assert!(
        row.folders.iter().all(|f| !f.subscribed),
        "no folder is push-subscribed when polling",
    );
}

#[tokio::test]
async fn sync_settings_defaults_to_pushing_the_inbox_when_idle_supported() {
    // A server that advertises IDLE: the account defaults to push, with the Inbox
    // subscribed: the "receive emails as they come in" default the product calls for.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct", FakeProvider::with_idle(vec![]))],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    let row = &app.sync_settings().await.accounts[0];
    assert!(row.idle_supported);
    assert_eq!(row.strategy, mailcal_viewmodel::SyncStrategyKind::Push);
    let inbox = row
        .folders
        .iter()
        .find(|f| f.key == "a")
        .expect("inbox listed");
    assert!(
        inbox.subscribed,
        "the Inbox is watched by default under push"
    );
    assert!(!row.at_push_limit, "one subscribed folder is under the cap");
}

#[tokio::test]
async fn replacing_account_providers_refreshes_idle_support_settings() {
    // Interactive boot first lists provider-less placeholders, then the bindings replace each
    // account with live providers after the background dial. The settings UI must re-pull at that
    // point, otherwise it keeps the placeholder's "no IDLE" state while the watch is already live.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct", FakeProvider::with(vec![]))], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    assert!(!app.sync_settings().await.accounts[0].idle_supported);

    surfaces.lock().unwrap().clear();
    app.add_account_deferred(account("acct", FakeProvider::with_idle(vec![])))
        .await;

    let settings = app.sync_settings().await;
    assert!(settings.accounts[0].idle_supported);
    assert_eq!(
        settings.accounts[0].strategy,
        mailcal_viewmodel::SyncStrategyKind::Push
    );
    assert!(
        surfaces.lock().unwrap().contains(&Surface::Settings),
        "provider replacement must refresh the settings snapshot"
    );
}

#[tokio::test]
async fn set_push_folder_caps_subscriptions_at_the_limit() {
    // Subscribing past the five-folder cap is ignored, and the snapshot then reports the
    // account is at the limit so a client disables further toggles.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct", FakeProvider::with_idle(vec![]))],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    // The Inbox ("a") is already subscribed by default; add six more keys: only four more
    // fit (5 total), and the rest are dropped.
    for key in ["b", "c", "d", "e", "f", "g"] {
        app.set_push_folder("acct", key, true).await;
    }
    let row = &app.sync_settings().await.accounts[0];
    let subscribed = row.folders.iter().filter(|f| f.subscribed).count();
    // Folders b..g aren't in the engine's folder list, but the stored push set still caps
    // at five; the reported `at_push_limit` reflects that.
    assert!(row.at_push_limit, "the account is at the push-folder cap");
    // Only the Inbox ("a") exists as a real folder, so it's the one subscribed row shown.
    assert!(subscribed <= 5);
}

#[tokio::test]
async fn message_grouping_defaults_to_threaded_and_persists_across_a_relaunch() {
    // The grouping is now a persisted preference (default Threaded), not a runtime-only toggle.
    let dir = std::env::temp_dir().join("mailcal-grouping-persist-test");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("preferences.toml");
    let surfaces = Arc::new(Mutex::new(Vec::new()));

    // A fresh app (no preferences file yet) defaults to Threaded.
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );
    assert_eq!(app.view_mode(), ViewMode::Threaded);

    // Switching to Flat persists the choice and signals the settings surface.
    app.dispatch(Intent::SetViewMode(ViewMode::Flat)).await;
    assert!(surfaces.lock().unwrap().contains(&Surface::Settings));

    // A relaunch (a new app over the same preferences file) reads the stored Flat grouping.
    let relaunched = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );
    assert_eq!(relaunched.view_mode(), ViewMode::Flat);

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn per_account_sync_depth_overrides_only_that_account_and_survives_other_edits() {
    let dir = std::env::temp_dir().join("mailcal-account-depth-test");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("preferences.toml");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![
            account("acct-1", FakeProvider::with(vec![])),
            account("acct-2", FakeProvider::with(vec![])),
        ],
        path.clone(),
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await; // populate the engine's folder lists

    // Both accounts start at the product default depth (3 months); the picker options come through
    // unhardcoded.
    let before = app.sync_settings().await;
    assert_eq!(before.sync_depths, vec![3, 6, 9, 12, 24, 0]);
    assert!(before.accounts.iter().all(|r| r.sync_depth_months == 3));

    // Overriding acct-1 to 12 months affects only it; acct-2 still inherits the default.
    app.set_account_sync_depth("acct-1", 12).await;
    let after = app.sync_settings().await;
    let depth = |snap: &mailcal_viewmodel::SyncSettingsSnapshot, id: &str| {
        snap.accounts
            .iter()
            .find(|r| r.account_id == id)
            .unwrap()
            .sync_depth_months
    };
    assert_eq!(depth(&after, "acct-1"), 12);
    assert_eq!(depth(&after, "acct-2"), 3);
    assert_eq!(u16::from(app.effective_sync_depth("acct-1")), 12);

    // A later poll-interval change on acct-1 must not drop its depth override.
    app.set_poll_interval("acct-1", 60).await;
    assert_eq!(u16::from(app.effective_sync_depth("acct-1")), 12);

    // The other account remains at the product default until it gets its own explicit override.
    assert_eq!(depth(&after, "acct-2"), 3);

    let _ = std::fs::remove_dir_all(&dir);
}
