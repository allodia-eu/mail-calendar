//! Gated live check of an account's servers on its page, against the harness: a save the servers
//! refuse keeps the edit as it was typed, and servers saved by any other path (accepting a server
//! change does it) are what the form is drawn from after that, never the edit refused before.
//!
//! Skips unless `MAILCAL_HARNESS_IMAP` and `MAILCAL_EXTRA_CA` are set; `scripts/dev/harness.sh
//! test` sets both. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! MAILCAL_HARNESS_IMAP=localhost:12993 MAILCAL_EXTRA_CA=docker/stalwart/tls/harness-ca.pem \
//!   cargo test -p mailcal-bindings --test live -- live_account_servers:: --nocapture
//! ```

// Shared with the sign-in tests, which use the rest of it.
#[allow(dead_code)]
use std::time::{Duration, Instant};

use mailcal_bindings::{AccountChangeNotice, AccountPage, AccountPageIntent, Intent, MailcalApp};

use crate::live_oauth::{RecordingStore, app};

/// The page once the save on it has ended.
fn settled(app: &MailcalApp, id: &str) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let page = app.account_page(id.to_owned()).expect("listed");
        let servers = page.servers.as_ref().expect("a password account's servers");
        if !servers.save.running {
            return page;
        }
        assert!(Instant::now() < deadline, "the save never ended");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn servers_saved_elsewhere_replace_the_edit_a_save_refused() {
    let (Ok(_), Ok(imap)) = (
        std::env::var("MAILCAL_EXTRA_CA"),
        std::env::var("MAILCAL_HARNESS_IMAP"),
    ) else {
        eprintln!("skipping the servers live test: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let (host, port) = imap.rsplit_once(':').expect("host:port");
    let app = app("account-servers", RecordingStore::default());
    let config = format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"{host}\"\n\
         username = \"alice@test.local\"\npassword = \"harness-alice-pw\"\n\
         [smtp]\naddr = \"127.0.0.1:12587\"\nserver_name = \"{host}\"\nsecurity = \"starttls\"\n"
    );
    let id = app.add_account(config).expect("alice connects").id;
    let stored = settled(&app, &id).servers.expect("editable").form;

    let mut wrong = stored.clone();
    wrong.password = Some("not-her-password".to_owned());
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SaveServers {
            account: id.clone(),
            endpoints: wrong,
        },
    });
    // Polled until the page says it, because the save starts on a thread of its own.
    let deadline = Instant::now() + Duration::from_secs(60);
    let page = loop {
        let page = settled(&app, &id);
        if page.notice.is_some() {
            break page;
        }
        assert!(Instant::now() < deadline, "the refusal was never said");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(matches!(
        page.notice.map(|raised| raised.notice),
        Some(AccountChangeNotice::ServersFailed { .. })
    ));
    assert_eq!(
        page.servers.expect("editable").form.password.as_deref(),
        Some("not-her-password"),
        "the refused edit is drawn as it was typed"
    );

    let mut right = stored;
    right.password = Some("harness-alice-pw".to_owned());
    app.update_account_endpoints(id.clone(), right)
        .expect("the servers take her own password");
    let form = settled(&app, &id).servers.expect("editable").form;
    assert_eq!(
        form.password, None,
        "the stored servers are drawn, not the edit refused before they were saved"
    );
}
