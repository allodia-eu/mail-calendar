//! `mailcal-telemetry`: the HTTPS delivery adapter for consented product analytics.
//!
//! The product core builds a [`Batch`] and hands it to a `mailcal_app::TelemetrySink`; this crate
//! is the one implementation that puts it on the network. Keeping delivery *here* rather than in
//! `mailcal-app` is what lets the core stay network-free: the demo, the showcase, and every test
//! run with no sink at all, so no test can accidentally phone home.
//!
//! # The four rules delivery must obey
//!
//! 1. **Never block.** [`HttpTelemetrySink::send`] pushes onto a bounded channel and returns. It is
//!    called from the runtime's worker threads, on the dispatch and sync paths; a slow or dead
//!    endpoint must not slow the app down by a single millisecond.
//! 2. **Never grow without bound.** The queue is capped (`QUEUE_CAPACITY`) and **drops on
//!    overflow**. Telemetry is best-effort by definition; losing a batch is always allowed, and an
//!    unbounded queue in front of a dead endpoint is a memory leak.
//! 3. **Never persist.** Events live in memory and nowhere else. There is no on-disk spool, so
//!    there is no analytics data at rest on the user's device to leak, and a crash simply loses a
//!    few counts.
//! 4. **Never storm.** One bounded attempt per batch, then drop. A self-hosted or air-gapped
//!    deployment runs with the endpoint permanently unreachable, and it must behave *identically*
//!    to one with a reachable endpoint: no retry loop, no backoff spiral, no error surfaced to the
//!    user (sovereignty doctrine, enforcement principle 4).
//!
//! # What it sends
//!
//! A POST to the Allodia relay, whose whole job is to be the place the user's IP address stops.
//! The relay drops the IP, validates the payload against a closed key whitelist, holds the
//! analytics backend's credential (a shipped binary cannot keep a secret), and forwards on. See
//! `docs/analytics.md`.

use std::time::Duration;

use mailcal_app::{Batch, TelemetrySink};
use tokio::sync::mpsc;

/// How many batches may be in flight before we start dropping them. Small on purpose: the app
/// emits a handful of events per session, so a queue this deep is only ever reached when the
/// endpoint is unreachable, and that is exactly the case where dropping is correct.
const QUEUE_CAPACITY: usize = 64;

/// How long one delivery attempt may take before it is abandoned. Short: a batch of counts is
/// worth no more than this.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Something that stopped the sink from being built. Delivery failures are **not** errors; they
/// are dropped batches: so this is only ever about construction.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// The shared TLS policy could not be built.
    #[error("tls error: {0}")]
    Tls(engine_tls::TlsError),
    /// The HTTP client could not be built.
    #[error("http client error: {0}")]
    Transport(#[from] reqwest::Error),
}

/// What the sink talks to: the Allodia relay's base URL, and the app key that identifies this
/// product to it.
///
/// The key is **not a secret** and is not treated as one; it ships in the binary, so anyone can
/// read it. It exists to let the relay tell Allodia Mail's events from another product's, and to
/// let us turn one product's ingest off without a client release. The credential that *is* secret
/// (the analytics backend's) lives on the relay, which is the whole reason the relay exists.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// The relay's base URL, e.g. `https://telemetry.allodia.eu`.
    pub base_url: String,
    /// The app key identifying this product to the relay.
    pub app_key: String,
}

/// Delivers consented analytics batches to the Allodia relay over HTTPS.
///
/// Construction spawns one detached worker on the ambient tokio runtime; [`send`](Self::send) and
/// [`erase`](Self::erase) hand it work over a bounded channel and return immediately.
#[derive(Debug)]
pub struct HttpTelemetrySink {
    queue: mpsc::Sender<Job>,
}

/// One unit of work for the delivery worker.
#[derive(Debug)]
enum Job {
    /// Deliver a batch of events.
    Send(Box<Batch>),
    /// Ask the backend to erase everything held under an install id (GDPR Art. 17).
    Erase(String),
}

impl HttpTelemetrySink {
    /// Builds the sink and spawns its delivery worker.
    ///
    /// Must be called from within a tokio runtime context (the bindings construct it inside the
    /// app's runtime, alongside the engine).
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError`] if the shared TLS policy or the HTTP client cannot be built.
    /// Note this is the *only* fallible part: once running, a delivery that fails is dropped, not
    /// reported.
    pub fn new(config: RelayConfig) -> Result<Self, TelemetryError> {
        // The same TLS construction the OAuth client uses: `reqwest` is kept off its default TLS
        // path workspace-wide, so an HTTPS client is built from engine-tls's rustls config.
        let tls = engine_tls::client_config(&engine_tls::TlsPolicy::bundled_and_system())
            .map_err(TelemetryError::Tls)?;
        let http = tls.reqwest_builder().timeout(REQUEST_TIMEOUT).build()?;

        let (queue, mut jobs) = mpsc::channel::<Job>(QUEUE_CAPACITY);
        tokio::spawn(async move {
            while let Some(job) = jobs.recv().await {
                deliver(&http, &config, job).await;
            }
        });
        Ok(Self { queue })
    }
}

impl TelemetrySink for HttpTelemetrySink {
    /// Queues a batch. **Never blocks and never fails**: a full queue means the endpoint is not
    /// keeping up (or is gone), and the batch is dropped. `try_send` (not `send`) is the whole
    /// point; awaiting here would push network latency onto the dispatch path.
    fn send(&self, batch: Batch) {
        if self.queue.try_send(Job::Send(Box::new(batch))).is_err() {
            // Counted, not retried. A dropped batch of counts costs nothing; a retry storm on a
            // flaky network costs the user battery and bandwidth.
            log::debug!("telemetry: queue full or closed, dropping a batch");
        }
    }

    /// Queues an erasure request. Best-effort like everything else: the install id has already
    /// been deleted from the device by the time this is called, so the user's local state is
    /// correct regardless of whether this lands.
    fn erase(&self, install_id: String) {
        if self.queue.try_send(Job::Erase(install_id)).is_err() {
            log::warn!("telemetry: could not queue an erasure request");
        }
    }
}

/// Performs one delivery attempt. Every outcome (success, HTTP error, transport failure) is a
/// `debug`/`warn` log line and nothing more. **One attempt, then done**: see rule 4 in the module
/// docs.
async fn deliver(http: &reqwest::Client, config: &RelayConfig, job: Job) {
    let (url, body) = match job {
        Job::Send(batch) => (
            format!("{}/v1/events", config.base_url.trim_end_matches('/')),
            serde_json::to_value(&*batch).ok(),
        ),
        Job::Erase(install_id) => (
            format!(
                "{}/v1/installs/{install_id}/erase",
                config.base_url.trim_end_matches('/')
            ),
            None,
        ),
    };

    let mut request = http.post(&url).header("x-allodia-app-key", &config.app_key);
    if let Some(body) = body {
        request = request.json(&body);
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            log::debug!("telemetry: delivered ({})", response.status());
        }
        Ok(response) => {
            // The relay rejected it; most likely a payload key that isn't on its whitelist, which
            // means the core and the relay have drifted. Log the status, never the body: a relay
            // error message is not ours to assume is safe.
            log::warn!(
                "telemetry: relay rejected the request ({})",
                response.status()
            );
        }
        Err(_) => {
            // Offline, air-gapped, or self-hosted with no relay. This is a **normal** state, not an
            // error: it must never surface to the user or change the app's behaviour. No message is
            // logged from the error itself: a transport error embeds the URL and can embed proxy
            // details, and the log is attachable to a support request.
            log::debug!("telemetry: endpoint unreachable, dropping");
        }
    }
}
