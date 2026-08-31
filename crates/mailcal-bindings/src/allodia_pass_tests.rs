//! What one pass writes, and what it hands back for a person to answer.
//!
//! Everything here runs against a transport that never opens a socket and a bookkeeping store that
//! lives in memory, so each case pins a rule rather than a service's mood on the day.

use std::sync::Mutex;

use allodia_license::{
    AccountList, AccountService, LocalAccount, Method, Request, Response, SyncState, SyncedAccount,
    SyncedConfig, Transport, fingerprint,
};

use super::{Pass, forget_at_service, local_accounts};
use crate::{
    allodia_sync::AllodiaAccountKind,
    setup::ConnectionSecurity,
    sync_state::{StoredSyncState, SyncBookkeeping, SyncStateError, SyncStateStore},
};

/// A transport that answers from a script and records what it was asked.
struct Scripted {
    answers: Mutex<Vec<Response>>,
    seen: Mutex<Vec<Request>>,
}

impl Scripted {
    fn new(answers: Vec<Response>) -> Self {
        Self {
            answers: Mutex::new(answers),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn ok(bodies: &[&str]) -> Self {
        Self::new(
            bodies
                .iter()
                .map(|body| Response {
                    status: 200,
                    body: (*body).to_owned(),
                })
                .collect(),
        )
    }

    fn requests(&self) -> Vec<Request> {
        self.seen.lock().expect("seen").clone()
    }
}

impl Transport for Scripted {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.lock().expect("seen").push(request.clone());
        let mut answers = self.answers.lock().expect("answers");
        if answers.is_empty() {
            return Err("the script ran out of answers".to_owned());
        }
        Ok(answers.remove(0))
    }
}

/// A bookkeeping store in memory, which can be made to refuse.
#[derive(Default)]
struct Prefs {
    blob: Mutex<Option<String>>,
    refuse: bool,
}

impl SyncStateStore for Prefs {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        Ok(self.blob.lock().expect("blob").clone())
    }

    fn save(&self, blob: String) -> Result<(), SyncStateError> {
        if self.refuse {
            return Err(SyncStateError::Store("this store refuses".to_owned()));
        }
        *self.blob.lock().expect("blob") = Some(blob);
        Ok(())
    }
}

fn book() -> SyncBookkeeping {
    SyncBookkeeping::load(Box::new(Prefs::default())).expect("an empty store")
}

fn google(email: &str) -> SyncedConfig {
    SyncedConfig::Google {
        email: email.to_owned(),
    }
}

fn record(id: &str, version: u64, config: SyncedConfig) -> SyncedAccount {
    SyncedAccount {
        id: id.to_owned(),
        version,
        config,
        updated_at: "2026-08-27T12:00:00.000Z".to_owned(),
    }
}

fn held(accounts: Vec<SyncedAccount>) -> AccountList {
    AccountList {
        accounts,
        deleted: Vec::new(),
        synced_at: "2026-08-27T12:00:00.000Z".to_owned(),
    }
}

/// The record the service answers a store with, as JSON.
fn stored_json(id: &str, version: u64, email: &str) -> String {
    format!(
        r#"{{"id":"{id}","version":{version},"config":{{"kind":"google","email":"{email}"}},
           "updatedAt":"2026-08-27T12:00:00.000Z"}}"#
    )
}

fn pass<'a>(
    service: &'a AccountService,
    transport: &'a Scripted,
    bookkeeping: &'a SyncBookkeeping,
) -> Pass<'a> {
    Pass {
        service,
        transport,
        token: "an-access-token",
        bookkeeping,
    }
}

fn service() -> AccountService {
    AccountService::new("https://mailcal.example.com")
}

#[test]
fn an_account_the_service_has_never_seen_is_uploaded_and_written_down() {
    let transport = Scripted::ok(&[&stored_json("abc", 1, "someone@gmail.com")]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(&local, &held(Vec::new()));

    assert_eq!(report.sent, 1);
    assert!(report.offers.is_empty());
    let stored = bookkeeping
        .get("someone@gmail.com")
        .expect("the device now knows the record");
    assert_eq!(stored.id, "abc");
    assert_eq!(stored.version, 1);
    assert_eq!(
        stored.fingerprint,
        fingerprint(&google("someone@gmail.com")),
        "the base is what was sent, so an unchanged account does not push again"
    );
}

/// The bug this key scheme exists for, and it took two goes to get right.
///
/// A key derived from the account alone is the same for its whole life, so once the record is
/// deleted the next create replays the first and is refused; permanently, because neither the key
/// nor the answer ever changes. Measured on a device: unlink, switch sync back on, and every pass
/// reported "could not be sent: this account was changed elsewhere". A single retry past the
/// tombstone was not enough either: each delete stacks another layer.
#[test]
fn a_create_after_a_delete_uses_a_key_the_service_has_never_seen() {
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];
    let service = service();

    let first = Scripted::ok(&[&stored_json("abc", 1, "someone@gmail.com")]);
    pass(&service, &first, &bookkeeping).apply(&local, &held(Vec::new()));
    let first_key = first.requests()[0]
        .idempotency_key
        .clone()
        .expect("a create carries one");

    // The account is taken off every device, which forgets what this one knew about it.
    bookkeeping.forget("someone@gmail.com").expect("forgotten");

    let again = Scripted::ok(&[&stored_json("def", 1, "someone@gmail.com")]);
    pass(&service, &again, &bookkeeping).apply(&local, &held(Vec::new()));
    let second_key = again.requests()[0]
        .idempotency_key
        .clone()
        .expect("a create carries one");

    assert_ne!(
        first_key, second_key,
        "a create after a delete is a new create, not a replay of the old one"
    );
}

