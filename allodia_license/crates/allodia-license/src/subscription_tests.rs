use super::read;
use crate::{
    AccountService, Biller, Error, OwnStatus, Plan, Refusal, Request, Response, Store, StoreStatus,
    Transport,
};

/// A transport that answers with one status and body.
struct Canned {
    status: u16,
    body: String,
    seen: std::sync::Mutex<Vec<Request>>,
}

impl Canned {
    fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.to_owned(),
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn only_request(&self) -> Request {
        let seen = self.seen.lock().expect("the recorded requests");
        assert_eq!(seen.len(), 1, "exactly one request");
        seen[0].clone()
    }
}

impl Transport for Canned {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen
            .lock()
            .expect("the recorded requests")
            .push(request.clone());
        Ok(Response {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

const FREE: &str = r#"{
    "entitled": false, "plan": "free", "currentPeriodEnd": null,
    "subscription": null, "storeSubscriptions": [], "duplicateBilling": [],
    "actions": {"canCancel": false, "canResubscribe": false, "canSwitchInterval": false,
                "canStartCheckout": true},
    "prices": {"monthly": 199, "yearly": 2000, "currency": "EUR"},
    "checkoutAvailable": true}"#;

fn refusal(code: &str) -> String {
    format!(
        r#"{{"defined":true,"code":"CONFLICT","status":409,"message":"nope","data":{{"code":"{code}"}}}}"#
    )
}

/// Somebody who has never paid: no subscription anywhere, and the one thing they may do is start
/// a checkout.
#[test]
fn a_free_account_reads_as_free_and_may_start_a_checkout() {
    let subscription = read(FREE).expect("the service answered");

    assert!(!subscription.entitled);
    assert_eq!(subscription.plan, "free");
    assert!(subscription.own.is_none());
    assert!(subscription.stores.is_empty());
    assert!(subscription.duplicate_billing.is_empty());
    assert!(subscription.actions.can_start_checkout);
    assert!(!subscription.actions.can_cancel);
}

/// The prices are minor units and a currency, because the core carries no locale data and cannot
/// format one. Turning 199 and `EUR` into something somebody reads is the platform's job.
#[test]
fn the_list_prices_arrive_as_minor_units_and_a_currency() {
    let subscription = read(FREE).expect("the service answered");

    assert_eq!(subscription.prices.of(Plan::Monthly), 199);
    assert_eq!(subscription.prices.of(Plan::Yearly), 2_000);
    assert_eq!(subscription.prices.currency, "EUR");
}

/// A store subscription is read only, and what a client offers instead of a cancel button is the
/// store's own page. Sending somebody to Allodia's page is how a cancellation quietly does not
/// happen.
#[test]
fn a_store_subscription_carries_the_page_that_can_actually_change_it() {
    let body = r#"{
        "entitled": true, "plan": "personal", "currentPeriodEnd": "2026-10-01T00:00:00.000Z",
        "subscription": null,
        "storeSubscriptions": [{"source":"apple","status":"active","interval":"yearly",
            "productId":"eu.allodia.mailcal.services.yearly","priceInCents":2399,"currency":"EUR",
            "currentPeriodEnd":"2026-10-01T00:00:00.000Z","autoRenewing":true,
            "environment":"production","manageUrl":"https://apps.apple.com/account/subscriptions"}],
        "duplicateBilling": [],
        "actions": {"canCancel": false, "canResubscribe": false, "canSwitchInterval": false,
                    "canStartCheckout": false},
        "prices": {"monthly": 199, "yearly": 2000, "currency": "EUR"},
        "checkoutAvailable": true}"#;

    let subscription = read(body).expect("the service answered");
    let store = &subscription.stores[0];

    assert_eq!(store.source, Store::Apple);
    assert_eq!(store.status, StoreStatus::Active);
    assert_eq!(store.interval, Plan::Yearly);
    assert_eq!(
        store.manage_url,
        "https://apps.apple.com/account/subscriptions"
    );
    // The store's own local price, not Allodia's euro one: quoting ours would be a figure the
    // subscriber can disprove from their receipt.
    assert_eq!(store.price_in_cents, Some(2_399));
    // And nothing here may be cancelled through this API.
    assert!(!subscription.actions.can_cancel);
    assert!(!subscription.actions.can_start_checkout);
}

/// Google Play reports no price. `None` is the ordinary answer there, not a gap to fill in with
/// Allodia's own figure.
#[test]
fn a_play_subscription_with_no_price_is_read_as_having_none() {
    let body = FREE.replace(
        r#""storeSubscriptions": [],"#,
        r#""storeSubscriptions": [{"source":"google","status":"grace","interval":"monthly",
            "productId":null,"priceInCents":null,"currency":null,
            "currentPeriodEnd":null,"autoRenewing":true,"environment":"production",
            "manageUrl":"https://play.google.com/store/account/subscriptions"}],"#,
    );

    let subscription = read(&body).expect("the service answered");
    let store = &subscription.stores[0];

    assert_eq!(store.source, Store::Google);
    // A failed charge the store is still retrying: access continues, so this is not a lapse.
    assert_eq!(store.status, StoreStatus::Grace);
    assert!(store.price_in_cents.is_none());
    assert!(store.currency.is_none());
}

