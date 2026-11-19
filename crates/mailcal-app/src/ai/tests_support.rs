//! A host and a relay for the assistant's tests: a transport answering from a script and recording
//! what it was sent, a keystore that can be told to refuse, and a runtime for the engine's reads.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpResponse, HttpTransport, TransportFailed,
    wire::{ChatRequest, ChatResponse},
};
use tempfile::TempDir;

use super::{AiCancel, AiHost, AiRelay, BuildKind, KeyNotStored, RelayStatus};
use crate::{
    App, Surface,
    tests_fakes::{FakeProvider, account, app, app_with_prefs},
};

/// What the host saw.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    /// Each request: its URL and its bearer.
    pub(crate) requests: Vec<(String, Option<String>)>,
    /// Each keystore write: the key, or `None` for a removal.
    pub(crate) keys: Vec<Option<String>>,
    /// Whether a request saw its stop set.
    pub(crate) stopped: bool,
}

/// A host whose transport answers `200` with an empty choice list and whose keystore takes every
/// key until told otherwise.
#[derive(Default)]
pub(crate) struct FakeHost {
    pub(crate) seen: Arc<Mutex<Seen>>,
    pub(crate) refuse_keys: Mutex<bool>,
}

struct FakeTransport {
    seen: Arc<Mutex<Seen>>,
    cancel: AiCancel,
}

impl HttpTransport for FakeTransport {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        let mut seen = self.seen.lock().unwrap();
        seen.requests
            .push((request.url.to_owned(), request.bearer.map(str::to_owned)));
        if self.cancel.is_set() {
            seen.stopped = true;
            return Err(TransportFailed);
        }
        Ok(HttpResponse {
            status: 200,
            body: r#"{"choices":[]}"#.to_owned(),
        })
    }
}

impl AiHost for FakeHost {
    fn transport(&self, cancel: AiCancel) -> Box<dyn HttpTransport> {
        Box::new(FakeTransport {
            seen: Arc::clone(&self.seen),
            cancel,
        })
    }

    fn wait(&self, work: Pin<Box<dyn Future<Output = ()> + Send + '_>>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(work);
    }

    fn store_endpoint_key(&self, key: Option<&str>) -> Result<(), KeyNotStored> {
        if *self.refuse_keys.lock().unwrap() {
            return Err(KeyNotStored);
        }
        self.seen.lock().unwrap().keys.push(key.map(str::to_owned));
        Ok(())
    }
}

/// A relay in a state a test sets, whose backend answers every request with `answer`.
pub(crate) struct FakeRelay {
    pub(crate) status: Mutex<RelayStatus>,
    pub(crate) asked: Mutex<u32>,
}

impl FakeRelay {
    pub(crate) fn new(status: RelayStatus) -> Self {
        Self {
            status: Mutex::new(status),
            asked: Mutex::new(0),
        }
    }
}

struct RelayBackend;

impl AiBackend for RelayBackend {
    fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, AiError> {
        Err(AiError::Unauthorized)
    }
}

impl AiRelay for FakeRelay {
    fn status(&self) -> RelayStatus {
        *self.status.lock().unwrap()
    }

    fn backend(&self, _transport: Box<dyn HttpTransport>) -> Box<dyn AiBackend> {
        *self.asked.lock().unwrap() += 1;
        Box::new(RelayBackend)
    }
}

/// An app with one mail account, a [`FakeHost`] installed as a `build`, and the surfaces it
/// signals.
pub(crate) struct Fixture {
    pub(crate) app: App<FakeProvider>,
    pub(crate) host: Arc<FakeHost>,
    pub(crate) surfaces: Arc<Mutex<Vec<Surface>>>,
    /// The data directory, when the app keeps its files.
    pub(crate) dir: Option<TempDir>,
}

impl Fixture {
    /// Keeps nothing on disk.
    pub(crate) fn new(build: BuildKind) -> Self {
        let surfaces = Arc::new(Mutex::new(Vec::new()));
        let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
        Self::installed(app, surfaces, None, build)
    }

    /// Keeps its files in a directory of its own, which `seed` may write first.
    pub(crate) fn stored(build: BuildKind, seed: impl FnOnce(&std::path::Path)) -> Self {
        let dir = TempDir::new().unwrap();
        seed(dir.path());
        Self::reopened(dir, build)
    }

    /// An app over `dir` as a relaunch finds it.
    pub(crate) fn reopened(dir: TempDir, build: BuildKind) -> Self {
        let surfaces = Arc::new(Mutex::new(Vec::new()));
        let app = app_with_prefs(
            vec![account("acct-1", FakeProvider::new())],
            dir.path().join("preferences.toml"),
            &surfaces,
        );
        Self::installed(app, surfaces, Some(dir), build)
    }

    fn installed(
        app: App<FakeProvider>,
        surfaces: Arc<Mutex<Vec<Surface>>>,
        dir: Option<TempDir>,
        build: BuildKind,
    ) -> Self {
        let host = Arc::new(FakeHost::default());
        app.install_ai_host(Arc::clone(&host) as Arc<dyn AiHost>, build);
        surfaces.lock().unwrap().clear();
        Self {
            app,
            host,
            surfaces,
            dir,
        }
    }

    /// Whether `surface` was signalled since the last [`Fixture::forget_surfaces`].
    pub(crate) fn signalled(&self, surface: Surface) -> bool {
        self.surfaces.lock().unwrap().contains(&surface)
    }

    pub(crate) fn forget_surfaces(&self) {
        self.surfaces.lock().unwrap().clear();
    }
}
