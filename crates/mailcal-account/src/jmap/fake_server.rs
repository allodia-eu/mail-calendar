//! A JMAP server for the connect tests, on the crate's test server (`crate::http_fake`): it
//! serves a session and answers every `Foo/get`, and refuses whichever `Authorization` values a
//! test has revoked with a `Bearer` challenge.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

use crate::http_fake::{self, Received, Reply, Seen};

/// The account's own JMAP account id in the session.
pub(super) const OWN: &str = "a";
/// A mailbox shared with the account, listed in the session beside its own.
pub(super) const SHARED: &str = "s";
/// The address book the session's account lists, writable and its default.
pub(super) const BOOK: &str = "book-1";

/// The `accountId` of a request's first method call, `None` for a session `GET`.
pub(super) fn account_id(seen: &Seen) -> Option<String> {
    let body: Value = serde_json::from_str(&seen.body).ok()?;
    body.pointer("/methodCalls/0/1/accountId")?
        .as_str()
        .map(str::to_owned)
}

/// The running server.
pub(super) struct FakeJmap {
    url: String,
    seen: Received,
    revoked: Arc<Mutex<Vec<String>>>,
    hang_ups: Arc<AtomicUsize>,
}

impl FakeJmap {
    /// Starts a server on a loopback port, accepting every credential until one is revoked.
    pub(super) fn start() -> Self {
        let revoked = Arc::new(Mutex::new(Vec::<String>::new()));
        let refusals = Arc::clone(&revoked);
        let hang_ups = Arc::new(AtomicUsize::new(0));
        let hanging = Arc::clone(&hang_ups);
        let (url, seen) = http_fake::serve_with(move |request| {
            let hang_up = request.method == "POST"
                && hanging
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok();
            if hang_up {
                Reply::HangUp
            } else if refusals.lock().unwrap().contains(&request.authorization) {
                Reply::RefusedBearer
            } else if request.method == "GET" {
                Reply::Json(session().to_string())
            } else {
                Reply::Json(answer(&request.body).to_string())
            }
        });
        Self {
            url,
            seen,
            revoked,
            hang_ups,
        }
    }

    /// The base URL an account connects to.
    pub(super) fn url(&self) -> &str {
        &self.url
    }

    /// Refuses `authorization` (a whole header value) from now on.
    pub(super) fn revoke(&self, authorization: &str) {
        self.revoked.lock().unwrap().push(authorization.to_owned());
    }

    /// Closes the connection of the next method call with no answer, as a socket the OS killed
    /// while the machine slept is.
    pub(super) fn hang_up_next_call(&self) {
        self.hang_ups.fetch_add(1, Ordering::SeqCst);
    }

    /// How many method calls were received, the ones hung up on included.
    pub(super) fn calls(&self) -> usize {
        self.seen()
            .iter()
            .filter(|seen| seen.method == "POST")
            .count()
    }

    /// Every request so far, in order.
    pub(super) fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    /// How many times the session was fetched, which is once per connect.
    pub(super) fn discoveries(&self) -> usize {
        self.seen()
            .iter()
            .filter(|seen| seen.method == "GET")
            .count()
    }
}

/// A session with mail, calendars, contacts, an out-of-office reply and mail rules on the account's
/// own JMAP account, and mail and calendars on one shared with it.
fn session() -> Value {
    let core = "urn:ietf:params:jmap:core";
    let mail = "urn:ietf:params:jmap:mail";
    let calendars = "urn:ietf:params:jmap:calendars";
    let contacts = "urn:ietf:params:jmap:contacts";
    let vacation = "urn:ietf:params:jmap:vacationresponse";
    let sieve = "urn:ietf:params:jmap:sieve";
    json!({
        "capabilities": {
            core: {}, mail: {}, calendars: {}, contacts: {}, vacation: {}, sieve: {}
        },
        "primaryAccounts": {
            mail: OWN, calendars: OWN, contacts: OWN, vacation: OWN, sieve: OWN
        },
        "accounts": {
            OWN: {
                "name": "you@example.net",
                "isPersonal": true,
                "isReadOnly": false,
                "accountCapabilities": {
                    mail: {}, calendars: {}, contacts: {}, vacation: {}, sieve: {}
                },
            },
            SHARED: {
                "name": "team@example.net",
                "isPersonal": false,
                "isReadOnly": false,
                "accountCapabilities": { mail: {}, calendars: {} },
            },
        },
        "apiUrl": "https://mail.example.net/jmap/",
        "state": "session-1",
    })
}

/// The answer to every `Foo/get` in `body`: no objects but the one address book and the reply,
/// which is off.
fn answer(body: &str) -> Value {
    let request: Value = serde_json::from_str(body).unwrap_or_default();
    let calls = request["methodCalls"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let responses: Vec<Value> = calls
        .iter()
        .map(|call| {
            let name = call[0].as_str().unwrap_or_default();
            let list = if name == "AddressBook/get" {
                json!([{
                    "id": BOOK,
                    "name": "Contacts",
                    "isDefault": true,
                    "myRights": { "mayRead": true, "mayWrite": true, "mayDelete": true },
                }])
            } else if name == "VacationResponse/get" {
                json!([{ "id": "singleton", "isEnabled": false }])
            } else {
                json!([])
            };
            let result = json!({
                "accountId": call[1]["accountId"],
                "state": "state-1",
                "list": list,
                "notFound": [],
            });
            json!([name, result, call[2]])
        })
        .collect();
    json!({ "methodResponses": responses, "sessionState": "session-1" })
}