/// Two sources charging at once. The service reports it rather than resolving it, and `mollie`
/// becomes **Allodia**: naming a payment processor the person has never heard of, in a message
/// about being charged twice, is how a correct warning reads as a scam.
#[test]
fn being_charged_twice_is_reported_and_allodias_own_billing_is_named_allodia() {
    let body = FREE.replace(
        r#""duplicateBilling": [],"#,
        r#""duplicateBilling": ["mollie","apple"],"#,
    );

    let subscription = read(&body).expect("the service answered");

    assert_eq!(
        subscription.duplicate_billing,
        vec![Biller::Allodia, Biller::Apple]
    );
}

/// A biller this version cannot name is kept, so a client can still say that more than one thing
/// is charging even when it cannot name them all.
#[test]
fn a_biller_this_version_does_not_know_is_kept_rather_than_dropped() {
    let body = FREE.replace(
        r#""duplicateBilling": [],"#,
        r#""duplicateBilling": ["apple","some_later_biller"],"#,
    );

    let subscription = read(&body).expect("the service answered");

    assert_eq!(subscription.duplicate_billing.len(), 2);
    assert!(matches!(
        subscription.duplicate_billing[1],
        Biller::Unknown(ref label) if label == "some_later_biller"
    ));
}

/// A status this version does not know is kept and never read as permission, which is the rule an
/// unrecognised capability label already follows.
#[test]
fn a_status_this_version_does_not_know_is_kept_and_grants_nothing() {
    let body = FREE.replace(
        r#""subscription": null,"#,
        r#""subscription": {"status":"some_later_state","interval":"monthly","amountInCents":199,
            "nextPaymentDate":null,"currentPeriodEnd":null,"cancelledAt":null,"createdAt":"x"},"#,
    );

    let subscription = read(&body).expect("the service answered");
    let own = subscription.own.expect("an own subscription");

    assert!(matches!(own.status, OwnStatus::Unknown(ref label) if label == "some_later_state"));
    // What decides anything is `entitled` and `actions`, both of which the service sent.
    assert!(!subscription.entitled);
}

/// The invoice history the service also sends is not modelled here, and an answer carrying it must
/// still parse: serde ignores what nothing asked for, and adding a field later must not need a
/// service change.
#[test]
fn fields_this_version_does_not_model_do_not_break_the_read() {
    let body = FREE.replace(
        r#""subscription": null,"#,
        r#""subscription": {"status":"active","interval":"monthly","amountInCents":199,
            "nextPaymentDate":"2026-10-01T00:00:00.000Z","currentPeriodEnd":null,
            "cancelledAt":null,"createdAt":"2026-09-01T00:00:00.000Z",
            "payments":[{"id":"p1","amountInCents":199,"amountRefundedInCents":0,
                "status":"paid","paidAt":"2026-09-01T00:00:00.000Z",
                "createdAt":"2026-09-01T00:00:00.000Z"}]},"#,
    );

    let own = read(&body)
        .expect("the service answered")
        .own
        .expect("an own subscription");

    assert_eq!(own.status, OwnStatus::Active);
    assert_eq!(own.amount_in_cents, 199);
}

/// A refused token is that, and not a service that declined to act: a client refreshes rather than
/// telling somebody their subscription cannot do that.
#[test]
fn a_refused_token_is_not_read_as_a_refusal_to_act() {
    let transport = Canned::new(401, "");
    let service = AccountService::new("https://accounts.example.com");

    assert_eq!(
        service.subscription(&transport, "tok").unwrap_err(),
        Error::Unauthorized
    );
}

#[test]
fn the_account_screen_is_read_from_one_url() {
    let transport = Canned::new(200, FREE);
    let service = AccountService::new("https://accounts.example.com/");

    service.subscription(&transport, "tok").expect("answered");

    let request = transport.only_request();
    assert_eq!(
        request.url,
        "https://accounts.example.com/api/v1/subscription"
    );
    assert_eq!(request.method, crate::Method::Get);
    assert!(request.body.is_none());
}

