use super::{Ledger, LinkOutcome, Pending, Settled, StorePurchase};
use crate::{AccountService, Request, Response, Transport, purchase::Store};

fn purchase(id: &str) -> StorePurchase {
    at(id, 0)
}

fn at(id: &str, purchased_at: i64) -> StorePurchase {
    StorePurchase {
        store: Store::Apple,
        purchase: id.to_owned(),
        purchased_at,
    }
}

/// A transport that answers with one status and body, and remembers what it was asked.
struct Canned {
    status: u16,
    body: String,
    seen: std::sync::Mutex<Vec<Request>>,
}

impl Canned {
    fn new(status: u16) -> Self {
        Self::answering(status, "")
    }

    fn answering(status: u16, body: &str) -> Self {
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

fn refusal(code: &str) -> String {
    format!(
        r#"{{"defined":true,"code":"CONFLICT","status":409,"message":"nope","data":{{"code":"{code}"}}}}"#
    )
}

/// The rule the whole module exists for. Until the service has attached it, the store keeps the
/// purchase: StoreKit re-delivers it, and Play refunds it rather than letting somebody pay for
/// nothing.
#[test]
fn nothing_is_settled_until_the_service_has_attached_it() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1")]);

    assert_eq!(
        ledger.apply("t-1", LinkOutcome::Deferred, 1_000),
        Settled::Keep
    );
    assert_eq!(ledger.pending().len(), 1);

    assert_eq!(
        ledger.apply("t-1", LinkOutcome::Linked, 2_000),
        Settled::Settled
    );
    assert!(ledger.pending().is_empty());
}

/// A purchase serving another Allodia account is settled rather than retried forever: it is real,
/// it is doing its job, and leaving it unfinished makes StoreKit offer it again at every launch
/// for the life of the install.
#[test]
fn a_purchase_claimed_by_another_account_is_settled_and_dropped() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1")]);

    assert_eq!(
        ledger.apply("t-1", LinkOutcome::ClaimedElsewhere, 1_000),
        Settled::Settled
    );
    assert!(ledger.pending().is_empty());
}

/// StoreKit re-delivers every unfinished transaction at launch and again through its updates
/// stream, and Play returns it from every `queryPurchasesAsync`. Recording blindly would grow the
/// ledger without bound and attach one purchase once per copy.
#[test]
fn the_same_purchase_reported_twice_is_held_once() {
    let mut ledger = Ledger::default();
    ledger.sync(&[at("t-1", 500)]);
    ledger.sync(&[at("t-1", 900)]);

    assert_eq!(ledger.pending().len(), 1);
    assert_eq!(ledger.pending()[0].purchased_at, 500);
}

/// The store is the authority on what is outstanding. A purchase it has stopped reporting, because
/// it was refunded or attached from somewhere else, is gone, and a ledger that kept retrying it
/// would be attaching something nobody holds.
#[test]
fn a_purchase_the_store_no_longer_reports_is_dropped() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1"), purchase("t-2")]);
    assert_eq!(ledger.pending().len(), 2);

    ledger.sync(&[purchase("t-2")]);

    assert_eq!(ledger.pending().len(), 1);
    assert_eq!(ledger.pending()[0].purchase, "t-2");
}

/// A pass runs every time the store reports, which on a launch is immediately. Resetting the
/// pacing each time would turn the backoff into no backoff at all.
#[test]
fn bringing_the_ledger_into_step_keeps_the_retry_pacing() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1")]);
    ledger.apply("t-1", LinkOutcome::Deferred, 0);

    ledger.sync(&[purchase("t-1")]);

    assert!(ledger.due(30).is_empty(), "still inside the first wait");
    assert_eq!(ledger.pending()[0].attempts, 1);
}

/// Allodia's own checkout is billed against the account already, so there is nothing for a device
/// to carry over. A caller that passed one has confused the two routes rather than found a third.
#[test]
fn a_purchase_from_allodias_own_checkout_is_not_something_to_attach() {
    let mut ledger = Ledger::default();

    ledger.sync(&[StorePurchase {
        store: Store::Allodia,
        ..purchase("t-1")
    }]);

    assert!(ledger.pending().is_empty());
}

/// A fresh purchase is attempted at once; that is what carries an ordinary one straight through.
#[test]
fn a_fresh_purchase_is_due_immediately() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1")]);

    assert_eq!(ledger.due(1_000).len(), 1);
}

/// A failed attempt waits before the next one, and the wait grows. Without this a device with no
/// connectivity attaches in a loop for as long as it is awake.
#[test]
fn a_failed_attempt_waits_and_the_wait_grows() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("t-1")]);

    ledger.apply("t-1", LinkOutcome::Deferred, 0);
    assert!(ledger.due(30).is_empty(), "still inside the first wait");
    assert_eq!(ledger.due(60).len(), 1, "the first wait is a minute");

    ledger.apply("t-1", LinkOutcome::Deferred, 60);
    assert!(ledger.due(110).is_empty(), "the second wait is longer");
    assert_eq!(ledger.due(180).len(), 1);
}

