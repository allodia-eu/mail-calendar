//! Mock token endpoints for [`super::GraphTokenSource`]'s tests and the sibling provider tests.
//!
//! The one that matters is [`ratcheting_token_endpoint`]: it models an authorization server that
//! rotates the refresh token on every refresh **and treats a replay as theft**, revoking the
//! grant. That is Fastmail's behaviour, and it is the only kind of server on which a concurrent
//! double-refresh is fatal rather than merely wasteful: so it is the only kind that can catch
//! the regression this module exists to guard.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use engine_core::ids::AccountId;
use mailcal_oauth::{OAuthClient, OAuthProviderConfig};

use super::{GraphTokenSource, TokenSink};
use crate::http_fake::{self, Reply, Seen};

/// A mock token endpoint that models a **rotating, ratcheting** authorization server;
/// Fastmail's behaviour, and the one that turns a harmless-looking race into a dead
/// account.
///
/// It holds exactly one currently-valid refresh token. A refresh presenting it rotates to
/// the next one and succeeds; a refresh presenting **any** other value; including the one
/// that was valid a moment ago, is a replay, and the server answers `invalid_grant` *and
/// revokes the grant for good*. That last part is what makes a concurrent double-refresh
/// unrecoverable rather than merely wasteful: the second request does not just fail, it
/// takes the account with it.
///
/// Returns the endpoint URL and the count of refresh requests it saw.
pub(crate) fn ratcheting_token_endpoint(initial: &str) -> (String, Arc<AtomicUsize>) {
    let mut valid = initial.to_owned();
    let mut generation = 0usize;
    token_endpoint(move |request| {
        let presented = form_value(&request.body, "refresh_token").unwrap_or_default();
        // The ratchet: anything but the live token revokes the grant outright, so every later
        // refresh fails too; exactly like the real server.
        if presented == valid && !valid.is_empty() {
            generation += 1;
            valid = format!("rotated-{generation}");
            Reply::Json(format!(
                r#"{{"token_type":"Bearer","expires_in":3600,"access_token":"AT-{generation}","refresh_token":"{valid}"}}"#
            ))
        } else {
            valid.clear();
            Reply::Status(
                "400 Bad Request",
                r#"{"error":"invalid_grant","error_description":"ratchet or client_id mismatch"}"#
                    .to_owned(),
            )
        }
    })
}

/// A token endpoint that refuses every refresh with `invalid_grant`, and counts how many it was
/// asked. Unlike the one-shot listener the older `an_invalid_grant_refresh_is_a_reauth_signal`
/// builds inline, this keeps answering, which is the only way to tell "we stopped asking" from
/// "there was nobody left to ask".
pub(crate) fn dead_grant_token_endpoint() -> (String, Arc<AtomicUsize>) {
    token_endpoint(|_| {
        Reply::Status(
            "400 Bad Request",
            r#"{"error":"invalid_grant","error_description":"revoked"}"#.to_owned(),
        )
    })
}

/// One `application/x-www-form-urlencoded` value out of a request body.
fn form_value(body: &str, key: &str) -> Option<String> {
    body.split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}

/// A token endpoint at `/token` of the crate's test server, answering as `answer` decides, and
/// the count of requests it saw.
pub(crate) fn token_endpoint(
    mut answer: impl FnMut(&Seen) -> Reply + Send + 'static,
) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let (base, _) = http_fake::serve_with(move |request| {
        counter.fetch_add(1, Ordering::SeqCst);
        answer(request)
    });
    (format!("{base}/token"), hits)
}

/// A mock token endpoint that **accepts** the request and then hangs up without answering,
/// for the first `failures` requests, and serves a valid token response after that.
///
/// The hang-up is the point. A refused connection fails before a byte is written and is
/// provably safe to retry; this one fails *after* the request was delivered, which is
/// indistinguishable from a server that processed the refresh and lost the answer. It is the
/// shape that must never be retried in a tight loop, and, because the request really does
/// arrive; it is the only shape whose retries a test can count.
///
/// Returns the endpoint URL and the count of requests it saw.
pub(crate) fn flaky_token_endpoint(failures: usize) -> (String, Arc<AtomicUsize>) {
    let mut seen = 0;
    token_endpoint(move |_| {
        seen += 1;
        if seen <= failures {
            // Dropped mid-conversation: the request landed, no response comes back.
            Reply::HangUp
        } else {
            Reply::Json(
                r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT-OK"}"#.to_owned(),
            )
        }
    })
}

