//! What a Graph folder's provider sends: the token as the account's source answers it on each
//! request, a replacement for one Graph refused, the shared mailbox's own path, and a new client
//! after a dead connection.

use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicUsize, Ordering},
};

use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId, MessageIdHeader, SharedAccountId},
    mail::{EmailAddress, MessageMetadata},
    sync::SyncWindow,
};
use engine_provider::{
    Credential, CredentialRequest, CredentialSource, CredentialSubject, Draft, HandOver,
    MailboxEdit, MailboxWrites, Provider, ProviderError, Unrecorded,
};
use futures::StreamExt;
use provider_graph::{GraphClient, MailboxPrincipal};

use super::super::{
    GraphTokenSource, ReconnectingGraphProvider, client_connecting, principal,
    token_source::test_support::{mock_token_endpoint, refused_token_endpoint, source_at},
};
use crate::{
    http_fake::{self, Reply},
    tls::tls_with,
};

const ME: &str = r#"{"displayName":"You","mail":"you@example.net"}"#;

/// A source over a token endpoint answering `tokens` in turn, and how often it was asked.
fn tokens(tokens: Vec<String>) -> (Arc<GraphTokenSource>, Arc<AtomicUsize>) {
    let (endpoint, hits) = mock_token_endpoint(tokens);
    (source_at(endpoint, None), hits)
}

fn account() -> AccountId {
    AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap()
}

/// The inbox of `principal`'s mailbox, its requests sent to the server at `base`.
fn provider_at(
    base: String,
    tokens: &Arc<GraphTokenSource>,
    principal: MailboxPrincipal,
) -> ReconnectingGraphProvider {
    let client = client_connecting(
        Arc::clone(tokens),
        principal,
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GraphClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );
    ReconnectingGraphProvider::new(MailboxId::try_from("inbox").unwrap(), client, None).unwrap()
}

/// The token goes on each request as the source answers it then, and one Graph refused is
/// replaced once and the request sent again, on the same client.
#[tokio::test]
async fn each_request_presents_the_current_token_and_a_refused_one_is_replaced() {
    let (tokens, minted) = tokens(vec![http_fake::minted(1), http_fake::minted(2)]);
    let (base, received) = http_fake::serve(vec![
        Reply::RefusedBearer,
        Reply::Json(ME.to_owned()),
        Reply::Json(ME.to_owned()),
    ]);
    let provider = provider_at(base, &tokens, MailboxPrincipal::Me);

    provider.sender_identities(&account()).await.unwrap();
    provider.sender_identities(&account()).await.unwrap();

    assert_eq!(
        http_fake::presented(&received),
        [
            http_fake::bearer(1),
            http_fake::bearer(2),
            http_fake::bearer(2)
        ]
    );
    assert_eq!(minted.load(Ordering::SeqCst), 2);
    assert_eq!(provider.delegate.generation(), 0, "the client was kept");
}

/// A mailbox shared with the account is one the signed-in user reaches at `/users/{address}`,
/// with the signed-in user's own token.
#[tokio::test]
async fn a_shared_mailbox_is_reached_at_its_own_path() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![Reply::Json(ME.to_owned())]);
    let shared = SharedAccountId::try_from("support@example.org").unwrap();
    let provider = provider_at(base, &tokens, principal(Some(&shared)).unwrap());

    provider.sender_identities(&account()).await.unwrap();

    let seen = received.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].path.starts_with("/users/support@example.org?"),
        "{}",
        seen[0].path
    );
    assert_eq!(seen[0].authorization, "Bearer AT-1");
}

#[tokio::test]
async fn a_dead_connection_replaces_the_client_and_the_read_is_made_again() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![Reply::HangUp, Reply::Json(ME.to_owned())]);
    let provider = provider_at(base, &tokens, MailboxPrincipal::Me);

    provider.sender_identities(&account()).await.unwrap();

    assert_eq!(received.lock().unwrap().len(), 2);
    assert_eq!(provider.delegate.generation(), 1, "a new client");
}

/// Checking a shared mailbox's address is a read like any other: a dead connection replaces the
/// client and the check is made again.
#[tokio::test]
async fn a_shared_mailbox_check_on_a_dead_connection_is_made_again_on_a_new_client() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![
        Reply::HangUp,
        Reply::Status("404 Not Found", String::new()),
    ]);
    let provider = provider_at(base, &tokens, MailboxPrincipal::Me);

    let _ = provider.resolve_shared_account("support@example.org").await;

    assert_eq!(received.lock().unwrap().len(), 2, "made again");
    assert_eq!(provider.delegate.generation(), 1, "a new client");
}

/// A token endpoint that cannot be reached fails the request before anything is sent. A new
/// connection cannot help that, so the client is kept, for the read and for a streamed pass.
#[tokio::test]
async fn a_token_that_cannot_be_renewed_keeps_the_client() {
    let source = source_at(refused_token_endpoint(), None);
    let (base, received) = http_fake::serve(vec![]);
    let provider = provider_at(base, &source, MailboxPrincipal::Me);

    let refused = provider.sender_identities(&account()).await.unwrap_err();
    let streamed = provider
        .stream_email(&account(), None, SyncWindow::full().into(), 200, 50)
        .collect::<Vec<_>>()
        .await;

    assert_eq!(refused.class(), FailureClass::Retryable, "{refused}");
    assert!(streamed.iter().all(Result::is_err), "{streamed:?}");
    assert!(received.lock().unwrap().is_empty(), "nothing was sent");
    assert_eq!(provider.delegate.generation(), 0, "the client was kept");
}

/// A credential source that never answers, and how often it was asked.
#[derive(Debug, Default)]
struct Silent {
    asks: AtomicUsize,
}

