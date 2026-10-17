//! The order a launch brings accounts up in, apart from what bringing one up involves, so the
//! order can be tested without a mail server.

use futures::{StreamExt, channel::mpsc};

use crate::account_registry::{MAX_CONCURRENT_DIALS, dial_all};

/// Runs `dial` for every item, and `catch_up` on each that yields a value as soon as its own dial
/// lands; `after_dials` runs once every dial has finished, beside the catch-ups still going.
///
/// Dials and catch-ups each go at most
/// [`MAX_CONCURRENT_DIALS`](crate::account_registry::MAX_CONCURRENT_DIALS) at a time, in pools
/// of their own, so a long catch-up (a body prefetch) never holds back a later account's dial,
/// and what waits on every dial never waits on a download.
pub(crate) async fn dial_then_catch_up<T, U, D, DialFut, C, CatchUpFut, A>(
    items: Vec<T>,
    dial: D,
    catch_up: C,
    after_dials: A,
) where
    D: Fn(usize, T) -> DialFut,
    DialFut: Future<Output = Option<U>>,
    C: Fn(usize, U) -> CatchUpFut,
    CatchUpFut: Future<Output = ()>,
    A: Future<Output = ()>,
{
    let (landed, connected) = mpsc::unbounded::<(usize, U)>();
    let dials = async move {
        dial_all(items, |index, item| {
            let landed = landed.clone();
            let dialled = dial(index, item);
            async move {
                if let Some(dialled) = dialled.await {
                    let _ = landed.unbounded_send((index, dialled));
                }
            }
        })
        .await;
        // `landed` is dropped here, which ends `connected` once the last catch-up is queued.
        after_dials.await;
    };
    let catch_ups = connected.for_each_concurrent(MAX_CONCURRENT_DIALS, |(index, dialled)| {
        catch_up(index, dialled)
    });
    futures::join!(dials, catch_ups);
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