/// …and the half that key is *for*: a create whose answer never arrived must present the same key
/// again, or the retry makes a second account for the same mailbox.
#[test]
fn a_create_whose_answer_was_lost_retries_under_the_same_key() {
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];
    let service = service();

    // The service never answers, so nothing is confirmed and the key has to survive the pass.
    let lost = Scripted::new(vec![Response {
        status: 503,
        body: "{}".to_owned(),
    }]);
    pass(&service, &lost, &bookkeeping).apply(&local, &held(Vec::new()));

    let retry = Scripted::ok(&[&stored_json("abc", 1, "someone@gmail.com")]);
    pass(&service, &retry, &bookkeeping).apply(&local, &held(Vec::new()));

    assert_eq!(
        lost.requests()[0].idempotency_key,
        retry.requests()[0].idempotency_key,
        "the retry is the same create, so it presents the same key"
    );
    assert!(
        bookkeeping
            .pending_create_key("someone@gmail.com")
            .is_none(),
        "and the confirmed create drops it, or the NEXT create would replay this one"
    );
}

/// A create refused because the record has since been deleted is not re-sent under a fresh key:
/// that would resurrect an account somebody removed. The key is dropped so nothing is wedged.
#[test]
fn a_create_refused_by_a_tombstone_drops_its_key_rather_than_resurrecting() {
    let tombstoned = r#"{"defined":true,"code":"CONFLICT","status":409,
        "data":{"current":{"id":"abc","version":6,
          "deletedAt":"2026-08-26T08:00:00.000Z"}}}"#;
    let transport = Scripted::new(vec![Response {
        status: 409,
        body: tombstoned.to_owned(),
    }]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(&local, &held(Vec::new()));

    assert_eq!(report.sent, 0);
    assert_eq!(transport.requests().len(), 1, "one attempt, not a re-send");
    assert!(
        bookkeeping
            .pending_create_key("someone@gmail.com")
            .is_none(),
        "a key whose answer can no longer change is how an account gets wedged"
    );
}

/// A create carries a key at all, and it is written down before the request goes.
///
/// The failure it guards against is a response that never arrives, so a key that only exists in
/// the stack frame that sent it is gone by the time the retry needs it. What the key must NOT be
/// is stable for the account's life; see the two cases above.
#[test]
fn a_create_carries_a_key_and_records_it_before_sending() {
    let transport = Scripted::ok(&[&stored_json("abc", 1, "someone@gmail.com")]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];

    pass(&service, &transport, &bookkeeping).apply(&local, &held(Vec::new()));

    let sent = transport.requests();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].method, Method::Post);
    assert!(sent[0].idempotency_key.is_some());
}