/// A checkout is a page to open, and the period is named on the request rather than defaulted: it
/// is the field that decides what somebody is charged.
#[test]
fn starting_a_checkout_names_the_period_and_returns_a_page() {
    let transport = Canned::new(200, r#"{"checkoutUrl":"https://pay.example.com/abc"}"#);
    let service = AccountService::new("https://accounts.example.com");

    let checkout = service
        .start_checkout(&transport, "tok", Plan::Yearly)
        .expect("answered");

    assert_eq!(checkout.checkout_url, "https://pay.example.com/abc");
    let request = transport.only_request();
    assert_eq!(
        request.url,
        "https://accounts.example.com/api/v1/subscription/checkout"
    );
    assert_eq!(request.body.as_deref(), Some(r#"{"interval":"yearly"}"#));
}

/// Cancelling says when access ends, and nothing is refunded. A client that cannot say the date
/// before the confirm button is one whose users think they are getting money back.
#[test]
fn cancelling_says_when_access_ends() {
    let transport = Canned::new(200, r#"{"endDate":"2026-10-01T00:00:00.000Z"}"#);
    let service = AccountService::new("https://accounts.example.com");

    let cancelled = service.cancel_subscription(&transport, "tok").expect("ok");

    assert_eq!(cancelled.end_date, "2026-10-01T00:00:00.000Z");
    // The service takes an object on every write, empty where there is nothing to say.
    assert_eq!(transport.only_request().body.as_deref(), Some("{}"));
}

/// Switching changes the next charge and nothing else, and the date it reports back is the one
/// that was always going to come. Saying so is what stops a switch being silent for a month.
#[test]
fn switching_period_changes_the_next_charge_and_not_the_date() {
    let transport = Canned::new(
        200,
        r#"{"interval":"yearly","amountInCents":2000,"nextPaymentDate":"2026-10-01T00:00:00.000Z"}"#,
    );
    let service = AccountService::new("https://accounts.example.com");

    let changed = service
        .switch_interval(&transport, "tok", Plan::Yearly)
        .expect("ok");

    assert_eq!(changed.interval, "yearly");
    assert_eq!(changed.amount_in_cents, 2_000);
    assert_eq!(
        changed.next_payment_date.as_deref(),
        Some("2026-10-01T00:00:00.000Z")
    );
}

/// Resubscribing either reuses the authorisation already held, with nothing to pay and nothing to
/// re-enter, or hands back a page. A client has to draw both.
#[test]
fn resubscribing_either_restarts_or_hands_back_a_page() {
    let service = AccountService::new("https://accounts.example.com");

    let reused = Canned::new(200, r#"{"reactivated":true,"checkoutUrl":null}"#);
    let answer = service.resubscribe(&reused, "tok").expect("ok");
    assert!(answer.reactivated);
    assert!(answer.checkout_url.is_none());

    let fresh = Canned::new(
        200,
        r#"{"reactivated":false,"checkoutUrl":"https://pay.example.com/abc"}"#,
    );
    let answer = service.resubscribe(&fresh, "tok").expect("ok");
    assert!(!answer.reactivated);
    assert_eq!(
        answer.checkout_url.as_deref(),
        Some("https://pay.example.com/abc")
    );
}

/// Every refusal arrives as a code a client switches on. The service's own sentence is never read:
/// a client that rendered one would ship whatever the service happened to say.
#[test]
fn a_refusal_arrives_as_a_code_and_never_as_the_services_words() {
    let service = AccountService::new("https://accounts.example.com");
    for (code, expected) in [
        ("already_cancelled", Refusal::AlreadyCancelled),
        ("already_active", Refusal::AlreadyActive),
        ("already_on_interval", Refusal::AlreadyOnInterval),
        ("not_switchable", Refusal::NotSwitchable),
        ("not_found", Refusal::NotFound),
        ("unavailable", Refusal::Unavailable),
    ] {
        let transport = Canned::new(409, &refusal(code));
        let error = service
            .cancel_subscription(&transport, "tok")
            .expect_err("refused");

        assert_eq!(error, Error::Refused(expected), "code {code}");
        assert!(!format!("{error}").contains("nope"), "the service's words");
    }
}

/// A code this version does not know is kept rather than turned into a parse failure: something
/// refused the call, and a client says something general instead of reporting a broken service.
#[test]
fn a_refusal_this_version_does_not_know_is_still_a_refusal() {
    let service = AccountService::new("https://accounts.example.com");

    let transport = Canned::new(409, &refusal("some_later_reason"));
    assert_eq!(
        service
            .cancel_subscription(&transport, "tok")
            .expect_err("refused"),
        Error::Refused(Refusal::Other("some_later_reason".to_owned()))
    );

    // And a body that cannot be read at all is still a refusal, not a malformed answer: reporting
    // "the response was unreadable" would describe the wrong problem.
    let unreadable = Canned::new(500, "<html>maintenance</html>");
    assert_eq!(
        service
            .cancel_subscription(&unreadable, "tok")
            .expect_err("refused"),
        Error::Refused(Refusal::Other(String::new()))
    );
}
