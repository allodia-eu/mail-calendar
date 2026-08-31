//! Debug-only account fixtures for the local seeded Stalwart harness.

#![cfg(debug_assertions)]

use mailcal_bindings::{JmapSetup, MailcalError, jmap_account_config_toml};

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
