//! Shared test doubles: a scriptable [`Fetch`] and [`MxResolver`], so strategies and
//! the orchestrator are exercised deterministically without real sockets or DNS.

use std::{collections::HashMap, time::Duration};

use url::Url;

use crate::{
    fetch::{Fetch, FetchOutcome, FetchResponse},
    mx::{MxError, MxRecord, MxResolution, MxResolver, SrvRecord, SrvResolution},
};

/// A canned reply for one URL.
#[derive(Clone)]
pub(crate) enum Reply {
    /// A terminal HTTP response.
    Ok {
        /// Status code.
        status: u16,
        /// Body bytes.
        body: Vec<u8>,
        /// Whether the (simulated) chain stayed HTTPS.
        trusted: bool,
        /// Whether a `WWW-Authenticate` header was present.
        www_authenticate: bool,
    },
    /// An unusable response chain.
    Miss,
    /// A transport failure.
    Net,
}

impl Reply {
    /// A trusted 2xx autoconfig-XML body.
    pub(crate) fn xml(body: &str) -> Self {
        Self::Ok {
            status: 200,
            body: body.as_bytes().to_vec(),
            trusted: true,
            www_authenticate: false,
        }
    }

    /// A 2xx JSON body with the given trust.
    pub(crate) fn json(body: &str, trusted: bool) -> Self {
        Self::Ok {
            status: 200,
            body: body.as_bytes().to_vec(),
            trusted,
            www_authenticate: false,
        }
    }

    /// A `401` challenge, optionally carrying `WWW-Authenticate`.
    pub(crate) fn unauthorized(www_authenticate: bool) -> Self {
        Self::Ok {
            status: 401,
            body: Vec::new(),
            trusted: true,
            www_authenticate,
        }
    }

    /// A body-less response with an arbitrary status and trust: for the CalDAV probe,
    /// whose signal is the status code (and whether the hops stayed HTTPS).
    pub(crate) fn status(status: u16, trusted: bool) -> Self {
        Self::Ok {
            status,
            body: Vec::new(),
            trusted,
            www_authenticate: false,
        }
    }
}

/// A [`Fetch`] that replays canned replies per URL (unknown URLs get a configurable
/// default), each after an optional virtual-time delay.
pub(crate) struct FakeFetch {
    entries: HashMap<String, (Reply, Duration)>,
    default: Reply,
}

impl FakeFetch {
    /// A fake whose unknown URLs return [`Reply::Miss`].
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            default: Reply::Miss,
        }
    }

    /// Serves `reply` for `url`.
    pub(crate) fn on(mut self, url: &str, reply: Reply) -> Self {
        self.entries.insert(url.to_owned(), (reply, Duration::ZERO));
        self
    }

    /// Serves `reply` for `url` after `delay` of (virtual) time; used to prove the
    /// orchestrator's priority ordering under `tokio` paused time.
    pub(crate) fn on_after(mut self, url: &str, reply: Reply, delay: Duration) -> Self {
        self.entries.insert(url.to_owned(), (reply, delay));
        self
    }

    /// Sets the reply for any URL not otherwise scripted.
    pub(crate) fn default_reply(mut self, reply: Reply) -> Self {
        self.default = reply;
        self
    }
}

#[async_trait::async_trait]
impl Fetch for FakeFetch {
    async fn get(&self, url: &Url) -> FetchOutcome {
        let (reply, delay) = self
            .entries
            .get(url.as_str())
            .cloned()
            .unwrap_or((self.default.clone(), Duration::ZERO));
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        match reply {
            Reply::Ok {
                status,
                body,
                trusted,
                www_authenticate,
            } => FetchOutcome::Response(FetchResponse {
                status,
                www_authenticate,
                body,
                trusted,
                final_url: url.clone(),
            }),
            Reply::Miss => FetchOutcome::Miss,
            Reply::Net => FetchOutcome::NetworkError,
        }
    }
}

/// An [`MxResolver`] with a fixed MX answer and per-name scripted SRV answers. An
/// unscripted SRV name resolves clean-empty, so a test only scripts the records it cares
/// about.
pub(crate) struct FakeResolver {
    mx: Option<MxResolution>,
    srv: HashMap<String, Option<SrvResolution>>,
}

impl FakeResolver {
    /// Resolves MX to `records` with the given DNSSEC-authentication flag; SRV names
    /// resolve clean-empty until scripted with [`FakeResolver::srv`].
    pub(crate) fn with(records: Vec<(u16, &str)>, authentic_data: bool) -> Self {
        Self {
            mx: Some(MxResolution {
                records: records
                    .into_iter()
                    .map(|(preference, exchange)| MxRecord {
                        preference,
                        exchange: exchange.to_owned(),
                    })
                    .collect(),
                authentic_data,
            }),
            srv: HashMap::new(),
        }
    }

    /// A resolver whose MX lookup fails (unscripted SRV names still resolve clean-empty).
    pub(crate) fn failing() -> Self {
        Self {
            mx: None,
            srv: HashMap::new(),
        }
    }

    /// Scripts an SRV answer for the owner `name` (e.g. `_jmap._tcp.example.com`); each
    /// tuple is `(priority, weight, port, target)`.
    pub(crate) fn srv(
        mut self,
        name: &str,
        records: Vec<(u16, u16, u16, &str)>,
        authentic_data: bool,
    ) -> Self {
        let records = records
            .into_iter()
            .map(|(priority, weight, port, target)| SrvRecord {
                priority,
                weight,
                port,
                target: target.to_owned(),
            })
            .collect();
        self.srv.insert(
            name.to_owned(),
            Some(SrvResolution {
                records,
                authentic_data,
            }),
        );
        self
    }

    /// Scripts an SRV lookup *failure* for the owner `name`.
    pub(crate) fn srv_failing(mut self, name: &str) -> Self {
        self.srv.insert(name.to_owned(), None);
        self
    }
}

impl MxResolver for FakeResolver {
    fn resolve_mx(&self, _domain: &str) -> Result<MxResolution, MxError> {
        self.mx
            .clone()
            .ok_or_else(|| MxError::Lookup("fake resolver failure".to_owned()))
    }

    fn resolve_srv(&self, name: &str) -> Result<SrvResolution, MxError> {
        match self.srv.get(name) {
            Some(Some(resolution)) => Ok(resolution.clone()),
            Some(None) => Err(MxError::Lookup("fake srv failure".to_owned())),
            None => Ok(SrvResolution {
                records: Vec::new(),
                authentic_data: false,
            }),
        }
    }
}
