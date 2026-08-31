//! Message-action tests for [`super::App`]: action routing to the owning account on a
//! provider-key collision, opening/reading a message body (sanitise, inline images,
//! attachments, recipients), marking read, and reply/forward subject formatting. The body
//! warm lives in `tests_warm.rs`; the mark-read-on-open + cold-open-retry behaviour lives in
//! `tests_reading.rs`; the calendar write-action tests live in `tests_calendar_actions.rs`. The
//! shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_api::EmailAddress;
use engine_provider::MailEdit;
use fakes::{
    FakeConnector, FakeProvider, account, app, app_with_connector, message, msg, open_folder,
};

use super::{Intent, Surface};
use crate::helpers::{forward_subject, reply_subject};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn a_message_action_routes_by_owning_account_on_a_key_collision() {
    // Two accounts whose stores BOTH hold a message with the SAME provider key
    // ("shared"); IMAP keys (`imap:v{validity}:u{uid}@{mailbox}`) are only unique
    // within an account, so two accounts colliding on a key is real, not contrived. An
    // action carrying account B's id must affect ONLY B: the old all-account scan
    // returned the FIRST match (account A) and would have misrouted B's action to A.
    let a = FakeProvider::with(vec![message("shared", "a", "A's message")]);
    let b = FakeProvider::with(vec![message("shared", "a", "B's message")]);
    let a_edits = a.edits();
    let b_edits = b.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-a", a), account("acct-b", b)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // Mark the shared key read FOR ACCOUNT B: only B's provider receives the edit, even
    // though account A (synced first) holds the same key.
    app.dispatch(Intent::MarkRead {
        message: msg("acct-b", "shared"),
        read: true,
    })
    .await;
    assert_eq!(
        b_edits.lock().unwrap().len(),
        1,
        "B's provider got the edit"
    );
    assert!(
        a_edits.lock().unwrap().is_empty(),
        "A's provider was untouched"
    );
}

#[tokio::test]
async fn mark_read_routes_to_the_owning_account() {
    let work = FakeProvider::with(vec![message("w1", "a", "Work")]);
    let home = FakeProvider::with(vec![message("h1", "a", "Home")]);
    let work_edits = work.edits();
    let home_edits = home.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("work", work), account("home", home)],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    // Marking a home message read routes the edit to the home provider only.
    app.dispatch(Intent::MarkRead {
        message: msg("home", "h1"),
        read: true,
    })
    .await;
    assert_eq!(home_edits.lock().unwrap().len(), 1);
    assert!(work_edits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn open_message_fetches_sanitizes_and_publishes_the_body() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    // Sync first so the message is in the store and `find_message` resolves the key.
    app.dispatch(Intent::RefreshMail).await;
    surfaces.lock().unwrap().clear();

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    // The reading surface fired FIRST, carrying the opened key. (The fixture message is unread,
    // so the auto-mark-read then re-syncs in the background; its SyncProgress/MailboxList
    // signals follow the Reading one, which is what the open itself publishes.)
    assert_eq!(surfaces.lock().unwrap().first(), Some(&Surface::Reading));
    let reading = app.reading_view();
    assert_eq!(reading.key, "m1");

    // The HTML crossed through sanitised: the script is gone, presentational markup kept,
    // and the remote image is kept but flagged (the WebView CSP gates the load).
    let html = reading.html.expect("an html body");
    assert!(html.contains("summary"));
    assert!(!html.contains("<script"));
    assert!(html.contains("tracker.example"));
    assert!(reading.has_remote_images);
    assert!(reading.plain.is_some());
    assert!(!reading.load_error);
}

#[tokio::test]
async fn open_message_resolves_inline_cid_images_to_data_uris() {
    // A `multipart/related` message whose HTML references an inline image by `cid:`, the
    // image carried in a sibling part with a matching Content-ID. `aGVsbG8=` is base64 for
    // `hello`, so the resolved data: URI is easy to assert.
    let related = concat!(
        "Content-Type: multipart/related; boundary=\"b\"\r\n\r\n",
        "--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
        "<p>Logo: <img src=\"cid:logo@allodia\"></p>\r\n",
        "--b\r\nContent-Type: image/png\r\nContent-ID: <logo@allodia>\r\n",
        "Content-Transfer-Encoding: base64\r\nContent-Disposition: inline\r\n\r\naGVsbG8=\r\n",
        "--b--\r\n",
    );
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider =
        FakeProvider::with(vec![message("m1", "a", "With logo")]).with_source(related.as_bytes());
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let reading = app.reading_view();
    let html = reading.html.expect("an html body");
    // The `cid:` reference resolved to a self-contained inline data: image…
    assert!(html.contains("data:image/png;base64,aGVsbG8="), "{html}");
    assert!(!html.contains("cid:"), "no cid reference remains: {html}");
    // …and inline images are local (part of the message), so they never trip the
    // "load remote images" gate.
    assert!(!reading.has_remote_images);
    assert!(!reading.load_error);
}

