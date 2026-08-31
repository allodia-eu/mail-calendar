//! Every case here is something that can be wrong while the app still looks like it is working: a
//! capability granted on a lapsed plan, an unknown label read as permission, an outage that takes
//! away what someone paid for, a cancellation honoured a month late.

use std::cell::RefCell;

use super::{
    AccountService, Answer, Cache, Capability, Entitlement, Error, GRACE_SECONDS, Outcome, Request,
    Response, Stored, Transport,
};

const DAY: i64 = 24 * 60 * 60;
const NOW: i64 = 1_800_000_000;

/// A transport that answers from a script and records what it was asked.
struct Fake {
    answers: RefCell<Vec<Result<Response, String>>>,
    seen: RefCell<Vec<Request>>,
}

impl Fake {
    fn ok(status: u16, body: &str) -> Self {
        Self {
            answers: RefCell::new(vec![Ok(Response {
                status,
                body: body.to_owned(),
            })]),
            seen: RefCell::new(Vec::new()),
        }
    }

    fn dead(reason: &str) -> Self {
        Self {
            answers: RefCell::new(vec![Err(reason.to_owned())]),
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl Transport for Fake {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.borrow_mut().push(request.clone());
        self.answers
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| panic!("no scripted answer for {}", request.url))
    }
}

const ACTIVE: &str = r#"{"plan":"personal","active":true,
    "capabilities":["push","send_later"],
    "currentPeriodEnd":"2026-09-24T00:00:00.000Z","refreshAfterSeconds":43200}"#;

fn paid() -> Answer {
    Answer {
        entitlement: Entitlement {
            plan: "personal".to_owned(),
            active: true,
            capabilities: [Capability::Push].into_iter().collect(),
            current_period_end: None,
        },
        refresh_after_seconds: 12 * 60 * 60,
    }
}

fn lapsed() -> Answer {
    Answer {
        entitlement: Entitlement::free(),
        refresh_after_seconds: 12 * 60 * 60,
    }
}

// --- Asking -------------------------------------------------------------------------------------

#[test]
fn an_entitlement_is_read_with_its_capabilities() {
    let fake = Fake::ok(200, ACTIVE);
    let service = AccountService::new("https://allodia.example/");
    let answer = service.entitlement(&fake, "tok_abc").unwrap();

    assert!(answer.entitlement.active);
    assert_eq!(answer.entitlement.plan, "personal");
    assert_eq!(answer.refresh_after_seconds, 43_200);
    assert!(answer.entitlement.grants(&Capability::Push));
    assert!(answer.entitlement.grants(&Capability::SendLater));
    // Not in the list, so not granted -- absence is the answer, never a default.
    assert!(!answer.entitlement.grants(&Capability::CentralAdmin));

    let seen = fake.seen.borrow();
    // The trailing slash in the base URL must not become a double slash in the path.
    assert_eq!(seen[0].url, "https://allodia.example/api/v1/entitlement");
    assert_eq!(seen[0].bearer, "tok_abc");
}

