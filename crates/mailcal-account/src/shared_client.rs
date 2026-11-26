//! The HTTP client an account's Graph or Google providers share, and the engine provider each
//! wrapper builds on it.
//!
//! The client asks the account's credential source for the token on every request, so nothing
//! about a token ends it. A dead connection does: a keep-alive socket the OS closed while the
//! machine slept fails a request as `Retryable`, and the pool that held it may hold more like it.
//! The wrapper that met the failure drops the client ([`Delegate::drop_client`]); the next request
//! of every wrapper on it is made on a new one.

use std::sync::{Arc, Mutex};

use engine_core::error::FailureClass;
use engine_provider::{ProviderError, ProviderResult};

/// One client, connected on first use and again after it was dropped.
pub(crate) struct SharedClient<C> {
    connect: Box<dyn Fn() -> ProviderResult<C> + Send + Sync>,
    slot: Mutex<Slot<C>>,
}

/// The client in use, and how many clients were dropped before it.
struct Slot<C> {
    generation: u64,
    client: Option<C>,
}

impl<C: Clone> SharedClient<C> {
    /// A client `connect` builds; nothing is sent until its first request.
    pub(crate) fn new(connect: impl Fn() -> ProviderResult<C> + Send + Sync + 'static) -> Self {
        Self {
            connect: Box::new(connect),
            slot: Mutex::new(Slot {
                generation: 0,
                client: None,
            }),
        }
    }

    /// The client in use and its generation, connecting one when the last was dropped.
    ///
    /// # Errors
    ///
    /// What `connect` answers when the client cannot be built.
    pub(crate) fn current(&self) -> ProviderResult<(u64, C)> {
        let mut slot = self.slot.lock().expect("shared client mutex poisoned");
        if let Some(client) = &slot.client {
            return Ok((slot.generation, client.clone()));
        }
        let client = (self.connect)()?;
        slot.client = Some(client.clone());
        Ok((slot.generation, client))
    }

    /// The generation of the client the next request is made on.
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.slot
            .lock()
            .expect("shared client mutex poisoned")
            .generation
    }

    /// Drops the client of `generation`, which a request found dead. A later client is kept:
    /// another wrapper met the same failure and has already replaced it.
    pub(crate) fn drop_dead(&self, generation: u64) {
        let mut slot = self.slot.lock().expect("shared client mutex poisoned");
        if slot.generation == generation && slot.client.is_some() {
            slot.client = None;
            slot.generation += 1;
        }
    }
}

impl<C> core::fmt::Debug for SharedClient<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let generation = self.slot.lock().map_or(0, |slot| slot.generation);
        f.debug_struct("SharedClient")
            .field("generation", &generation)
            .finish_non_exhaustive()
    }
}

/// The engine provider a wrapper forwards to, built on the shared client and built again once
/// that client was replaced. Never empty: the first is built with the wrapper, so what is answered
/// without a request ([`last`](Self::last)) is always the engine's answer.
pub(crate) struct Delegate<C, P> {
    client: Arc<SharedClient<C>>,
    build: Box<dyn Fn(C) -> P + Send + Sync>,
    built: Mutex<(u64, Arc<P>)>,
}

impl<C: Clone, P> Delegate<C, P> {
    /// The provider `build` makes of `client`'s current client.
    ///
    /// # Errors
    ///
    /// When the client cannot be built.
    pub(crate) fn new(
        client: Arc<SharedClient<C>>,
        build: impl Fn(C) -> P + Send + Sync + 'static,
    ) -> ProviderResult<Self> {
        let (generation, current) = client.current()?;
        let first = Arc::new(build(current));
        Ok(Self {
            client,
            build: Box::new(build),
            built: Mutex::new((generation, first)),
        })
    }

    /// The provider on the client in use, and that client's generation, which a request that
    /// fails on it hands back to [`drop_client`](Self::drop_client). Built again only for a newer
    /// client: a caller that read an older one never stores its provider over a newer one.
    ///
    /// # Errors
    ///
    /// When a replacement client cannot be built.
    pub(crate) fn get(&self) -> ProviderResult<(u64, Arc<P>)> {
        let (generation, client) = self.client.current()?;
        let mut built = self.built.lock().expect("delegate mutex poisoned");
        if generation > built.0 {
            *built = (generation, Arc::new((self.build)(client)));
        }
        Ok((built.0, Arc::clone(&built.1)))
    }