/// The wait is capped rather than left to double. Play refunds an unacknowledged purchase after
/// three days and the acknowledgement happens on the far side of this call, so a backoff that grew
/// past that would turn an outage into a refund nobody asked for.
#[test]
fn the_wait_never_grows_past_an_hour() {
    let stuck = Pending {
        store: Store::Apple,
        purchase: "t-1".to_owned(),
        purchased_at: 0,
        last_attempt_at: Some(0),
        attempts: 30,
    };

    assert_eq!(stuck.backoff_seconds(), 60 * 60);
}

/// Money was taken and nothing was granted. A client that cannot say so leaves the person with no
/// way to find out.
///
/// Measured from the **store's** purchase time rather than from an attempt count, so it survives
/// the app being killed. On a phone that is the ordinary case, and a count would reset before it
/// ever reached the threshold.
#[test]
fn a_purchase_still_waiting_an_hour_later_becomes_worth_mentioning() {
    let mut ledger = Ledger::default();
    ledger.sync(&[at("t-1", 10_000)]);

    assert!(
        !ledger.has_stuck(10_000 + 59 * 60),
        "not while it is merely a slow network"
    );
    assert!(ledger.has_stuck(10_000 + 60 * 60));
}

/// The same purchase, seen again after a relaunch that lost every attempt count. It is still an
/// hour old, because the store said so, and the person is still owed an explanation.
#[test]
fn a_relaunch_does_not_make_an_old_purchase_look_new() {
    let mut fresh_process = Ledger::default();
    fresh_process.sync(&[at("t-1", 10_000)]);

    assert!(fresh_process.has_stuck(10_000 + 60 * 60));
}

/// The oldest is the closest to Play's refund clock, so it goes first.
#[test]
fn the_oldest_purchase_is_attempted_first() {
    let mut ledger = Ledger::default();
    ledger.sync(&[at("newer", 2_000), at("older", 1_000)]);

    let due = ledger.due(3_000);
    assert_eq!(due[0].purchase, "older");
}

/// A purchase attached on an earlier run whose store call was lost is still offered by StoreKit,
/// and something has to finish it or it is offered forever.
#[test]
fn an_answer_about_a_purchase_the_ledger_forgot_still_settles_it() {
    let mut ledger = Ledger::default();

    assert_eq!(
        ledger.apply("t-1", LinkOutcome::Linked, 1_000),
        Settled::Settled
    );
}

/// The service takes a store and an identifier, and **nothing else**: every fact about the
/// subscription is read back from the store, so there is nothing a device could claim about what
/// it bought. The account is the token's rather than anything the body names.
#[test]
fn a_purchase_is_posted_as_a_store_and_an_identifier_and_nothing_else() {
    let transport = Canned::new(200);
    let service = AccountService::new("https://accounts.example.com");

    let outcome = service
        .link_store_purchase(&transport, "an-access-token", &purchase("t-1"))
        .expect("the service answered");

    assert_eq!(outcome, LinkOutcome::Linked);
    let request = transport.only_request();
    assert_eq!(request.method, crate::Method::Post);
    assert_eq!(
        request.url,
        "https://accounts.example.com/api/v1/subscription/store"
    );
    assert_eq!(request.bearer, "an-access-token");
    assert_eq!(request.idempotency_key.as_deref(), Some("t-1"));
    assert_eq!(
        request.body.as_deref(),
        Some(r#"{"store":"apple","purchase":"t-1"}"#)
    );
}

/// The refusal's own `data.code` decides, not the status. The service answers `404` for more than
/// one thing, and a `409` could later carry a code that is not this one, so reading the status
/// alone would conclude from a shape rather than from a reason.
#[test]
fn only_the_owned_by_another_account_code_means_another_account_has_it() {
    let service = AccountService::new("https://accounts.example.com");
    for (status, body, expected) in [
        (200, String::new(), LinkOutcome::Linked),
        (
            409,
            refusal("owned_by_another_account"),
            LinkOutcome::ClaimedElsewhere,
        ),
        // The store does not recognise it **yet**. A transaction made seconds ago is routinely
        // not queryable at the store, so this is retried rather than concluded from.
        (404, refusal("unknown_purchase"), LinkOutcome::Deferred),
        // Billing is not configured on this deployment. Nothing about the purchase.
        (503, refusal("unavailable"), LinkOutcome::Deferred),
        // A conflict whose reason this build does not know.
        (409, refusal("some_later_reason"), LinkOutcome::Deferred),
        (401, String::new(), LinkOutcome::Deferred),
        (500, String::new(), LinkOutcome::Deferred),
        (409, "not json at all".to_owned(), LinkOutcome::Deferred),
    ] {
        let transport = Canned::answering(status, &body);
        let outcome = service
            .link_store_purchase(&transport, "an-access-token", &purchase("t-1"))
            .expect("the service answered");
        assert_eq!(outcome, expected, "status {status} body {body}");
    }
}

/// The identifier claims a subscription: presenting one attaches it to whoever is signed in. So it
/// never reaches a log line, from either of the two types that hold one.
#[test]
fn neither_a_purchase_nor_a_pending_one_prints_its_identifier() {
    let mut ledger = Ledger::default();
    ledger.sync(&[purchase("a-secret-transaction-id")]);

    assert!(!format!("{:?}", purchase("a-secret-transaction-id")).contains("a-secret"));
    assert!(!format!("{ledger:?}").contains("a-secret"));
}
