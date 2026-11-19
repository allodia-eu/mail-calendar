//! The relay's status from each sign-in and scope state, and the offer in a production build fed
//! from the stored entitlement: with `ai`, through an outage, past grace, on an answer without it
//! and at sign-out. Nothing here reaches the network: the entitlement is recorded as a read would
//! record it, and no token is minted.

use std::sync::{Arc, Mutex, mpsc};

use allodia_license::{Answer, Capability, Entitlement, GRACE_SECONDS, Outcome};
use mailcal_ai::{
    AiError, HttpRequest, HttpResponse, HttpTransport, TransportFailed,
    wire::{ChatMessage, ChatRequest, Purpose},
};
use mailcal_app::ai::{AiFailure, AiHost, AiRelay, AiRoute, BuildKind, RelayStatus};

use super::{AllodiaRelay, relay_status};
use crate::{
    AllodiaGrantHealth, LogLevel, MailcalApp, Surface,
    ai_host::BindingsAiHost,
    allodia::StoredAccount,
    app_ai::tests::every_ai_surface_signalled,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

const NOW: i64 = 1_800_000_000;
const DAY: i64 = 24 * 60 * 60;

fn signed_in(scopes: Option<&[&str]>) -> StoredAccount {
    StoredAccount {
        id: Some("sub-1".to_owned()),
        email: "person@example.eu".to_owned(),
        name: None,
        refresh_token: "refresh".to_owned(),
        granted_scopes: scopes.map(|list| list.iter().map(|s| (*s).to_owned()).collect()),
        end_session_endpoint: None,
    }
}

const WITH_AI: &[&str] = &[
    "offline_access",
    "mailcal:entitlement:read",
    "mailcal:ai:use",
];
const WITHOUT_AI: &[&str] = &["offline_access", "mailcal:entitlement:read"];

#[test]
fn the_relay_is_ready_only_for_a_live_grant_that_may_use_it() {
    let with = signed_in(Some(WITH_AI));
    let without = signed_in(Some(WITHOUT_AI));
    let unrecorded = signed_in(None);
    for (stored, health, expected) in [
        (None, AllodiaGrantHealth::Ok, RelayStatus::SignedOut),
        (Some(&with), AllodiaGrantHealth::Ok, RelayStatus::Ready),
        // A grant short of another feature's scope still reaches the relay.
        (
            Some(&with),
            AllodiaGrantHealth::NeedsReauth,
            RelayStatus::Ready,
        ),
        (
            Some(&with),
            AllodiaGrantHealth::SignedOut,
            RelayStatus::SignInNeeded,
        ),
        (
            Some(&without),
            AllodiaGrantHealth::Ok,
            RelayStatus::SignInNeeded,
        ),
        // Not knowing concludes nothing: the relay's own answer decides.
        (
            Some(&unrecorded),
            AllodiaGrantHealth::Ok,
            RelayStatus::Ready,
        ),
    ] {
        assert_eq!(relay_status(stored, health), expected, "{health:?}");
    }
}

/// The relay's scope is asked of the Writing style surfaces, never of everybody's account card.
#[test]
fn the_relay_s_scope_raises_no_standing_prompt() {
    assert!(!allodia_license::Feature::ALL.contains(&allodia_license::Feature::UseAi));
    assert_eq!(
        crate::allodia_health::health_from_scopes(Some(
            &allodia_license::Feature::ALL
                .iter()
                .map(|feature| feature.scope().to_owned())
                .collect()
        )),
        Some(AllodiaGrantHealth::Ok)
    );
}

fn app(name: &str) -> (Arc<MailcalApp>, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let dir = crate::tests::temp_data_dir(name);
    let store = Arc::new(RecordingCredentialStore::default());
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(store)),
    )
    .expect("the app boots");
    (app, rx)
}

/// The app answering as a production build does: offered only on the entitlement.
fn as_production(app: &MailcalApp) {
    let host = BindingsAiHost::new(
        app.runtime.handle().clone(),
        Arc::clone(&app.credential_store),
    );
    app.app
        .install_ai_host(Arc::new(host) as Arc<dyn AiHost>, BuildKind::Production);
}

fn answer(capabilities: &[Capability]) -> Outcome {
    Outcome::Answered(Answer {
        entitlement: Entitlement {
            plan: "personal".to_owned(),
            capabilities: capabilities.iter().cloned().collect(),
            payment_status: None,
            current_period_end: None,
        },
        refresh_after_seconds: 12 * 60 * 60,
    })
}

fn sign_in(app: &MailcalApp, scopes: Option<&[&str]>) {
    *app.allodia.lock().unwrap() = Some(signed_in(scopes));
}

#[test]
fn a_production_build_offers_it_while_the_stored_answer_carries_ai() {
    let (app, _) = app("ai-offer-production");
    as_production(&app);
    sign_in(&app, Some(WITH_AI));
    app.publish_ai_entitlement_at(NOW);
    assert!(!app.ai_offer(), "nothing stored is the free app");

    app.allodia_entitlement
        .record(answer(&[Capability::AccountsSync, Capability::Ai]), NOW);
    app.publish_ai_entitlement_at(NOW);
    assert!(app.ai_offer());

    // An outage changes nothing, until the grace runs out.
    app.allodia_entitlement
        .record(Outcome::Unreachable, NOW + DAY);
    app.publish_ai_entitlement_at(NOW + DAY);
    assert!(app.ai_offer(), "a stored answer grants through an outage");
    app.publish_ai_entitlement_at(NOW + GRACE_SECONDS - 1);
    assert!(app.ai_offer());
    app.publish_ai_entitlement_at(NOW + GRACE_SECONDS);
    assert!(!app.ai_offer(), "past its grace it grants nothing");
}

