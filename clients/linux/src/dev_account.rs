//! Debug-only account fixtures for the local seeded Stalwart harness.

#![cfg(debug_assertions)]

use std::sync::Arc;

use mailcal_bindings::{JmapSetup, MailcalApp, MailcalError, jmap_account_config_toml};

/// The canned IMAP account. It dials by IP while validating the certificate for `localhost`;
/// the shared setup builder deliberately has no server-name override, so this fixture is
/// hand-written like its Apple, Android, and Windows counterparts.
pub(crate) const STALWART_IMAP_TOML: &str = r#"
[imap]
addr = "127.0.0.1:12993"
server_name = "localhost"
username = "alice@test.local"
password = "harness-alice-pw"
"#;

/// Builds the JMAP harness config through the production config builder so its schema cannot drift.
pub(crate) fn stalwart_jmap_toml() -> Result<String, MailcalError> {
    jmap_toml_for("alice@test.local", "harness-alice-pw")
}

/// The harness's second mailbox, connected beside the first by `stalwart-multi`.
///
/// It exists for contacts: the engine merges people across accounts on a shared address, and the
/// seeded `shared-*` card is filed in alice's book **and** bob's, so only a two-account boot
/// renders it as the one row marked "In 2 accounts" (`docs/contacts.md`).
pub(crate) fn stalwart_jmap_toml_second() -> Result<String, MailcalError> {
    jmap_toml_for("bob@test.local", "harness-bob-pw")
}

fn jmap_toml_for(email: &str, password: &str) -> Result<String, MailcalError> {
    jmap_account_config_toml(JmapSetup {
        email: email.to_owned(),
        server_url: Some("http://127.0.0.1:28080".to_owned()),
        password: password.to_owned(),
    })
}

/// Gives each harness account the name the harness server already holds for it, which is what the
/// step after a connect would have asked for (`docs/sending.md`).
///
/// The canned account is injected as a stored config and never goes through the setup form, so
/// nothing ever asks, and every harness run would otherwise send as a bare address and show an
/// empty field on its Settings card. This is the step's two calls without the dialog.
///
/// Only where the name is still empty, which `suggested_sender_name` already decides: it answers
/// the stored name when there is one, so a name typed during a dev session survives the next
/// launch rather than being overwritten by the server's.
///
/// Called only from the harness boot arms, never from the real-account one: pulling a provider's
/// copy into an account nobody asked about is what `docs/sending.md` keeps out of the product, and
/// a dev convenience may not smuggle it in.
///
/// On a worker, because `suggested_sender_name` is a provider round trip and `boot::app` runs on
/// the GLib main loop: the rule `setup_widgets::sender_name_suggestion` states holds for this
/// caller too. Nothing waits for it. The name reaches the card and the From through the settings
/// signal the setter raises, exactly as it would had someone typed it.
pub(crate) fn seed_sender_names(app: &Arc<MailcalApp>) {
    let app = Arc::clone(app);
    std::thread::spawn(move || {
        for account in app.sync_settings().accounts {
            if !account.sender_name.is_empty() {
                continue;
            }
            let suggestion = app.suggested_sender_name(account.account_id.clone());
            if suggestion.is_empty() {
                continue;
            }
            app.set_account_sender_name(account.account_id, suggestion);
            log::info!("harness: seeded a sender name from the provider");
        }
    });
}
