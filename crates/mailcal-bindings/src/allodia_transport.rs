//! The HTTPS half of the Allodia account service port.
//!
//! `allodia-license` opens no socket of its own; it takes a
//! [`Transport`](allodia_license::Transport), and this crate is where that is implemented,
//! because this crate is the one allowed to reach the network. What it costs to get wrong is
//! specific: `reqwest` is pinned to `rustls-no-provider` workspace-wide, so a client built any way
//! but through the shared TLS policy has **no crypto provider** and dies at the first request. The
//! client here comes from [`mailcal_oauth::discovery_client`], which is the same one the sign-in
//! flow and every other discovered OAuth path already run on.
//!
//! **The port is synchronous and the client is not.** That is not an oversight in either: the
//! account-service calls are a handful of round trips a person is waiting on, so the pass reads
//! straight down the page rather than through a state machine, and the futures are driven on the
//! app's own runtime rather than on a second one.

use allodia_license::{Method, Request, Response, Transport};

/// The account service over the app's own TLS policy and runtime.
pub(crate) struct HttpsTransport {
    http: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for HttpsTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HttpsTransport")
    }
}

impl HttpsTransport {
    /// Build one on `handle`'s runtime.
    ///
    /// # Errors
    ///
    /// Returns the shared TLS policy's own message when a client cannot be built, which is the
    /// same failure every OAuth path in the app reports.
    pub(crate) fn new(handle: tokio::runtime::Handle) -> Result<Self, String> {
        let http = mailcal_oauth::discovery_client().map_err(|error| error.to_string())?;
        Ok(Self { http, handle })
    }

    /// The request, as `reqwest` wants it.
    fn build(&self, request: &Request) -> reqwest::RequestBuilder {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self
            .http
            .request(method, &request.url)
            .bearer_auth(&request.bearer)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = &request.body {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
        }
        if let Some(key) = &request.idempotency_key {
            builder = builder.header("Idempotency-Key", key.clone());
        }
        builder
    }
}

impl Transport for HttpsTransport {
    fn send(&self, request: &Request) -> Result<Response, String> {
        let call = self.build(request).send();
        let answered = block_on(&self.handle, async {
            let response = call.await.map_err(|error| error.to_string())?;
            let status = response.status().as_u16();
            // The body is read whatever the status: the service's `409` carries the record this
            // device is disagreeing with, and its `4xx` bodies carry the reason. Reading only on
            // success would throw away the half that decides what happens next.
            let body = response.text().await.map_err(|error| error.to_string())?;
            Ok::<_, String>(Response { status, body })
        })?;
        Ok(answered)
    }
}

/// Drive one future to completion on the app's runtime, from a synchronous caller.
///
/// A pass runs on the thread that asked for it (a host's background thread) where handing the
/// future to the runtime is all there is to it. A pass started *from* the runtime, which a
/// scheduled one would be, is on a worker instead, and parking that worker is what
/// [`block_in_place`](tokio::task::block_in_place) exists to avoid: it moves the thread out of the
/// scheduler first, so the remaining work still has somewhere to run.
pub(crate) fn block_on<T>(handle: &tokio::runtime::Handle, future: impl Future<Output = T>) -> T {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        handle.block_on(future)
    }
}

#[cfg(test)]
mod tests {
    use allodia_license::{Method, Request};

    use super::HttpsTransport;

    fn transport() -> (tokio::runtime::Runtime, HttpsTransport) {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let transport = HttpsTransport::new(runtime.handle().clone()).expect("a TLS client");
        (runtime, transport)
    }

    /// The pin that matters most here, and the one a unit test can actually hold: a client built
    /// off the shared policy has a crypto provider. Built any other way it would construct fine
    /// and panic at the first request, several layers from anything that names TLS.
    #[test]
    fn the_client_is_built_from_the_shared_tls_policy() {
        let (_runtime, _transport) = transport();
    }

    /// Every field of the port reaches the wire. Asserted on the built request rather than on a
    /// served one, because what has been wrong here before is a header that was never attached;
    /// the idempotency key above all, whose absence is invisible until a retry duplicates an
    /// account.
    #[test]
    fn a_write_carries_its_body_its_bearer_and_its_idempotency_key() {
        let (_runtime, transport) = transport();
        let request = Request {
            url: "https://mailcal.example.com/api/v1/accounts".to_owned(),
            bearer: "an-access-token".to_owned(),
            method: Method::Post,
            body: Some(r#"{"config":{}}"#.to_owned()),
            idempotency_key: Some("attempt-1".to_owned()),
        };
        let built = transport
            .build(&request)
            .build()
            .expect("the request is well formed");

        assert_eq!(built.method(), reqwest::Method::POST);
        assert_eq!(built.url().as_str(), request.url);
        let headers = built.headers();
        assert_eq!(headers["authorization"], "Bearer an-access-token");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["idempotency-key"], "attempt-1");
        assert!(built.body().is_some());
    }

    /// A `GET` sends no body and no idempotency key. The first is refused by enough intermediaries
    /// to be worth never producing; the second would be meaningless on a read.
    #[test]
    fn a_read_sends_nothing_it_was_not_given() {
        let (_runtime, transport) = transport();
        let built = transport
            .build(&Request::get(
                "https://mailcal.example.com/api/v1/accounts",
                "an-access-token",
            ))
            .build()
            .expect("the request is well formed");

        assert_eq!(built.method(), reqwest::Method::GET);
        assert!(built.body().is_none());
        assert!(!built.headers().contains_key("idempotency-key"));
        assert!(!built.headers().contains_key("content-type"));
    }
}