#[test]
fn an_answer_without_ai_takes_it_away_at_once() {
    let (app, rx) = app("ai-offer-withdrawn");
    as_production(&app);
    sign_in(&app, Some(WITH_AI));
    app.allodia_entitlement
        .record(answer(&[Capability::Ai]), NOW);
    app.publish_ai_entitlement_at(NOW);
    assert!(app.ai_offer());
    rx.try_iter().for_each(drop);

    app.allodia_entitlement
        .record(answer(&[Capability::AccountsSync]), NOW + 60);
    app.publish_ai_entitlement_at(NOW + 60);
    assert!(!app.ai_offer());
    every_ai_surface_signalled(&rx);
    assert_eq!(app.app.ai_route(), Err(AiFailure::NotEntitled));
}

#[test]
fn signing_out_takes_the_offer_and_the_relay_with_it() {
    let (app, rx) = app("ai-offer-sign-out");
    as_production(&app);
    sign_in(&app, Some(WITH_AI));
    app.allodia_entitlement
        .record(answer(&[Capability::Ai]), NOW);
    app.publish_ai_entitlement_at(NOW);
    assert_eq!(app.app.ai_route(), Ok(AiRoute::Relay));
    rx.try_iter().for_each(drop);

    app.sign_out_of_allodia().expect("signed out");

    assert!(!app.ai_offer());
    every_ai_surface_signalled(&rx);
}

/// A test build is a development build: offered to everyone, and the relay is where a request
/// goes exactly while its status is ready.
#[test]
fn the_route_names_the_relay_or_why_it_cannot_be_asked() {
    let (app, _) = app("ai-relay-route");
    assert_eq!(app.app.ai_route(), Err(AiFailure::NowhereToSend));
    sign_in(&app, Some(WITHOUT_AI));
    assert_eq!(app.app.ai_route(), Err(AiFailure::RelaySignIn));
    sign_in(&app, Some(WITH_AI));
    assert_eq!(app.app.ai_route(), Ok(AiRoute::Relay));
    app.note_allodia_health(AllodiaGrantHealth::SignedOut);
    assert_eq!(app.app.ai_route(), Err(AiFailure::RelaySignIn));
}

/// What the relay's transport was sent: the URL and the bearer.
type Sent = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// Answers every request with one status and body, and keeps what it was sent.
struct Canned(u16, &'static str, Sent);

impl HttpTransport for Canned {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        self.2
            .lock()
            .unwrap()
            .push((request.url.to_owned(), request.bearer.map(str::to_owned)));
        Ok(HttpResponse {
            status: self.0,
            body: self.1.to_owned(),
        })
    }
}

fn held_token() -> mailcal_oauth::TokenSet {
    mailcal_oauth::TokenSet {
        access_token: mailcal_oauth::Secret::new("AT".to_owned()),
        refresh_token: None,
        expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(45),
        scope: String::new(),
        token_type: "Bearer".to_owned(),
        username: None,
    }
}

fn draft_request() -> ChatRequest {
    ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    }
}

/// The relay the core is handed posts to the build's own account service, with the signed-in
/// account's token; a refusal of that token drops it, so the next request mints another.
#[test]
fn the_backend_posts_to_the_account_service_with_the_account_s_token() {
    let (app, _) = app("ai-relay-backend");
    sign_in(&app, Some(WITH_AI));
    app.allodia_tokens.hold(held_token());
    let relay = AllodiaRelay {
        app: Arc::downgrade(&app),
    };
    let sent = Sent::default();
    let backend = relay.backend(Box::new(Canned(
        200,
        r#"{"choices":[],"allodia":{"credits_charged":1,"balance_credits":9}}"#,
        Arc::clone(&sent),
    )));
    backend.chat(&draft_request()).expect("answered");
    assert_eq!(
        *sent.lock().unwrap(),
        [(
            format!(
                "{}/api/v1/ai/chat",
                allodia_license::host().trim_end_matches('/')
            ),
            Some("AT".to_owned())
        )]
    );

    let refused = relay.backend(Box::new(Canned(401, "", Arc::clone(&sent))));
    assert_eq!(
        refused.chat(&draft_request()).unwrap_err(),
        AiError::Unauthorized
    );
    assert_eq!(app.allodia_tokens.held(), None);
}

/// A grant recorded without the relay's scope fails before a token is minted or a byte is sent.
#[test]
fn a_grant_without_the_scope_is_a_sign_in_before_anything_is_sent() {
    let (app, _) = app("ai-relay-token");
    sign_in(&app, Some(WITHOUT_AI));
    app.allodia_tokens.hold(held_token());
    let sent = Sent::default();
    let backend = AllodiaRelay {
        app: Arc::downgrade(&app),
    }
    .backend(Box::new(Canned(200, "{}", Arc::clone(&sent))));
    assert_eq!(
        backend.chat(&draft_request()).unwrap_err(),
        AiError::Unauthorized
    );
    assert!(sent.lock().unwrap().is_empty());
}