#[tokio::test]
async fn open_message_lists_and_saves_downloadable_attachments() {
    let raw = concat!(
        "Content-Type: multipart/mixed; boundary=\"m\"\r\n\r\n",
        "--m\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody\r\n",
        "--m\r\nContent-Type: application/pdf; name=\"report.pdf\"\r\n",
        "Content-Disposition: attachment; filename=\"report.pdf\"\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\nUERG\r\n",
        "--m--\r\n",
    );
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider =
        FakeProvider::with(vec![message("m1", "a", "With attachment")]).with_source(raw.as_bytes());
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let reading = app.reading_view();
    assert_eq!(reading.attachments.len(), 1);
    assert_eq!(reading.attachments[0].file_name, "report.pdf");
    assert_eq!(reading.attachments[0].media_type, "application/pdf");
    assert_eq!(reading.attachments[0].size, 3);

    let path = std::env::temp_dir().join(format!(
        "mailcal-attachment-{}-{}.pdf",
        std::process::id(),
        reading.attachments[0].id
    ));
    app.save_attachment(
        msg("acct-1", "m1"),
        reading.attachments[0].id,
        path.to_str().expect("temp path"),
    )
    .await
    .expect("save attachment");
    assert_eq!(std::fs::read(&path).expect("saved bytes"), b"PDF");
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn open_message_surfaces_to_cc_and_bcc_recipients_formatted() {
    // A stored message carrying To/Cc/Bcc; like the sender's own Sent copy, whose APPENDed
    // bytes keep the Bcc header. Opening it surfaces every recipient in the reading snapshot,
    // so the sender can see whom they Bcc'd.
    let mut m = message("m1", "a", "Quarterly report");
    m.envelope.to = vec![
        EmailAddress::named("Bob Jones", "bob@remote.test"),
        EmailAddress::new("team@remote.test"),
    ];
    m.envelope.cc = vec![EmailAddress::new("carol@remote.test")];
    m.envelope.bcc = vec![EmailAddress::new("dave@remote.test")];

    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", FakeProvider::with(vec![m]))],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let reading = app.reading_view();
    // Named addresses render as "Name <email>", bare ones as the address; comma-joined.
    assert_eq!(reading.to, "Bob Jones <bob@remote.test>, team@remote.test");
    assert_eq!(reading.cc, "carol@remote.test");
    assert_eq!(reading.bcc, "dave@remote.test");
}

#[tokio::test]
async fn open_message_flags_a_load_error_when_the_body_cannot_be_fetched() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // A key that isn't in the account's synced set can't resolve a message → load_error.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "does-not-exist"),
    })
    .await;
    let reading = app.reading_view();
    assert_eq!(reading.key, "does-not-exist");
    assert!(reading.load_error);
    assert!(reading.html.is_none() && reading.plain.is_none());
}

#[test]
fn reply_and_forward_subjects_prefix_without_doubling() {
    assert_eq!(
        reply_subject(Some("Quarterly report")),
        "Re: Quarterly report"
    );
    assert_eq!(
        forward_subject(Some("Quarterly report")),
        "Fwd: Quarterly report"
    );
    // Already-prefixed subjects (any case) are not doubled.
    assert_eq!(reply_subject(Some("Re: Lunch")), "Re: Lunch");
    assert_eq!(reply_subject(Some("re: Lunch")), "re: Lunch");
    assert_eq!(forward_subject(Some("FWD: Deck")), "FWD: Deck");
    // A missing/blank subject yields the bare prefix.
    assert_eq!(reply_subject(None), "Re:");
    assert_eq!(reply_subject(Some("   ")), "Re:");
}

