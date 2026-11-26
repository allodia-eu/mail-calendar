//! An engine provider that is connected again, by an asynchronous dial, after a request found its
//! connection dead: the shape of an adapter whose connect is itself a request (JMAP's session
//! `GET`), where [`Delegate`](crate::shared_client::Delegate) builds its provider on a client
//! connected without one.
//!
//! The same rule decides as for every other wrapper: a read is made once more on a new provider
//! after it failed on a dead connection ([`should_reconnect`]), and a write is made once, a dead
//! connection only dropping the provider for the next request ([`lost_its_connection`]).

use std::{future::Future, pin::Pin, sync::Arc};

use engine_provider::{ProviderError, ProviderResult};

use crate::shared_client::{lost_its_connection, should_reconnect};

/// Connects a new provider: the same connect that made the first one.
pub(crate) type Redial<P> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = ProviderResult<P>> + Send>> + Send + Sync>;

/// A provider connected by `redial` again once a request found it dead.
pub(crate) struct Redialled<P> {
    redial: Redial<P>,
    slot: std::sync::Mutex<Slot<P>>,
    /// Held while a new provider is connected, so callers that find none wait for one dial.
    dialling: tokio::sync::Mutex<()>,
}

/// The provider in use, the one connected last, how many were dropped before it, and how the
/// dials since the first connect ended.
struct Slot<P> {
    generation: u64,
    current: Option<Arc<P>>,
    last: Arc<P>,
    /// How many dials have ended.
    dials: u64,
    /// The last dial's failure, beside its number, while no dial has succeeded since.
    failed: Option<(u64, ProviderError)>,
}

impl<P: Send + Sync> Redialled<P> {
    /// Starts with `connected`, which a connect made; `redial` makes the next one.
    pub(crate) fn adopt(connected: P, redial: Redial<P>) -> Self {
        let connected = Arc::new(connected);
        Self {
            redial,
            slot: std::sync::Mutex::new(Slot {
                generation: 0,
                current: Some(Arc::clone(&connected)),
                last: connected,
                dials: 0,
                failed: None,
            }),
            dialling: tokio::sync::Mutex::new(()),
        }
    }

    /// The provider connected last, for what is answered without a request. Never empty, so
    /// those answers are always the adapter's own.
    pub(crate) fn last(&self) -> Arc<P> {
        Arc::clone(&self.lock().last)
    }

    /// The provider in use and its generation, connecting a new one when the last was dropped.
    ///
    /// One dial serves every caller that found no provider while it ran: they wait for it and
    /// take its answer, a failure included, so a server that cannot be reached costs one dial
    /// and one deadline however many requests were waiting.
    ///
    /// # Errors
    ///
    /// What the dial answers when no provider can be connected.
    pub(crate) async fn get(&self) -> ProviderResult<(u64, Arc<P>)> {
        let dials_seen = {
            let slot = self.lock();
            if let Some(current) = &slot.current {
                return Ok((slot.generation, Arc::clone(current)));
            }
            slot.dials
        };
        let _dialling = self.dialling.lock().await;
        {
            let slot = self.lock();
            if let Some(current) = &slot.current {
                return Ok((slot.generation, Arc::clone(current)));
            }
            if let Some((dial, failure)) = &slot.failed
                && *dial == slot.dials
                && slot.dials != dials_seen
            {
                return Err(failure.reworded(failure.detail()));
            }
        }
        let dialled = (self.redial)().await;
        let mut slot = self.lock();
        slot.dials += 1;
        match dialled {
            Ok(connected) => {
                let connected = Arc::new(connected);
                slot.current = Some(Arc::clone(&connected));
                slot.last = Arc::clone(&connected);
                slot.failed = None;
                Ok((slot.generation, connected))
            }
            Err(failure) => {
                slot.failed = Some((slot.dials, failure.reworded(failure.detail())));
                Err(failure)
            }
        }
    }

    /// Drops the provider of `generation`, which a request on it found dead. A later one is kept:
    /// another request met the same failure and a new one was connected since.
    pub(crate) fn drop_dead(&self, generation: u64) {
        let mut slot = self.lock();
        if slot.generation == generation && slot.current.is_some() {
            slot.current = None;
            slot.generation += 1;
        }
    }

    /// Runs `call`, and once more on a new provider when the first attempt failed on a dead
    /// connection ([`should_reconnect`]). For a read.
    pub(crate) async fn retrying<T, F, Fut>(&self, call: F) -> ProviderResult<T>
    where
        F: Fn(Arc<P>) -> Fut,
        Fut: Future<Output = ProviderResult<T>>,
    {
        let (generation, provider) = self.get().await?;
        match call(provider).await {
            Err(err) if should_reconnect(&err) => {
                self.drop_dead(generation);
                call(self.get().await?.1).await
            }
            result => result,
        }
    }

    /// Runs `call` once: a write, which may have been applied before the connection died. A dead
    /// connection still drops the provider ([`lost_its_connection`]), so the next request goes
    /// out on a new one.
    pub(crate) async fn once<T, Fut>(&self, call: impl FnOnce(Arc<P>) -> Fut) -> ProviderResult<T>
    where
        Fut: Future<Output = ProviderResult<T>>,
    {
        let (generation, provider) = self.get().await?;
        call(provider).await.inspect_err(|err| {
            if lost_its_connection(err) {
                self.drop_dead(generation);
            }
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot<P>> {
        self.slot.lock().expect("redialled provider mutex poisoned")
    }
}

impl<P: core::fmt::Debug> core::fmt::Debug for Redialled<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.slot.lock() {
            Ok(slot) => f
                .debug_struct("Redialled")
                .field("generation", &slot.generation)
                .field("provider", &slot.last)
                .finish_non_exhaustive(),
            Err(_) => f.debug_struct("Redialled").finish_non_exhaustive(),
        }
    }
}

#[cfg(test)]
#[path = "redialled_tests.rs"]
mod tests;