    /// The provider built last, for what is answered without a request.
    pub(crate) fn last(&self) -> Arc<P> {
        Arc::clone(&self.built.lock().expect("delegate mutex poisoned").1)
    }

    /// Drops the client of `generation`: a request on it failed as a dead connection does. A
    /// client already replaced is kept.
    pub(crate) fn drop_client(&self, generation: u64) {
        self.client.drop_dead(generation);
    }

    /// Runs `call` on the provider, and once more on a new client when the first attempt failed
    /// on a dead connection ([`should_reconnect`]). For a request that repeats safely: a read, or
    /// an edit that lands on the same state however often it is applied.
    pub(crate) async fn retrying<T, F, Fut>(&self, call: F) -> ProviderResult<T>
    where
        F: Fn(Arc<P>) -> Fut,
        Fut: Future<Output = ProviderResult<T>>,
    {
        let (generation, provider) = self.get()?;
        match call(provider).await {
            Err(err) if should_reconnect(&err) => {
                self.drop_client(generation);
                call(self.get()?.1).await
            }
            result => result,
        }
    }

    /// Runs `call` once: a request that must not be repeated, because it may have been applied
    /// before the connection died. A dead connection still drops the client
    /// ([`lost_its_connection`]), so the next request goes out on a new one.
    pub(crate) async fn once<T, Fut>(&self, call: impl FnOnce(Arc<P>) -> Fut) -> ProviderResult<T>
    where
        Fut: Future<Output = ProviderResult<T>>,
    {
        let (generation, provider) = self.get()?;
        call(provider).await.inspect_err(|err| {
            if lost_its_connection(err) {
                self.drop_client(generation);
            }
        })
    }

    /// The generation of the shared client the next request is made on.
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.client.generation()
    }

    /// The generation of the shared client the last provider was built on.
    #[cfg(test)]
    pub(crate) fn built_on(&self) -> u64 {
        self.built.lock().expect("delegate mutex poisoned").0
    }
}

/// Whether a failed request warrants a new client: a retryable **transport** failure (a
/// keep-alive socket the OS killed while the machine slept, a broken pipe) that a fresh
/// connection pool may clear. Not a `429`, which is waited out one layer down on the same
/// connection; not a refusal of the credential, which the engine has already answered with one
/// replaced token; and not a credential that could not be had
/// ([`ProviderError::is_credential_failure`]: the source failed or did not answer in time), which
/// fails the request or the dial before anything is sent, so a new connection would only ask the
/// same source again. Tried at most once per call.
///
/// Repeating the request after it is safe for a read and for an edit that lands on the same state
/// however often it is applied. A send, a create and a folder change are made
/// [`once`](Delegate::once): a transport failure cannot tell a request lost before the server
/// from one lost after it, so the outbox, not the wrapper, decides whether to send it again.
pub(crate) fn should_reconnect(err: &ProviderError) -> bool {
    err.class() == FailureClass::Retryable && !err.is_credential_failure()
}

/// Whether a write made [`once`](Delegate::once) failed on its connection: as a read does
/// ([`should_reconnect`]), or after the request may have reached the server, which the engine
/// answers as a change to confirm rather than as a failure that may pass.
pub(crate) fn lost_its_connection(err: &ProviderError) -> bool {
    should_reconnect(err) || err.requires_confirmation()
}

impl<C, P: core::fmt::Debug> core::fmt::Debug for Delegate<C, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.built.lock() {
            Ok(built) => f
                .debug_struct("Delegate")
                .field("generation", &built.0)
                .field("provider", &built.1)
                .finish_non_exhaustive(),
            Err(_) => f.debug_struct("Delegate").finish_non_exhaustive(),
        }
    }
}

#[cfg(test)]
#[path = "shared_client_tests.rs"]
mod tests;
