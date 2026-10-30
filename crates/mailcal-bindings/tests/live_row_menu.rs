//! Gated live check of a conversation's row menu against the harness's three conversations
//! (`docker/stalwart/seed.sh`, `docs/row-actions.md`), over JMAP and over IMAP: each one is
//! gathered from the Inbox, Sent Items and Drafts into one row, and its menu answers its newest
//! message, whether that was received, sent or is a draft. A draft says it is one on its row,
//! inside a conversation and on a flat row alike, which is what opens it in a composer
//! (`docs/drafts.md`).
//!
//! Skips unless `STALWART_HTTP_ADDR` is set; the IMAP half also needs `MAILCAL_HARNESS_IMAP` and
//! `MAILCAL_EXTRA_CA`. `scripts/dev/harness.sh test` sets all three. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 MAILCAL_HARNESS_IMAP=localhost:12993 \
//!   MAILCAL_EXTRA_CA=docker/stalwart/tls/harness-ca.pem \
//!   cargo test -p mailcal-bindings --test live_row_menu -- --nocapture --test-threads=1
//! ```

// Shared with the sign-in tests, which use the rest of it.
#[allow(dead_code)]
mod live_oauth;

use std::time::{Duration, Instant};

use live_oauth::{RecordingStore, app};
use mailcal_bindings::{
    FolderRole, Intent, JmapSetup, MailcalApp, MessageKind, RowAction, RowMenu, SelectedRow,
    SnapshotRow, ThreadRow, ViewMode,
};

/// Each seeded conversation: its subject, how many messages it holds, and its newest one.
const CONVERSATIONS: [(&str, usize, MessageKind); 3] = [
    ("Venue for the spring meeting", 3, MessageKind::Received),
    ("Minutes of the January meeting", 2, MessageKind::Sent),
    ("Contribution for next year", 4, MessageKind::Draft),
];

/// Waits up to a minute for `check` to answer something, polling.
fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The conversation row whose subject ends with `subject`, once it holds `count` messages.
fn conversation(app: &MailcalApp, subject: &str, count: usize) -> ThreadRow {
    eventually(subject, || {
        app.mailbox_list()
            .rows
            .into_iter()
            .find_map(|row| match row {
                SnapshotRow::Thread { row }
                    if row.subject.ends_with(subject) && row.messages.len() == count =>
                {
                    Some(row)
                }
                _ => None,
            })
    })
}

fn item(menu: &RowMenu, action: RowAction) -> Option<&mailcal_bindings::RowMenuItem> {
    menu.sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| item.action == action)
}

fn every_conversation_answers_its_newest_message(app: &MailcalApp) {
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Threaded,
    });
    app.dispatch(Intent::RefreshMail);
    for (subject, count, newest) in CONVERSATIONS {
        let row = conversation(app, subject, count);
        let menu = app.row_menu(vec![SelectedRow::Thread {
            account: row.account.clone(),
            thread_id: row.thread_id.clone(),
        }]);
        let open = item(&menu, RowAction::Open).expect("Open");
        let target = open.target.as_ref().expect("Open names a message");
        assert_eq!(
            target.key, row.messages[0].key,
            "{subject}: the newest message"
        );
        assert_eq!(target.kind, newest, "{subject}: its kind");
        assert_eq!(
            row.messages[0].draft,
            newest == MessageKind::Draft,
            "{subject}: the row says whether its newest message is a draft",
        );
        let reply = item(&menu, RowAction::Reply).expect("Reply");
        assert_eq!(
            reply.enabled,
            newest != MessageKind::Draft,
            "{subject}: a draft cannot be answered",
        );
        let flag = item(&menu, RowAction::Flag).expect("Flag");
        assert_eq!(
            flag.target.as_ref().map(|m| m.kind),
            Some(MessageKind::Received),
            "{subject}: a flag marks what was received",
        );
        let read = item(&menu, RowAction::MarkRead).or_else(|| item(&menu, RowAction::MarkUnread));
        assert!(
            read.is_some_and(|read| read.enabled),
            "{subject}: read state"
        );
    }
    a_flat_draft_says_it_is_one(app);
}

/// The draft conversation's draft, listed flat in Drafts, says it is a draft, and the received
/// message it answers, listed flat in the Inbox, says it is not.
fn a_flat_draft_says_it_is_one(app: &MailcalApp) {
    let (subject, count, _) = CONVERSATIONS[2];
    let account = conversation(app, subject, count).account;
    let folder = |role: fn(&FolderRole) -> bool| {
        app.mailbox_list()
            .account_folders
            .into_iter()
            .find(|tree| tree.account_id == account)
            .and_then(|tree| {
                tree.folders
                    .into_iter()
                    .find(|f| f.role.as_ref().is_some_and(role))
            })
            .expect("the harness account has the folder")
            .key
    };
    let drafts = folder(|role| matches!(role, FolderRole::Drafts));
    let inbox = folder(|role| matches!(role, FolderRole::Inbox));
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Flat,
    });
    for (key, draft) in [(drafts, true), (inbox, false)] {
        app.dispatch(Intent::SelectFolder {
            account: account.clone(),
            key: key.clone(),
        });
        // The folder asked for, not the one listed before it.
        let row = eventually("a flat row of the conversation", || {
            let list = app.mailbox_list();
            if list.selected.as_ref() != Some(&key) {
                return None;
            }
            list.rows.into_iter().find_map(|row| match row {
                SnapshotRow::Flat { row } if row.subject.ends_with(subject) => Some(row),
                _ => None,
            })
        });
        assert_eq!(row.draft, draft, "{subject}: {}", row.key);
    }
}

#[test]
fn a_jmap_conversation_menu_answers_its_newest_message() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping row-menu live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let app = app("row-menu-jmap", RecordingStore::default());
    let config = mailcal_bindings::jmap_account_config_toml(JmapSetup {
        email: "alice@test.local".to_owned(),
        server_url: Some(format!("http://{http}")),
        password: "harness-alice-pw".to_owned(),
    })
    .expect("a JMAP config");
    app.add_account(config).expect("alice connects");
    every_conversation_answers_its_newest_message(&app);
}

#[test]
fn an_imap_conversation_menu_answers_its_newest_message() {
    let (Ok(_), Ok(imap)) = (
        std::env::var("MAILCAL_EXTRA_CA"),
        std::env::var("MAILCAL_HARNESS_IMAP"),
    ) else {
        eprintln!("skipping row-menu IMAP live test: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let (host, port) = imap.rsplit_once(':').expect("host:port");
    let app = app("row-menu-imap", RecordingStore::default());
    let config = format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"{host}\"\n\
         username = \"alice@test.local\"\npassword = \"harness-alice-pw\"\n\
         [smtp]\naddr = \"127.0.0.1:12587\"\nserver_name = \"{host}\"\nsecurity = \"starttls\"\n"
    );
    app.add_account(config).expect("alice connects");
    every_conversation_answers_its_newest_message(&app);
}