/// A token endpoint URL that nothing is listening on, so a connection to it is refused
/// before any byte of the request is written.
///
/// Port 1, never a port a listener was bound to and dropped: the OS hands an ephemeral port it
/// has taken back to the next bind, so another test's server running at the same time would
/// answer this endpoint's requests and lose its own replies to them. No test binds port 1, and
/// the OS never hands it out for a bind to port 0.
pub(crate) fn refused_token_endpoint() -> String {
    "http://127.0.0.1:1/token".to_owned()
}

/// A mock token endpoint that serves each queued response once, counting hits: so a
/// test can prove the second `access_token()` was served from cache (no second hit).
pub(crate) fn mock_token_endpoint(responses: Vec<String>) -> (String, Arc<AtomicUsize>) {
    let mut responses = responses.into_iter();
    token_endpoint(move |_| Reply::Json(responses.next().unwrap_or_default()))
}

/// What an OAuth IMAP dial meets when the token endpoint at `endpoint` gives no token, as the
/// engine hands it on: dialled through the real connect, which asks for the credential before it
/// opens anything.
pub(crate) async fn imap_dial_without_token(endpoint: String) -> engine_provider::ProviderError {
    use engine_provider::CredentialSource;

    let config = provider_imap::ImapConfig::from_credential_source(
        "127.0.0.1:1",
        "127.0.0.1",
        source_at(endpoint, None) as Arc<dyn CredentialSource>,
    );
    let tls = crate::tls::tls_with(&[]).unwrap();
    match provider_imap::ImapAccount::connect(&config, tls.connector()).await {
        Ok(_) => panic!("no token, so no sign-in"),
        Err(err) => err.into(),
    }
}

/// A **distinct** account for every test source.
///
/// Not cosmetic. Token state is now shared per account across a process, so two sources built with
/// the same id share one state, which is the property under test in one place and a hidden
/// coupling between unrelated tests everywhere else. They all used
/// `alice@example.com@graph.microsoft.com`, so a failure memo left by one would have leaked into
/// the next: green or red depending on which order the harness happened to run them in.
fn unique_account() -> AccountId {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    AccountId::try_from(format!("alice+{n}@example.com@graph.microsoft.com").as_str())
        .expect("a valid account id")
}

/// The name every test source signs in as.
pub(crate) const SIGN_IN_NAME: &str = "you@example.net";

pub(crate) fn source_at(
    token_endpoint: String,
    sink: Option<Arc<dyn TokenSink>>,
) -> Arc<GraphTokenSource> {
    source_for(unique_account(), token_endpoint, sink)
}

/// A source over a **named** account, for the one test that needs two sources to be the same
/// account; i.e. two cores in one process, which is what
/// [`CredentialOrigin`](crate::CredentialOrigin) exists for.
pub(crate) fn source_for(
    account: AccountId,
    token_endpoint: String,
    sink: Option<Arc<dyn TokenSink>>,
) -> Arc<GraphTokenSource> {
    let provider = OAuthProviderConfig {
        authorize_endpoint: "https://example/authorize".to_owned(),
        token_endpoint,
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://oauth".to_owned(),
        expected_issuer: None,
        scopes: vec!["offline_access".to_owned()],
        resource: None,
        style: mailcal_oauth::AuthStyle::Microsoft,
        authorization_resources: Vec::new(),
    };
    GraphTokenSource::from_parts(
        OAuthClient::new(provider).unwrap(),
        account,
        SIGN_IN_NAME,
        "initial-refresh".to_owned(),
        sink,
        "graph",
        crate::CredentialOrigin::Stored,
    )
}