#[test]
fn a_refused_token_is_reported_as_such_and_not_as_an_outage() {
    // The caller acts on these differently: one is a refresh, the other is "keep what you have".
    let fake = Fake::ok(401, r#"{"message":"expired"}"#);
    let service = AccountService::new("https://allodia.example");
    assert_eq!(
        service.entitlement(&fake, "tok_old").unwrap_err(),
        Error::Unauthorized
    );
}

#[test]
fn an_unreachable_service_is_a_transport_error_carrying_the_reason() {
    let fake = Fake::dead("dns: no such host");
    let service = AccountService::new("https://allodia.example");
    assert_eq!(
        service.entitlement(&fake, "tok_abc").unwrap_err(),
        Error::Transport("dns: no such host".to_owned())
    );
}

#[test]
fn a_body_this_version_cannot_read_is_malformed_rather_than_a_panic() {
    let fake = Fake::ok(200, "<html>maintenance</html>");
    let service = AccountService::new("https://allodia.example");
    let error = service.entitlement(&fake, "tok_abc").unwrap_err();
    assert!(matches!(error, Error::Malformed(_)), "{error:?}");
}

#[test]
fn a_capability_on_an_inactive_plan_grants_nothing() {
    // The service already refuses to list capabilities for a lapsed plan. This is the same rule on
    // the side that draws the UI: a client must never grant on the list alone.
    let fake = Fake::ok(
        200,
        r#"{"plan":"personal","active":false,"capabilities":["push"],"refreshAfterSeconds":43200}"#,
    );
    let service = AccountService::new("https://allodia.example");
    let answer = service.entitlement(&fake, "tok_abc").unwrap();

    assert!(!answer.entitlement.active);
    assert!(!answer.entitlement.grants(&Capability::Push));
}

#[test]
fn an_unknown_capability_is_kept_but_never_granted() {
    // A client older than a capability has to keep working. It reads the label, does not draw it,
    // and above all does not treat "I do not recognise this" as permission.
    let fake = Fake::ok(
        200,
        r#"{"plan":"business","active":true,"capabilities":["push","time_travel"],"refreshAfterSeconds":1}"#,
    );
    let service = AccountService::new("https://allodia.example");
    let answer = service.entitlement(&fake, "tok_abc").unwrap();

    assert!(answer.entitlement.grants(&Capability::Push));
    assert!(
        answer
            .entitlement
            .capabilities
            .contains(&Capability::Unknown("time_travel".to_owned()))
    );
    assert!(
        !answer
            .entitlement
            .grants(&Capability::Unknown("anything_else".to_owned()))
    );
}

// --- What to draw between answers
// -----------------------------------------------------------------

#[test]
fn nothing_stored_is_the_free_app_and_asks_at_once() {
    let cache = Cache::default();
    assert_eq!(cache.effective(NOW), Entitlement::free());
    assert!(cache.should_refresh(NOW));
    assert!(cache.stored().is_none());
}

#[test]
fn an_outage_does_not_take_away_what_someone_paid_for() {
    // The failure this prevents is the one nobody reports: Allodia has a bad afternoon and every
    // paying customer quietly loses their capabilities.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    for days in [0, 1, 7, 29] {
        let later = NOW + days * DAY;
        cache.apply(Outcome::Unreachable, later);
        assert!(
            cache.effective(later).grants(&Capability::Push),
            "lost the capability after {days} days of outage"
        );
    }
}

#[test]
fn grace_does_run_out() {
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    let just_inside = NOW + GRACE_SECONDS - 1;
    assert!(cache.effective(just_inside).grants(&Capability::Push));

    let past = NOW + GRACE_SECONDS;
    assert_eq!(cache.effective(past), Entitlement::free());
    assert!(!cache.effective(past).grants(&Capability::Push));
}

#[test]
fn a_cancellation_takes_effect_at_once_rather_than_after_grace() {
    // The other half of the rule above, and the one that is easy to get backwards: an answer
    // replaces the stored one whatever it says. Treating a `no` like an outage would honour a
    // cancellation a month late.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);
    assert!(cache.effective(NOW).grants(&Capability::Push));

    let next_day = NOW + DAY;
    cache.apply(Outcome::Answered(lapsed()), next_day);
    assert!(!cache.effective(next_day).grants(&Capability::Push));
    assert_eq!(cache.effective(next_day), Entitlement::free());
}

#[test]
fn it_asks_again_on_the_services_own_interval() {
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    assert!(!cache.should_refresh(NOW));
    assert!(!cache.should_refresh(NOW + 12 * 60 * 60 - 1));
    assert!(cache.should_refresh(NOW + 12 * 60 * 60));
}

#[test]
fn a_failed_attempt_does_not_reset_the_interval() {
    // Otherwise an unreachable service pushes the next attempt further away every time it is
    // tried, and a client that has been offline for a week does not ask when it comes back.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);
    let due = NOW + 12 * 60 * 60;

    cache.apply(Outcome::Unreachable, due);
    assert!(cache.should_refresh(due));
    assert_eq!(cache.stored().unwrap().fetched_at, NOW);
}