#[tokio::test]
async fn mark_read_forwards_a_seen_edit_through_the_loop() {
    use engine_core::mail::{Keyword, SystemKeyword};

    let provider = FakeProvider::new();
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // Sync first so the message is in the store and the edit can resolve its owning account.
    app.dispatch(Intent::RefreshMail).await;
    surfaces.lock().unwrap().clear();

    // Marking a message read forwards a SetKeywords{add: $seen} for that key to the
    // provider, then re-syncs (so the surface fires).
    app.dispatch(Intent::MarkRead {
        message: msg("acct-1", "m1"),
        read: true,
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1);
    match &edits[0] {
        MailEdit::SetKeywords { target, add, .. } => {
            assert_eq!(target.as_str(), "m1");
            assert!(add.contains(&Keyword::system(SystemKeyword::Seen)));
        }
        other => panic!("expected a SetKeywords edit, got {other:?}"),
    }
    // The edit re-synced and republished the list (ignoring background sync-progress pulses).
    assert_eq!(
        surfaces
            .lock()
            .unwrap()
            .iter()
            .filter(|s| **s == Surface::MailboxList)
            .count(),
        1
    );
}

#[tokio::test]
async fn failing_bodies_do_not_block_older_mail_from_warming() {
    // The NEWEST chunk of the window persistently fails to fetch (a real backfill froze this
    // way: ~200 unfetchable messages filled every newest-first batch and the walk gave up).
    // The pass must look past the failures and still warm everything behind them.
    let mut messages = Vec::new();
    for i in 0..250 {
        let mut m = message(&format!("m{i}"), "a", &format!("Subject {i}"));
        // Descending dates: m0 is the newest, so the failing m0..m209 rank first.
        let minute = 59 - (i % 60);
        let hour = 23 - (i / 60);
        m.received_at = Some(
            format!("2026-06-15T{hour:02}:{minute:02}:00Z")
                .parse()
                .unwrap(),
        );
        messages.push(m);
    }
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider =
        FakeProvider::with(messages).with_failing_sources((0..210).map(|i| format!("m{i}")));
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    // m249 is the oldest message, ranked behind all 210 failures; it must still be warm.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m249"),
    })
    .await;
    let reading = app.reading_view();
    assert!(
        !reading.load_error,
        "the warm pass looked past the failing newest chunk",
    );
    assert!(reading.plain.is_some());
}

#[tokio::test]
async fn a_body_conflict_resyncs_the_folder_and_warms_its_renumbered_keys() {
    // A folder synced on demand holds keys under an old UIDVALIDITY; the server renumbers
    // (every stored key is now stale) and nothing else ever re-syncs that folder; on a real
    // account this left 882 bodies failing on every pass, forever. On a fetch **conflict**
    // the warm pass must re-sync that folder and warm the replacement keys.
    let provider = FakeProvider::new() // inbox m1, m2
        .with_failing_sources(["c-old".to_owned()]); // the stale key conflicts on fetch
    let offline = provider.failure_switch();
    let connector = FakeConnector::new(vec![(
        "custom".to_owned(),
        vec![message("c-old", "custom", "Filed report")],
    )]);
    let folders = connector.folders();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_connector(vec![account("acct-1", provider)], connector, &surfaces);

    // Sync the inbox, then open the custom folder (folders are per-account, so select the
    // account first) so its message lands in the store under the old key.
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    app.dispatch(open_folder("acct-1", "custom")).await;

    // The server renumbers the folder: the same message now lives under a fresh key.
    folders.lock().unwrap()[0].1 = vec![message("c-new", "custom", "Filed report")];

    // The next refresh's warm pass hits the stale key's conflict, re-syncs the folder
    // through the connector, and warms the renumbered message.
    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "c-new"),
    })
    .await;
    let reading = app.reading_view();
    assert!(
        !reading.load_error,
        "the conflict re-synced the folder and warmed the renumbered key",
    );
    assert!(reading.plain.is_some());
}

#[tokio::test]
async fn mail_arriving_on_a_later_refresh_is_warmed_by_that_refresh() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::new();
    let offline = provider.failure_switch();
    let late = provider.late_delivery();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // First refresh syncs + warms the initial mailbox.
    app.dispatch(Intent::RefreshMail).await;

    // A new message arrives after that (the IDLE / poll-tick case), delivered by the next
    // cursored sync. That refresh must warm its body too; fresh mail used to land as
    // headers+preview only, unreadable offline until first opened.
    late.lock()
        .unwrap()
        .push(message("m3", "a", "Late arrival"));
    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m3"),
    })
    .await;
    let reading = app.reading_view();
    assert!(
        !reading.load_error,
        "the refresh that delivered m3 also warmed its body",
    );
    assert!(reading.plain.is_some());
}