/// Asserts that `provider`, built over a token source whose grant is dead, takes both draft verbs
/// to its delegate rather than answering them with the trait's rejecting default.
///
/// The two are told apart by their error: reaching the delegate means minting a token first,
/// which the dead grant refuses as an authentication failure, while the default answers
/// `InvalidState` without asking anyone. So the test needs no mail server, and still fails on
/// exactly the wrapper that advertises drafts and does not forward them.
pub(crate) async fn assert_drafts_reach_the_delegate(provider: &dyn engine_provider::Provider) {
    use engine_core::{
        error::FailureClass,
        ids::MessageIdHeader,
        mail::{EmailAddress, MessageMetadata},
    };
    use engine_provider::Draft;

    assert!(
        provider.connection_info().capabilities.mail_drafts(),
        "drafts are forwarded, so they must be advertised"
    );
    let account = unique_account();
    let draft = Draft::new(
        MessageIdHeader::new("draft@example.com").expect("valid message id"),
        EmailAddress::new("from@example.com"),
        vec![EmailAddress::new("to@example.com")],
        "Subject",
        "Body",
    );
    let key = engine_core::ids::ProviderKey::new("draft-1").expect("valid key");

    let saved = provider
        .put_draft(&account, &draft, None, &MessageMetadata::default())
        .await
        .unwrap_err();
    assert_eq!(saved.class(), FailureClass::Authentication, "{saved}");
    let removed = provider.delete_draft(&account, &key).await.unwrap_err();
    assert_eq!(removed.class(), FailureClass::Authentication, "{removed}");
}

/// An event in the calendar `calendar`, and an acceptance of it.
pub(crate) fn an_invitation() -> (engine_core::calendar::Event, engine_provider::EventRsvp) {
    use engine_core::{
        calendar::Event,
        ids::{CalendarId, EventId, Uid},
        membership::Memberships,
        time::CalendarDateTime,
    };
    use engine_provider::{EventRsvp, RsvpResponse};

    let event = Event::new(
        EventId::try_from("calendar/event-1").expect("valid event id"),
        Uid::new("event-1@example.com").expect("valid uid"),
        Memberships::of_one(CalendarId::try_from("calendar").expect("valid calendar id")),
        CalendarDateTime::Floating("2026-09-01T09:00:00".parse().expect("valid time")),
    );
    let rsvp = EventRsvp::to(&event, "mailto:me@example.com", RsvpResponse::Accepted);
    (event, rsvp)
}

/// Asserts that `provider`, built over a token source whose grant is dead, takes the calendar
/// and event fetches, an invitation answer and the calendar-user lookup to its delegate, the way
/// [`assert_drafts_reach_the_delegate`] does for drafts. The defaults answer without asking
/// anyone: the fetches and `rsvp_event` refuse as unsupported, so a calendar wrapper left with
/// an empty `CalendarSync` impl syncs no calendar at all, and `calendar_user_addresses` reports
/// `Unknown`.
pub(crate) async fn assert_calendar_reaches_the_delegate(provider: &dyn engine_provider::Provider) {
    use engine_core::{
        error::FailureClass,
        ids::CalendarId,
        time::{Horizon, UtcDateTime},
    };

    let account = unique_account();
    let (event, rsvp) = an_invitation();
    let calendar = CalendarId::try_from("calendar").expect("valid calendar id");
    let window = Horizon::new(
        UtcDateTime::new(2026, 8, 1, 0, 0, 0).expect("valid start"),
        UtcDateTime::new(2026, 10, 1, 0, 0, 0).expect("valid end"),
    )
    .expect("valid window");

    let listed = provider.sync_calendars(&account, None).await.unwrap_err();
    assert_eq!(listed.class(), FailureClass::Authentication, "{listed}");
    let fetched = provider
        .sync_events(&account, &calendar, None, window)
        .await
        .unwrap_err();
    assert_eq!(fetched.class(), FailureClass::Authentication, "{fetched}");
    let answered = provider
        .rsvp_event(&account, &event, &rsvp)
        .await
        .unwrap_err();
    assert_eq!(answered.class(), FailureClass::Authentication, "{answered}");
    let addresses = provider.calendar_user_addresses(&account).await;
    assert!(
        addresses
            .as_ref()
            .is_err_and(|err| err.class() == FailureClass::Authentication),
        "{addresses:?}"
    );
}
