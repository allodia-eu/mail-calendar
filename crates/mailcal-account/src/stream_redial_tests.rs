//! Streaming a pass through [`redialling_stream`]: its chunks reach the sync as they arrive, a
//! refusal keeps what came before it, a connection that dies is redialled once, before the first
//! chunk or after, and a second drop is reported.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use engine_api::CalendarWrites;
use engine_core::{
    error::FailureClass,
    ids::AccountId,
    mail::Mailbox,
    sync::{SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, Provider, ProviderError, ProviderResult,
    ScopeSync,
};
use futures::StreamExt;

use super::redialling_stream;

/// What one step of a scripted pass does.
#[derive(Clone, Copy)]
enum Step {
    /// Delivers a chunk.
    Chunk,
    /// The connection drops.
    Drop,
    /// The server refuses for now, as Gmail does once a user's per-minute quota is spent.
    Refuse,
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
                    Step::Refuse => Err(ProviderError::new(FailureClass::RateLimited, "quota spent"))?,
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

/// A pass whose first delegate plays `first` and whose every later one plays `then`; each
/// delegate after the first is a redial, and is counted.
fn pass<'a>(first: Vec<Step>, then: Vec<Step>, redials: &Arc<AtomicUsize>) -> EmailStream<'a> {
    let handed_out = Arc::new(AtomicUsize::new(0));
    let redials = Arc::clone(redials);
    let delegate = move || {
        let steps = if handed_out.fetch_add(1, Ordering::SeqCst) == 0 {
            first.clone()
        } else {
            redials.fetch_add(1, Ordering::SeqCst);
            then.clone()
        };
        async move { Ok(Arc::new(StreamDelegate { steps })) }
    };
    redialling_stream(
        delegate,
        || {},
        &account(),
        None,
        SyncWindow::full(),
        200,
        50,
    )
}

/// Every outcome of one pass, `true` for a chunk and `false` for an error.
async fn outcomes(first: Vec<Step>, then: Vec<Step>, redials: &Arc<AtomicUsize>) -> Vec<bool> {
    pass(first, then, redials)
        .map(|item| item.is_ok())
        .collect()
        .await
}

/// A first sync of a large mailbox takes many seconds, and each chunk is what puts rows on
/// screen, so a chunk must reach the sync while the server is still sending the rest.
#[tokio::test]
async fn a_pass_reaches_the_sync_as_it_arrives() {
    let redials = Arc::new(AtomicUsize::new(0));
    let mut stream = pass(vec![Step::Chunk, Step::Hang], Vec::new(), &redials);

    let first = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;

    assert!(
        matches!(first, Ok(Some(Ok(_)))),
        "the first chunk waited for the rest of the pass"
    );
}

/// The sync commits each chunk it is given, so a pass refused partway keeps what it fetched
/// before the refusal. Held back until the pass ended, the chunks went down with it, and a
/// mailbox larger than one quota window never stored a message.
#[tokio::test]
async fn a_refusal_after_a_chunk_keeps_the_chunk() {
    let redials = Arc::new(AtomicUsize::new(0));

    let seen = outcomes(
        vec![Step::Chunk, Step::Chunk, Step::Refuse],
        Vec::new(),
        &redials,
    )
    .await;

    assert_eq!(seen, [true, true, false]);
    assert_eq!(
        redials.load(Ordering::SeqCst),
        0,
        "a refusal is not a dead connection"
    );
}

/// A session that died while the machine slept fails on its first command: redialled once, the
/// pass runs on the fresh one.
#[tokio::test]
async fn a_connection_that_dies_before_the_first_chunk_is_redialled_once() {
    let redials = Arc::new(AtomicUsize::new(0));

    let seen = outcomes(vec![Step::Drop], vec![Step::Chunk, Step::Chunk], &redials).await;

    assert_eq!(seen, [true, true]);
    assert_eq!(redials.load(Ordering::SeqCst), 1);
}

/// A connection dropped partway through a first sync is redialled once and the pass run again,
/// rather than leaving the rest for the next pass: the rows already passed on are written again,
/// which is harmless.
#[tokio::test]
async fn a_connection_that_dies_after_a_chunk_is_redialled_once() {
    let redials = Arc::new(AtomicUsize::new(0));

    let seen = outcomes(
        vec![Step::Chunk, Step::Drop],
        vec![Step::Chunk, Step::Chunk],
        &redials,
    )
    .await;

    assert_eq!(seen, [true, true, true]);
    assert_eq!(redials.load(Ordering::SeqCst), 1);
}

/// A drop on the fresh session too is reported, and the next pass resumes from the last
/// committed checkpoint.
#[tokio::test]
async fn a_second_drop_is_reported() {
    let redials = Arc::new(AtomicUsize::new(0));

    let seen = outcomes(vec![Step::Drop], vec![Step::Chunk, Step::Drop], &redials).await;

    assert_eq!(seen, [true, false]);
    assert_eq!(redials.load(Ordering::SeqCst), 1);
}
