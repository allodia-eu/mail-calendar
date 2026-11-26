//! The one way a reconnecting mail wrapper streams a pass: IMAP's, Graph's and Gmail's alike.
//!
//! Each chunk is handed on the moment the delegate yields it, because the sync commits per chunk:
//! a chunk held back until the pass ends is lost with the pass when a later page fails, and on a
//! provider that refuses partway (Gmail's per-minute quota) that is every pass.

use std::{future::Future, sync::Arc};

use engine_core::{
    ids::AccountId,
    sync::{MailboxWindows, SyncState},
};
use engine_provider::{EmailStream, Provider, ProviderResult};
use futures::StreamExt;

/// Streams one pass from the delegate `delegate` hands out, forwarding each chunk as it arrives.
/// `delegate` answers what it was built on beside it, which `invalidate` is handed back, so a
/// failure drops only the connection it met.
///
/// A failure on a dead connection ([`should_reconnect`](crate::shared_client::should_reconnect):
/// a socket that died while the machine slept) drops the delegate through `invalidate` and runs the
/// pass again, once, on a fresh one; the chunks already passed on are written again, which is
/// harmless. Any other failure, or a second drop, ends the pass with what was already passed on
/// committed, and a second drop still drops the delegate, so the next call dials afresh.
pub(crate) fn redialling_stream<'a, P, G, D, F>(
    delegate: D,
    invalidate: impl Fn(G) + Send + 'a,
    account: &AccountId,
    cursor: Option<&SyncState>,
    window: MailboxWindows,
    fetch_batch: usize,
    chunk_size: usize,
) -> EmailStream<'a>
where
    P: Provider + ?Sized + 'a,
    D: Fn() -> F + Send + 'a,
    G: Copy + Send + 'a,
    F: Future<Output = ProviderResult<(G, Arc<P>)>> + Send + 'a,
{
    let account = account.clone();
    let cursor = cursor.cloned();
    Box::pin(async_stream::try_stream! {
        let mut retried = false;
        'attempt: loop {
            let (built_on, provider) = delegate().await?;
            let mut stream = provider.stream_email(
                &account,
                cursor.as_ref(),
                window.clone(),
                fetch_batch,
                chunk_size,
            );
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => yield chunk,
                    Err(err) if crate::shared_client::should_reconnect(&err) => {
                        invalidate(built_on);
                        if retried {
                            Err(err)?;
                        }
                        retried = true;
                        continue 'attempt;
                    }
                    Err(err) => Err(err)?,
                }
            }
            break;
        }
    })
}

#[cfg(test)]
#[path = "stream_redial_tests.rs"]
mod tests;