#[test]
fn an_account_this_device_changed_is_pushed_at_the_version_it_read() {
    let transport = Scripted::ok(&[&stored_json("abc", 8, "someone@gmail.com")]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: Some(SyncState {
            id: "abc".to_owned(),
            version: 7,
            fingerprint: "what it used to be".to_owned(),
            detached: false,
        }),
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(
        &local,
        &held(vec![record("abc", 7, google("someone@gmail.com"))]),
    );

    assert_eq!(report.sent, 1);
    let sent = transport.requests();
    assert_eq!(sent[0].method, Method::Put);
    assert!(sent[0].url.ends_with("/accounts/abc"), "{}", sent[0].url);
    assert!(
        sent[0].body.as_deref().unwrap().contains("\"version\":7"),
        "the write names the version this device read, not the one it wants"
    );
    assert_eq!(bookkeeping.get("someone@gmail.com").unwrap().version, 8);
}

/// The duplicate the opaque id exists to prevent. Two devices setting the same mailbox up
/// independently is ordinary; uploading a second record for it is not recoverable afterwards,
/// because nothing later can tell the two apart.
#[test]
fn an_account_the_service_already_holds_is_adopted_rather_than_uploaded_twice() {
    let transport = Scripted::ok(&[]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(
        &local,
        // The same mailbox, under an id this device has never seen.
        &held(vec![record("abc", 4, google("SOMEONE@gmail.com"))]),
    );

    assert_eq!(report.sent, 0, "adopting writes nothing to the service");
    assert!(transport.requests().is_empty());
    assert!(report.offers.is_empty(), "it is not offered back to itself");
    let stored = bookkeeping.get("someone@gmail.com").expect("adopted");
    assert_eq!(stored.id, "abc");
    assert_eq!(stored.version, 4);
    assert_eq!(
        stored.fingerprint,
        fingerprint(&google("SOMEONE@gmail.com")),
        "the base is the service's, so the next pass pushes this device's settings and the two \
         converge instead of both believing they are in step"
    );
}

/// An account whose settings the service's shape cannot carry keeps working here; it is simply not
/// synced. Refusing the pass over it would stop a whole account list for one account.
#[test]
fn an_account_that_cannot_be_represented_is_left_out_rather_than_failing_the_pass() {
    let bookkeeping = book();
    let configs = [
        (
            "someone@gmail.com".to_owned(),
            "[google]\nemail = \"someone@gmail.com\"\n".to_owned(),
        ),
        (
            "split@example.com".to_owned(),
            // The dial host and the TLS name differ, which the service's shape has no room for.
            "[imap]\naddr = \"10.0.0.4:993\"\nserver_name = \"mail.example.com\"\n\
             username = \"split@example.com\"\npassword = \"hunter2\"\n"
                .to_owned(),
        ),
    ]
    .into_iter()
    .collect();

    let local = local_accounts(&configs, &bookkeeping);

    assert_eq!(local.len(), 1);
    assert_eq!(local[0].account_id, "someone@gmail.com");
}

#[test]
fn what_the_bookkeeping_remembers_reaches_the_reconciler() {
    let bookkeeping = book();
    bookkeeping
        .set(
            "someone@gmail.com",
            StoredSyncState {
                id: "abc".to_owned(),
                version: 5,
                fingerprint: "a base".to_owned(),
            },
        )
        .expect("stored");
    let configs = [(
        "someone@gmail.com".to_owned(),
        "[google]\nemail = \"someone@gmail.com\"\n".to_owned(),
    )]
    .into_iter()
    .collect();

    let local = local_accounts(&configs, &bookkeeping);

    let sync = local[0].sync.as_ref().expect("the entry travelled");
    assert_eq!(sync.id, "abc");
    assert_eq!(sync.version, 5);
    assert_eq!(sync.fingerprint, "a base");
    assert!(!sync.detached, "nothing has been excluded");
}

/// An excluded account the service already holds is still handed to the reconciler, as detached.
/// Dropping it would leave its record spoken for by nobody, and offered straight back.
#[test]
fn an_excluded_account_with_a_record_keeps_its_record_claimed() {
    let bookkeeping = book();
    bookkeeping
        .set(
            "someone@gmail.com",
            StoredSyncState {
                id: "abc".to_owned(),
                version: 5,
                fingerprint: "a base".to_owned(),
            },
        )
        .expect("stored");
    bookkeeping
        .set_excluded("someone@gmail.com", true)
        .expect("excluded");
    let configs = [(
        "someone@gmail.com".to_owned(),
        "[google]\nemail = \"someone@gmail.com\"\n".to_owned(),
    )]
    .into_iter()
    .collect();

    let local = local_accounts(&configs, &bookkeeping);

    assert_eq!(local.len(), 1);
    assert!(local[0].sync.as_ref().expect("still claimed").detached);
}

/// The case the exclusion set exists for. An account excluded before it was ever sent has no entry
/// to mark, so leaving it in the list would upload the very account somebody asked this device to
/// keep to itself.
#[test]
fn an_excluded_account_that_was_never_sent_is_left_out_entirely() {
    let bookkeeping = book();
    bookkeeping
        .set_excluded("someone@gmail.com", true)
        .expect("excluded");
    let configs = [(
        "someone@gmail.com".to_owned(),
        "[google]\nemail = \"someone@gmail.com\"\n".to_owned(),
    )]
    .into_iter()
    .collect();

    let local = local_accounts(&configs, &bookkeeping);

    assert!(local.is_empty());
}

/// A removal names the version it read, in the query rather than a body: a `DELETE` with a body
/// is dropped by enough intermediaries to be worth never producing.
#[test]
fn removing_an_account_tells_the_service_the_version_it_read() {
    let transport = Scripted::ok(&["{}"]);
    let service = service();
    let bookkeeping = book();

    forget_at_service(
        &pass(&service, &transport, &bookkeeping),
        "someone@gmail.com",
        &StoredSyncState {
            id: "abc".to_owned(),
            version: 6,
            fingerprint: "a base".to_owned(),
        },
    );

    let sent = transport.requests();
    assert_eq!(sent[0].method, Method::Delete);
    assert!(
        sent[0].url.ends_with("/accounts/abc?version=6"),
        "{}",
        sent[0].url
    );
    assert!(sent[0].body.is_none());
}

#[path = "allodia_pass_report_tests.rs"]
mod report;
