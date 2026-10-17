//! The order a launch brings accounts up in, apart from what bringing one up involves, so the
//! order can be tested without a mail server.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use futures::channel::oneshot;

use crate::account_registry::dial_all;

/// Runs `dial` for every item and, for each that yields a value, `catch_up` on it in the same
/// slot, at most [`MAX_CONCURRENT_DIALS`](crate::account_registry::MAX_CONCURRENT_DIALS) slots at
/// a time; and runs `after_dials` once every dial has finished, beside the catch-ups still going.
///
/// So an account's catch-up starts as soon as its own dial lands, never waiting on a slower
/// server, and the bound covers the catch-ups as well as the dials. `after_dials` runs at once
/// when there is nothing to dial.
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
    let remaining = AtomicUsize::new(items.len());
    let (all_dialled, dialled) = oneshot::channel::<()>();
    let all_dialled = Mutex::new(Some(all_dialled));
    let (dial, catch_up) = (Arc::new(dial), Arc::new(catch_up));
    let slots = dial_all(items, |index, item| {
        let (dial, catch_up) = (Arc::clone(&dial), Arc::clone(&catch_up));
        let (remaining, all_dialled) = (&remaining, &all_dialled);
        async move {
            let dialled = dial(index, item).await;
            if remaining.fetch_sub(1, Ordering::AcqRel) == 1
                && let Some(sender) = all_dialled.lock().expect("dialled mutex poisoned").take()
            {
                let _ = sender.send(());
            }
            if let Some(dialled) = dialled {
                catch_up(index, dialled).await;
            }
        }
    });
    let after = async {
        // Cancelled rather than sent when nothing was dialled: the sender is dropped unused.
        let _ = dialled.await;
        after_dials.await;
    };
    // The sender lives in `all_dialled`, which outlives `slots`, so with nothing to dial it is
    // dropped only here; send it now so `after` does not wait for it.
    if remaining.load(Ordering::Acquire) == 0
        && let Some(sender) = all_dialled.lock().expect("dialled mutex poisoned").take()
    {
        let _ = sender.send(());
    }
    futures::join!(slots, after);
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