#[async_trait::async_trait]
impl CredentialSource for Silent {
    fn subject(&self) -> &CredentialSubject {
        static SUBJECT: LazyLock<CredentialSubject> =
            LazyLock::new(|| CredentialSubject::new("silent@example.com"));
        &SUBJECT
    }

    async fn credential(&self, _: &CredentialRequest<'_>) -> Result<Credential, ProviderError> {
        self.asks.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

/// A source that does not answer within the engine's deadline fails the request before anything
/// is sent, as one that cannot reach its token endpoint does: the client is kept and the read is
/// not made again, which would only wait out the deadline a second time.
#[tokio::test(start_paused = true)]
async fn a_source_that_does_not_answer_keeps_the_client() {
    let (tokens, _) = tokens(vec![]);
    let silent = Arc::new(Silent::default());
    let (base, received) = http_fake::serve(vec![]);
    let asked = Arc::clone(&silent);
    let client = client_connecting(
        tokens,
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
        move |_, tls, retry, agent| {
            GraphClient::with_base_and_source(Arc::clone(&asked) as _, &base, tls, retry, agent)
        },
    );
    let provider =
        ReconnectingGraphProvider::new(MailboxId::try_from("inbox").unwrap(), client, None)
            .unwrap();

    let failed = provider.sender_identities(&account()).await.unwrap_err();

    assert!(failed.is_credential_failure(), "{failed}");
    assert_eq!(silent.asks.load(Ordering::SeqCst), 1, "asked once");
    assert!(received.lock().unwrap().is_empty(), "nothing was sent");
    assert_eq!(provider.delegate.generation(), 0, "the client was kept");
}

/// A send, a saved draft and a new folder may each have been applied before the connection
/// died, so each is sent once and never again by the wrapper; the dead client is still replaced
/// for the next request.
#[tokio::test]
async fn a_write_on_a_dead_connection_is_sent_once_and_the_client_replaced() {
    let draft = Draft::new(
        MessageIdHeader::new("draft@example.com").unwrap(),
        EmailAddress::new("you@example.net"),
        vec![EmailAddress::new("them@example.org")],
        "Subject",
        // Larger than a socket buffer, so the request is still being written when a server that
        // stopped reading early would drop it.
        "Body ".repeat(100_000),
    );
    let folder = MailboxEdit::Create {
        name: "Receipts".to_owned(),
        parent: None,
    };
    for write in ["send", "draft", "folder"] {
        let (tokens, _) = tokens(vec![http_fake::minted(1)]);
        let (base, received) = http_fake::serve(vec![Reply::HangUp, Reply::Json(ME.to_owned())]);
        let provider = provider_at(base, &tokens, MailboxPrincipal::Me);

        let failed = match write {
            "send" => provider
                .submit_email(&account(), &draft, &HandOver::new(&Unrecorded))
                .await
                .map(|_| ()),
            "draft" => provider
                .put_draft(&account(), &draft, None, &MessageMetadata::default())
                .await
                .map(|_| ()),
            _ => provider.edit_mailbox(&account(), &folder).await.map(|_| ()),
        }
        .unwrap_err();

        assert_eq!(received.lock().unwrap().len(), 1, "{write}");
        if write == "send" {
            // A send that may have left is the person's to confirm, never to retry blindly.
            assert!(failed.requires_confirmation(), "{write}: {failed}");
        } else {
            assert_eq!(failed.class(), FailureClass::Retryable, "{write}: {failed}");
        }
        assert_eq!(provider.delegate.generation(), 1, "{write}: a new client");
    }
}

/// The folders of one account share one client, so one folder meeting a dead connection replaces
/// it for both, and the other folder's next request goes out on the new one.
#[tokio::test]
async fn two_folders_on_one_client_see_one_replacement() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![
        Reply::HangUp,
        Reply::Json(ME.to_owned()),
        Reply::Json(ME.to_owned()),
    ]);
    let client = client_connecting(
        Arc::clone(&tokens),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GraphClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );
    let folder = |id: &str| {
        ReconnectingGraphProvider::new(MailboxId::try_from(id).unwrap(), Arc::clone(&client), None)
            .unwrap()
    };
    let (inbox, sent) = (folder("inbox"), folder("sentitems"));

    inbox.sender_identities(&account()).await.unwrap();
    sent.sender_identities(&account()).await.unwrap();

    assert_eq!(received.lock().unwrap().len(), 3);
    assert_eq!(sent.delegate.generation(), 1, "one replacement");
    assert_eq!(inbox.delegate.built_on(), 1);
    assert_eq!(
        sent.delegate.built_on(),
        1,
        "the other folder moved to the new client"
    );
}

/// A token near its expiry is renewed by the source before the request, which goes out with
/// the new token on the client in use.
#[tokio::test]
async fn a_token_renewed_at_its_expiry_goes_out_on_the_same_client() {
    let expiring = r#"{"token_type":"Bearer","expires_in":1,"access_token":"AT-1"}"#;
    let (tokens, minted) = tokens(vec![expiring.to_owned(), http_fake::minted(2)]);
    let (base, received) =
        http_fake::serve(vec![Reply::Json(ME.to_owned()), Reply::Json(ME.to_owned())]);
    let provider = provider_at(base, &tokens, MailboxPrincipal::Me);

    provider.sender_identities(&account()).await.unwrap();
    provider.sender_identities(&account()).await.unwrap();

    assert_eq!(
        http_fake::presented(&received),
        [http_fake::bearer(1), http_fake::bearer(2)]
    );
    assert_eq!(minted.load(Ordering::SeqCst), 2);
    assert_eq!(provider.delegate.generation(), 0, "the client was kept");
}
