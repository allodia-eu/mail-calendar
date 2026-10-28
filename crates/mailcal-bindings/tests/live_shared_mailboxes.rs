//! Gated live check of shared mailboxes against the harness, where alice belongs to the
//! `support` group mailbox (`docker/stalwart/README.md`), over JMAP and over IMAP: the server
//! lists it, choosing it lists it as a mail account of its own that syncs the group's mail and
//! sends as its address, the choice is stored with alice's account, and switching it off takes
//! it out again (`docs/shared-mailboxes.md`).
//!
//! Skips unless `STALWART_HTTP_ADDR` is set; the IMAP half also needs `MAILCAL_HARNESS_IMAP` and
//! `MAILCAL_EXTRA_CA`. `scripts/dev/harness.sh test` sets all three. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 MAILCAL_HARNESS_IMAP=localhost:12993 \
//!   MAILCAL_EXTRA_CA=docker/stalwart/tls/harness-ca.pem \
//!   cargo test -p mailcal-bindings --test live_shared_mailboxes -- --nocapture --test-threads=1
//! ```

// Shared with the sign-in tests, which use the rest of it.
#[allow(dead_code)]
mod live_oauth;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use live_oauth::{RecordingStore, app};
use mailcal_bindings::{
    FolderRole, Intent, JmapSetup, MailcalApp, Recipients, SharedMailboxEntry,
    SharedMailboxSupport, SnapshotRow,
};

const SUPPORT: &str = "support@test.local";
/// The subject of the one message the harness seeds into the group's Inbox.
const SUPPORT_SUBJECT: &str = "Order 4711 arrived damaged";

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

fn subjects(app: &MailcalApp) -> Vec<String> {
    app.mailbox_list()
        .rows
        .into_iter()
        .map(|row| match row {
            SnapshotRow::Flat { row } => row.subject,
            SnapshotRow::Thread { row } => row.subject,
        })
        .collect()
}

/// Sends a message from the shared mailbox `child` and waits for the copy in its own Sent.
fn a_message_sent_from_it_is_filed_in_its_own_sent(app: &MailcalApp, child: &str) {
    let subject = format!(
        "Sent as the group {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    app.submit_composition(
        format!("shared-send-{subject}"),
        Recipients {
            to: "dave@test.local".to_owned(),
            cc: String::new(),
            bcc: String::new(),
        },
        subject.clone(),
        r#"{"blocks":[{"Paragraph":{"content":[{"Text":{"text":"Hello from the group",
            "bold":false,"italic":false,"underline":false,"link":null}}]}}]}"#
            .to_owned(),
        Vec::new(),
        Some(child.to_owned()),
    )
    .expect("the send is accepted");
    let sent = eventually("the shared mailbox's Sent folder", || {
        app.mailbox_list()
            .account_folders
            .into_iter()
            .find(|folders| folders.account_id == child)?
            .folders
            .into_iter()
            .find(|folder| matches!(folder.role, Some(FolderRole::Sent)))
    });
    app.dispatch(Intent::SelectFolder {
        account: child.to_owned(),
        key: sent.key,
    });
    let from = eventually("the sent copy in the shared mailbox's Sent", || {
        app.mailbox_list()
            .rows
            .into_iter()
            .find_map(|row| match row {
                SnapshotRow::Flat { row } if row.subject == subject => Some(row.from),
                SnapshotRow::Thread { row } if row.subject == subject => Some(row.latest_from),
                _ => None,
            })
    });
    assert!(
        from == SUPPORT || from == "Support Team",
        "sent as the group, not as alice: {from}"
    );
}

