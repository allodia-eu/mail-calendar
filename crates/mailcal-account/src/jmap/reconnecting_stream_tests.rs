//! A JMAP account's streamed reads after a dead connection: a widening is not walked again, and a
//! batch read whose provider cannot be connected answers each message with that failure.

use engine_core::{
    ids::{AccountId, MailboxId, MessageId},
    mail::Message,
    membership::Memberships,
    sync::{MailboxWindows, SyncState, SyncWindow, Widening},
    time::CalendarDate,
};
use engine_provider::MailboxEdit;
use futures::StreamExt;

use crate::{
    graph::test_support::{mock_token_endpoint, source_at},
    http_fake,
    jmap::{JmapAccountConfig, fake_server::FakeJmap},
    load_jmap_str,
};

fn any_account() -> AccountId {
    AccountId::try_from("you@example.net").unwrap()
}

/// A folder change the server never answers: made once, it drops the provider, so the next
/// request connects again.
async fn lose_the_connection(provider: &dyn engine_provider::Provider, server: &FakeJmap) {
    let folder = MailboxEdit::Create {
        name: "Receipts".to_owned(),
        parent: None,
    };
    server.hang_up_next_call();
    provider
        .edit_mailbox(&any_account(), &folder)
        .await
        .unwrap_err();
}

/// A widening that met a dead connection ends there and drops the provider; the next pass widens
/// from the state it kept.
#[tokio::test]
async fn a_widening_on_a_dead_connection_is_not_walked_again() {
    let server = FakeJmap::start();
    let account = load_jmap_str(&format!(
        "[jmap]\nemail = \"you@example.net\"\nbase_url = \"{}\"\npassword = \"hunter2\"\n",
        server.url()
    ))
    .unwrap();
    let providers = crate::connect_jmap_mail_providers(&account, None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    let since = SyncWindow::since(CalendarDate::new(2026, 1, 1).unwrap());
    let widening = Widening::new(
        MailboxWindows::new(since),
        MailboxWindows::new(SyncWindow::full()),
    )
    .expect("all mail is wider than this year's");
    let cursor = SyncState::new("state-1");

    server.hang_up_next_call();
    let walked: Vec<_> = provider
        .widen_email(&any_account(), &cursor, &widening, 50, 50)
        .collect()
        .await;

    assert!(matches!(walked.as_slice(), [Err(_)]), "{walked:?}");
    assert_eq!(server.calls(), 1, "not walked again");
    provider.sync_mailboxes(&any_account(), None).await.unwrap();
    assert_eq!(server.discoveries(), 2, "the provider was dropped");
}

/// An OAuth account at `base_url`.
fn oauth_account(base_url: &str) -> JmapAccountConfig {
    load_jmap_str(&format!(
        r#"
[jmap]
email = "you@example.net"
base_url = "{base_url}"

[jmap.oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://jmap-oauth"
scopes = ["offline_access"]
"#
    ))
    .unwrap()
}

/// The provider cannot be connected again because the server refuses the token and no other
/// can be had: each message of a batch read is answered with that failure, still a credential
/// that could not be had.
#[tokio::test]
async fn a_batch_read_without_a_provider_keeps_every_fact_for_each_message() {
    let server = FakeJmap::start();
    let (endpoint, _) = mock_token_endpoint(vec![http_fake::minted(1)]);
    let tokens = source_at(endpoint, None);
    let providers =
        crate::connect_jmap_mail_providers(&oauth_account(server.url()), Some(&tokens), None)
            .await
            .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    lose_the_connection(provider.as_ref(), &server).await;
    server.revoke("Bearer AT-1");
    let inbox = MailboxId::try_from("inbox").unwrap();
    let messages: Vec<_> = ["m1", "m2"]
        .into_iter()
        .map(|id| {
            Message::new(
                MessageId::try_from(id).unwrap(),
                Memberships::of_one(inbox.clone()),
            )
        })
        .collect();

    let answers: Vec<_> = provider
        .fetch_message_sources(&any_account(), &messages)
        .collect()
        .await;

    assert_eq!(answers.len(), 2);
    for (index, answer) in answers {
        let failed = answer.unwrap_err();
        assert!(failed.is_credential_failure(), "{index}: {failed}");
    }
}
