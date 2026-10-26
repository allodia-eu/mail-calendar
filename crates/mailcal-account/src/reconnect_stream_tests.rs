//! Streaming a pass through the redial wrapper: its chunks reach the sync as they arrive, a
//! connection that dies before the first one is redialled once, and one that dies after is
//! reported rather than replayed.
//!
//! In their own `#[path]` file to keep `reconnect_tests.rs` under the 500-line limit.

use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use engine_core::sync::SyncUpdate;
use engine_provider::{Capabilities, EmailChunk};

use super::*;

/// What one step of a scripted pass does.
#[derive(Clone, Copy)]
enum Step {
    /// Delivers a chunk.
    Chunk,
    /// The connection drops.
    Drop,
    /// The server goes quiet, as one still sending a large mailbox does.
    Hang,
}

/// A delegate whose `stream_email` plays `steps`.
struct StreamDelegate {
    steps: Vec<Step>,
}

#[async_trait]
impl Provider for StreamDelegate {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_mail())
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(Vec::new(), BTreeSet::new()),
            SyncState::new("cursor"),
        ))
    }

    fn stream_email<'a>(
        &'a self,
        _account: &'a AccountId,
        _cursor: Option<&'a SyncState>,
        _window: SyncWindow,
        _fetch_batch: usize,
        _chunk_size: usize,
    ) -> EmailStream<'a> {
        let steps = self.steps.clone();
        Box::pin(async_stream::try_stream! {
            for step in steps {
                match step {
                    Step::Chunk => yield EmailChunk::additive_held(Vec::new(), Vec::new(), None),
                    Step::Drop => Err(ProviderError::new(FailureClass::Retryable, "connection reset"))?,
                    Step::Hang => futures::future::pending::<()>().await,
                }
            }
        })
    }
}

impl engine_api::MailboxWrites for StreamDelegate {}
impl CalendarWrites for StreamDelegate {}

fn account() -> AccountId {
    AccountId::try_from("acct").expect("valid account id")
}

fn mailbox() -> MailboxId {
    MailboxId::try_from("INBOX").expect("valid mailbox id")
}

/// A wrapper over a session playing `first`, whose redials play `then` and are counted.
fn wrapper(
    first: Vec<Step>,
    then: Vec<Step>,
    redials: &Arc<AtomicUsize>,
) -> ReconnectingImapProvider {
    let redials = Arc::clone(redials);
    let redial: Redial = Box::new(move || {
        redials.fetch_add(1, Ordering::SeqCst);
        let steps = then.clone();
        Box::pin(async move { Ok(Arc::new(StreamDelegate { steps }) as Arc<dyn Provider>) })
    });
    ReconnectingImapProvider::adopt(Arc::new(StreamDelegate { steps: first }), mailbox(), redial)
}

/// Every outcome of one pass, `true` for a chunk and `false` for an error.
async fn outcomes(provider: &ReconnectingImapProvider) -> Vec<bool> {
    let account = account();
    let stream = provider.stream_email(&account, None, SyncWindow::full(), 200, 50);
    stream.map(|item| item.is_ok()).collect().await
}

/// A first sync of a large mailbox takes many seconds, and each chunk is what puts rows on
/// screen, so a chunk must reach the sync while the server is still sending the rest.
#[tokio::test]
async fn a_pass_reaches_the_sync_as_it_arrives() {
    let redials = Arc::new(AtomicUsize::new(0));
    let provider = wrapper(vec![Step::Chunk, Step::Hang], Vec::new(), &redials);
    let account = account();
    let mut stream = provider.stream_email(&account, None, SyncWindow::full(), 200, 50);

    let first = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;

    assert!(
        matches!(first, Ok(Some(Ok(_)))),
        "the first chunk waited for the rest of the pass"
    );
}

/// A session that died while the machine slept fails on its first command: redialled once, the
/// pass runs on the fresh one.
#[tokio::test]
async fn a_connection_that_dies_before_the_first_chunk_is_redialled_once() {
    let redials = Arc::new(AtomicUsize::new(0));
    let provider = wrapper(vec![Step::Drop], vec![Step::Chunk, Step::Chunk], &redials);

    assert_eq!(outcomes(&provider).await, [true, true]);
    assert_eq!(redials.load(Ordering::SeqCst), 1);
}

/// Chunks already passed on are committed, so replaying the pass would deliver them twice. The
/// drop is reported instead, and the next pass, which resumes from the last committed
/// checkpoint, runs on a fresh session.
#[tokio::test]
async fn a_connection_that_dies_after_a_chunk_is_reported_not_replayed() {
    let redials = Arc::new(AtomicUsize::new(0));
    let provider = wrapper(vec![Step::Chunk, Step::Drop], vec![Step::Chunk], &redials);

    assert_eq!(outcomes(&provider).await, [true, false]);
    assert_eq!(redials.load(Ordering::SeqCst), 0, "nothing was replayed");

    assert_eq!(outcomes(&provider).await, [true]);
    assert_eq!(
        redials.load(Ordering::SeqCst),
        1,
        "the next pass took a fresh session"
    );
}