#[test]
fn a_stored_answer_survives_a_round_trip_through_the_host() {
    // The host persists this beside the store and hands it back at the next launch.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    let json = serde_json::to_string(cache.stored().unwrap()).unwrap();
    let restored: Stored = serde_json::from_str(&json).unwrap();
    let cache = Cache::restore(Some(restored));

    assert!(cache.effective(NOW + DAY).grants(&Capability::Push));
}

#[test]
fn a_clock_that_went_backwards_does_not_grant_forever_or_panic() {
    // Device clocks move. A stored answer from "the future" must not underflow the subtraction or
    // read as infinitely fresh.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    let earlier = NOW - 400 * DAY;
    assert!(cache.effective(earlier).grants(&Capability::Push));
    assert!(!cache.should_refresh(earlier));
}

// --- The sovereignty carve-out's own premises
// -----------------------------------------------------

#[test]
fn the_shipped_destination_is_allodias_own_and_is_reached_over_tls() {
    // The carve-out in entitlement.md rests on one address chosen at build time with no runtime
    // path to it -- not on the address being a literal. A development build points somewhere else,
    // which is why this asserts the *default*: that is what every build that injects nothing gets,
    // and every shipped build with it.
    let default = mailcal_oauth::credentials::DEFAULT_ALLODIA_HOST;
    assert_eq!(default, "https://mailcal.allodia.eu");
    assert!(default.starts_with("https://"));
    assert!(
        !default.ends_with('/'),
        "a trailing slash doubles up in every URL built from it"
    );
}

#[test]
fn sign_in_asks_for_a_refresh_token_and_for_nothing_that_reaches_mail() {
    // `offline_access` is the load-bearing one: without it the service issues no refresh token and
    // the sign-in becomes a session that expires with no way back, which is the problem OAuth was
    // chosen to solve. What is actually sent is the intersection with what the service advertises
    // (`scopes_for`); this is the full set a build can ask for.
    assert_eq!(
        crate::SCOPES,
        [
            "openid",
            "profile",
            "email",
            "offline_access",
            "mailcal:entitlement:read",
            "mailcal:accounts:read",
            "mailcal:accounts:write",
        ]
    );
    assert!(crate::SCOPES.contains(&"offline_access"));
    // **The entitlement is read, and only read.** A scope that could change a plan from the device
    // would put the enforcement point back on the client, which entitlement.md says it never is.
    // The account list is a different thing entirely: it is the person's own settings, written
    // from their own devices, and it says nothing about what they are entitled to.
    assert!(
        crate::SCOPES
            .iter()
            .filter(|scope| scope.contains("entitlement"))
            .all(|scope| !scope.contains("write")),
        "no scope may let a device write its own entitlement"
    );
    // Nothing here reaches mail. An Allodia account and a mail account are different things, and a
    // token issued for this app cannot touch the second.
    assert!(
        crate::SCOPES
            .iter()
            .all(|scope| !scope.contains("mail:") && !scope.contains("message")),
    );
}

#[test]
fn the_surface_is_offered_exactly_when_a_registration_was_injected() {
    // A build from source carries none and offers no sign-in, which is why the surface is absent
    // rather than present-and-broken. Asserting the *absence* would have been a test that passes
    // only on a machine without an `.env` -- green in CI, red for whoever has credentials, and
    // proving nothing either way.
    assert_eq!(
        crate::available(),
        mailcal_oauth::credentials::allodia_client_id().is_some()
    );
}

#[test]
fn the_redirect_label_collides_with_nothing_already_dispatched() {
    // Windows and Android route a callback by this label. `auth` is Microsoft's and `jmap-oauth`
    // is JMAP's, so reusing either would deliver an Allodia callback to the wrong handler -- which
    // fails as a mismatched `state` rather than as anything that names the real cause.
    assert_eq!(crate::REDIRECT_HOST, "account-oauth");
    assert_ne!(crate::REDIRECT_HOST, "auth");
    assert_ne!(crate::REDIRECT_HOST, "jmap-oauth");
}
