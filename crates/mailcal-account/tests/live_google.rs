//! Gated live check that the **product** Gmail path can write, end to end against a real
//! Google account: build a [`GoogleConfig`] from a stored refresh token, connect the
//! account's mail provider, and drive `submit_email` + `edit_mail` **through the
//! token-refreshing wrapper** the app actually uses.
//!
//! This exists because the engine's own live tests cannot catch the bug it pins. They
//! drive `GmailProvider` **directly**, which has always implemented `edit_mail` and
//! `submit_email`; while the product wrapper around it
//! (`mailcal_account::google::RefreshingGmailProvider`) advertised mail read/sync only and
//! forwarded neither. So every archive, delete, mark-read and send on a Google account
//! failed with `InvalidState: provider does not support mail writes`, with a fully green
//! engine suite. A capability the wrapper does not forward is invisible to every test that
//! skips the wrapper.
//!
//! It **skips** unless `GOOGLE_REFRESH_TOKEN`, `GOOGLE_CLIENT_ID` and `GOOGLE_TEST_ADDRESS`
//! are set, so the offline `cargo test` stays green; there is no CI harness (no live Google
//! account in CI).
//!
//! Run locally: the token file `tools/google-oauth` writes in the engine checkout has
//! everything, so read it straight out of there:
//! ```sh
//! T=<engine checkout>/tools/google-oauth/.local/tokens.json
//! GOOGLE_TEST_ADDRESS=<the test account's own address> \
//! GOOGLE_CLIENT_ID=$(python3 -c "import json;print(json.load(open('$T'))['client_id'])") \
//! GOOGLE_CLIENT_SECRET=$(python3 -c "import json;print(json.load(open('$T'))['client_secret'])") \
//! GOOGLE_REFRESH_TOKEN=$(python3 -c "import json;print(json.load(open('$T'))['refresh_token'])") \
//!   cargo test -p mailcal-account --test live_google -- --nocapture
//! ```

use engine_core::{
    ids::{MailboxId, MessageIdHeader},
    mail::EmailAddress,
    sync::SyncUpdate,
};
use engine_provider::{Draft, MailEdit, Provider};
use mailcal_account::{GoogleConfig, Secret, connect_google_mail_providers, google_token_source};

/// The test account's own address, from `GOOGLE_TEST_ADDRESS`. Every live send is
/// self-addressed, so nothing leaves the mailbox.
fn self_address() -> String {
    std::env::var("GOOGLE_TEST_ADDRESS").unwrap_or_default()
}

/// A Google config built from the environment, or `None` to skip the gated test.
fn config() -> Option<GoogleConfig> {
    let refresh_token = std::env::var("GOOGLE_REFRESH_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())?;
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .ok()
        .filter(|t| !t.is_empty())?;
    let email = self_address();
    if email.is_empty() {
        return None;
    }
    Some(GoogleConfig {
        email,
        client_id,
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty()),
        // The loopback the desktop capture tool registers; unused on a refresh grant.
        redirect_uri: "http://127.0.0.1:8400".to_owned(),
        scopes: vec!["https://mail.google.com/".to_owned()],
        refresh_token: Secret::new(refresh_token),
    })
}

/// A self-addressed draft with a caller-generated Message-ID.
fn live_draft(marker: &str) -> Draft {
    let message_id = MessageIdHeader::new(format!("core-live-{marker}@example.test")).unwrap();
    Draft::new(
        message_id,
        EmailAddress::new(self_address()),
        vec![EmailAddress::new(self_address())],
        format!("Core live {marker}"),
        "Live core-wrapper write test.",
    )
}

#[tokio::test]
async fn the_product_gmail_wrapper_advertises_and_forwards_writes_and_sends() {
    let Some(config) = config() else {
        eprintln!("skipping live_google: GOOGLE_REFRESH_TOKEN/GOOGLE_CLIENT_ID unset");
        return;
    };
    let account = config.account_id().expect("account id");
    let tokens = google_token_source(
        &config,
        account.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("token source");
    let providers = connect_google_mail_providers(tokens, None)
        .await
        .expect("connect gmail");
    let provider = providers.first().expect("one account-global mail provider");

    // The wrapper must *advertise* what it forwards: the app never attempts a capability
    // the provider does not claim, and a claim it does not forward hits the trait's
    // rejecting default. Both halves were wrong before this fix.
    let caps = provider.connection_info().capabilities;
    assert!(caps.mail(), "mail read/sync");
    assert!(caps.mail_writes(), "mail writes are advertised");
    assert!(caps.submission(), "submission is advertised");

    // Send through the wrapper: this is the path the composer's send takes.
    let marker = format!("p{}", std::process::id());
    let receipt = provider
        .submit_email(&account, &live_draft(&marker))
        .await
        .expect("submit_email forwards through the wrapper");
    let key = receipt.email_key;

    // Mark read through the wrapper (the edit that fires on opening a message: the one
    // that was failing several times a minute in the production log).
    provider
        .edit_mail(&account, &MailEdit::mark_seen(key.clone(), true))
        .await
        .expect("edit_mail forwards through the wrapper");

    // Archive through the wrapper, to the id the core resolves for an account with no
    // Archive folder. This is the exact pair that was broken: the core picks `ALL_MAIL`
    // (its `\All` fallback) and the engine turns it into a removal-only modify.
    provider
        .edit_mail(
            &account,
            &MailEdit::move_to(key.clone(), MailboxId::try_from("ALL_MAIL").unwrap()),
        )
        .await
        .expect("archive forwards and is accepted");

    // Read it back: the archive really happened, through the whole product stack.
    let snapshot = provider.sync_email(&account, None).await.expect("snapshot");
    let SyncUpdate::Snapshot { objects, .. } = &snapshot.update else {
        panic!("expected a snapshot");
    };
    let archived = objects
        .iter()
        .find(|message| message.id.key() == &key)
        .expect("the archived message is in the account");
    let labels: Vec<&str> = archived.mailboxes.iter().map(MailboxId::as_str).collect();
    assert!(!labels.contains(&"INBOX"), "left the inbox, got {labels:?}");
    assert!(
        !labels.contains(&"UNREAD"),
        "stayed marked read: {labels:?}"
    );

    // Clean up the throwaway (a permanent delete, also through the wrapper).
    provider
        .edit_mail(&account, &MailEdit::delete(key))
        .await
        .expect("delete forwards through the wrapper");
}