/// Chooses the support mailbox for `parent`, checks what that does, and switches it off again.
fn the_support_mailbox_is_listed_chosen_synced_stored_and_dropped(
    app: &Arc<MailcalApp>,
    store: &RecordingStore,
    parent: &str,
    with_name: bool,
) {
    let view = eventually(
        "the account to connect and list its shared mailboxes",
        || {
            let view = app
                .shared_mailboxes(parent.to_owned())
                .inspect_err(|err| eprintln!("not yet: {err}"))
                .ok()?;
            (view.support != SharedMailboxSupport::NotConnected).then_some(view)
        },
    );
    assert_eq!(view.support, SharedMailboxSupport::Listed);
    let support: SharedMailboxEntry = view
        .mailboxes
        .iter()
        .find(|mailbox| mailbox.address.as_deref() == Some(SUPPORT))
        .cloned()
        .unwrap_or_else(|| panic!("the group mailbox is listed: {:?}", view.mailboxes));
    assert!(!support.selected && support.listed);
    assert!(
        view.mailboxes
            .iter()
            .all(|mailbox| mailbox.address.as_deref() != Some("alice@test.local")),
        "the account's own mailbox is not offered as a shared one"
    );

    app.set_shared_mailbox(parent.to_owned(), support.clone(), true)
        .expect("choosing it");
    let stored = store.0.lock().unwrap().last().cloned().unwrap();
    assert!(stored.contains("[[shared_mailbox]]"), "{stored}");
    let view = app.shared_mailboxes(parent.to_owned()).unwrap();
    assert!(view.mailboxes[0].selected, "the chosen one comes first");

    // A mail account of its own, under its own address, so it is in the folder pane and in
    // From.
    let child = eventually("the shared mailbox to be listed", || {
        app.mailbox_list()
            .accounts
            .into_iter()
            .find(|row| row.email == SUPPORT)
    });
    assert_ne!(child.id, parent);
    if with_name {
        // The server's own name for the group, taken when it connects, as an added account
        // takes its own (the harness names the group "Support Team").
        eventually("the group's name to be taken from its server", || {
            app.mailbox_list()
                .accounts
                .into_iter()
                .find(|row| row.id == child.id && row.name == "Support Team")
        });
    }
    app.dispatch(Intent::SelectAccount {
        account: Some(child.id.clone()),
    });
    eventually(
        "the group's message to sync into the shared mailbox",
        || {
            subjects(app)
                .iter()
                .any(|subject| subject == SUPPORT_SUBJECT)
                .then_some(())
        },
    );
    let inbox = eventually("the shared mailbox's folders", || {
        app.mailbox_list()
            .account_folders
            .into_iter()
            .find(|folders| folders.account_id == child.id && !folders.folders.is_empty())
    });
    assert!(
        inbox
            .folders
            .iter()
            .all(|folder| !folder.name.contains("Shared Folders")),
        "its folders read like any account's: {:?}",
        inbox
            .folders
            .iter()
            .map(|folder| &folder.name)
            .collect::<Vec<_>>()
    );

    a_message_sent_from_it_is_filed_in_its_own_sent(app, &child.id);

    // Its mail stays its own: alice's Inbox does not have the group's message.
    app.dispatch(Intent::SelectAccount {
        account: Some(parent.to_owned()),
    });
    eventually("alice's own mail", || {
        let list = app.mailbox_list();
        let subjects = subjects(app);
        (list.selected_account.as_deref() == Some(parent) && !subjects.is_empty())
            .then_some(subjects)
    })
    .iter()
    .for_each(|subject| assert_ne!(subject, SUPPORT_SUBJECT));

    app.set_shared_mailbox(parent.to_owned(), support, false)
        .expect("switching it off");
    assert!(
        app.mailbox_list()
            .accounts
            .iter()
            .all(|row| row.id != child.id),
        "switched off, it leaves the mail surfaces"
    );
    let stored = store.0.lock().unwrap().last().cloned().unwrap();
    assert!(!stored.contains("[[shared_mailbox]]"), "{stored}");
}

#[test]
fn a_jmap_account_opens_the_group_mailbox_shared_with_it() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping shared-mailbox live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("shared-jmap", store.clone());
    let config = mailcal_bindings::jmap_account_config_toml(JmapSetup {
        email: "alice@test.local".to_owned(),
        server_url: Some(format!("http://{http}")),
        password: "harness-alice-pw".to_owned(),
    })
    .expect("a JMAP config");
    let parent = app.add_account(config).expect("alice connects").id;
    the_support_mailbox_is_listed_chosen_synced_stored_and_dropped(&app, &store, &parent, true);
}

#[test]
fn an_imap_account_opens_the_group_mailbox_shared_with_it() {
    let (Ok(_), Ok(imap)) = (
        std::env::var("MAILCAL_EXTRA_CA"),
        std::env::var("MAILCAL_HARNESS_IMAP"),
    ) else {
        eprintln!("skipping shared-mailbox IMAP live test: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let (host, port) = imap.rsplit_once(':').expect("host:port");
    let store = RecordingStore::default();
    let app = app("shared-imap", store.clone());
    let config = format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"{host}\"\n\
         username = \"alice@test.local\"\npassword = \"harness-alice-pw\"\n\
         [smtp]\naddr = \"127.0.0.1:12587\"\nserver_name = \"{host}\"\nsecurity = \"starttls\"\n"
    );
    let parent = app.add_account(config).expect("alice connects").id;
    the_support_mailbox_is_listed_chosen_synced_stored_and_dropped(&app, &store, &parent, false);
    a_shared_folder_without_an_inbox_still_opens(&app, &parent);
}

/// Dave shares his `Board` folder and nothing else, so the store alice reaches has no Inbox: it
/// still opens, with that folder in it.
fn a_shared_folder_without_an_inbox_still_opens(app: &Arc<MailcalApp>, parent: &str) {
    let board = app
        .shared_mailboxes(parent.to_owned())
        .unwrap()
        .mailboxes
        .into_iter()
        .find(|mailbox| mailbox.handle.ends_with("dave@test.local"))
        .expect("dave's store is listed");
    app.set_shared_mailbox(parent.to_owned(), board.clone(), true)
        .expect("choosing it");
    let child = eventually("dave's store to be listed", || {
        app.mailbox_list()
            .accounts
            .into_iter()
            .find(|row| row.email == "dave@test.local")
    });
    eventually("dave's Board folder to open", || {
        app.mailbox_list()
            .account_folders
            .into_iter()
            .find(|folders| folders.account_id == child.id)?
            .folders
            .into_iter()
            .find(|folder| folder.name == "Board")
    });
    app.set_shared_mailbox(parent.to_owned(), board, false)
        .expect("switching it off");
}
