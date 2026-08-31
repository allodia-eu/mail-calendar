// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! What can be wrong here while sync still looks like it is working: a write that overwrites an
//! edit made elsewhere, a create that leaves a second record behind, a `since` window that quietly
//! covers the wrong hours, and two devices that fail to recognise the same mailbox.
//!
//! The wire shape is pinned as well as the behaviour. The service's schema and these types are two
//! descriptions of one payload, and the one that ships is this one: a field renamed on either side
//! stops parsing, and a client that cannot read its own account list has no way to say so beyond
//! looking broken.

use std::cell::RefCell;

use super::*;
use crate::{AccountService, Error, Method, Request, Response, Transport};

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

    /// What it was asked, in order.
    fn requests(&self) -> Vec<Request> {
        self.seen.borrow().clone()
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

fn service() -> AccountService {
    AccountService::new("https://allodia.example/")
}

const LIST: &str = r#"{
  "accounts": [
    {
      "id": "0b8e6f4a-2f1e-4a2b-9d3c-5f6a7b8c9d0e",
      "version": 3,
      "config": {
        "kind": "imap",
        "email": "someone@example.com",
        "imap": {
          "host": "imap.example.com",
          "port": 993,
          "security": "implicit-tls",
          "username": "someone@example.com"
        },
        "smtp": { "host": "smtp.example.com", "port": 465, "security": "implicit-tls" },
        "caldav": null
      },
      "updatedAt": "2026-08-27T10:00:00.000Z"
    },
    {
      "id": "1c9f7a5b-3a2f-4b3c-8e4d-6a7b8c9d0e1f",
      "version": 1,
      "config": { "kind": "google", "email": "someone@gmail.com" },
      "updatedAt": "2026-08-27T09:00:00.000Z"
    }
  ],
  "deleted": [
    { "id": "2daa8b6c-4b3a-4c4d-9f5e-7b8c9d0e1f2a", "version": 5,
      "deletedAt": "2026-08-26T08:00:00.000Z" }
  ],
  "syncedAt": "2026-08-27T10:05:00.000Z"
}"#;

#[test]
fn the_account_list_parses_as_the_service_writes_it() {
    let fake = Fake::ok(200, LIST);
    let list = service().list_accounts(&fake, "tok", None).unwrap();

    assert_eq!(list.accounts.len(), 2);
    assert_eq!(list.synced_at, "2026-08-27T10:05:00.000Z");
    assert_eq!(list.accounts[0].version, 3);
    match &list.accounts[0].config {
        SyncedConfig::Imap {
            email,
            imap,
            smtp,
            caldav,
        } => {
            assert_eq!(email, "someone@example.com");
            assert_eq!(imap.port, 993);
            assert_eq!(imap.security, Security::ImplicitTls);
            assert_eq!(smtp.as_ref().unwrap().host, "smtp.example.com");
            // A diary the account has not got is absent, not an empty one.
            assert!(caldav.is_none());
        }
        other => panic!("expected an IMAP account, got {other:?}"),
    }
    // Everything but the address is derived for the two provider kinds, so a payload carrying more
    // than the address would mean the service and this build disagree about who derives what.
    assert!(matches!(
        &list.accounts[1].config,
        SyncedConfig::Google { email } if email == "someone@gmail.com"
    ));
    assert_eq!(list.deleted[0].version, 5);
}

#[test]
fn a_deletion_comes_back_as_an_id_and_never_as_settings() {
    // A device asks its owner before removing a mailbox they may still want locally, so what the
    // list needs is which record went, not enough to rebuild the account behind the question.
    let fake = Fake::ok(200, LIST);
    let list = service().list_accounts(&fake, "tok", None).unwrap();
    assert_eq!(list.deleted.len(), 1);
    assert_eq!(list.deleted[0].id, "2daa8b6c-4b3a-4c4d-9f5e-7b8c9d0e1f2a");
}

#[test]
fn a_since_timestamp_is_encoded_so_its_offset_survives() {
    // `+` means a space to a form decoder, so an unencoded `+02:00` arrives as a space and the
    // delta silently covers a different window than the one asked for. Nothing fails; the answer
    // is just wrong.
    let fake = Fake::ok(200, LIST);
    service()
        .list_accounts(&fake, "tok", Some("2026-08-27T10:05:00+02:00"))
        .unwrap();

    let url = &fake.seen.borrow()[0].url;
    assert!(url.contains("%2B02%3A00"), "offset not encoded: {url}");
    assert!(!url.contains('+'), "a bare plus survived: {url}");
}

#[test]
fn a_create_carries_an_idempotency_key_and_no_version() {
    // The one write whose identity the server mints, so a lost response cannot be told from a new
    // account. Without the key a retry on a flaky connection leaves a second record behind.
    let created = r#"{"id":"3e1b9c7d-5c4b-4d5e-af6f-8c9d0e1f2a3b","version":1,
        "config":{"kind":"microsoft","email":"someone@example.com"},
        "updatedAt":"2026-08-27T11:00:00.000Z"}"#;
    let fake = Fake::ok(200, created);
    let config = SyncedConfig::Microsoft {
        email: "someone@example.com".to_owned(),
    };

    let account = service()
        .create_account(&fake, "tok", &config, "attempt-1")
        .unwrap();

    assert_eq!(account.version, 1);
    let seen = fake.seen.borrow();
    assert_eq!(seen[0].method, Method::Post);
    assert_eq!(seen[0].idempotency_key.as_deref(), Some("attempt-1"));
    let body = seen[0].body.as_deref().unwrap();
    assert!(body.contains("\"kind\":\"microsoft\""), "{body}");
    assert!(
        !body.contains("version"),
        "a create names no version: {body}"
    );
}

#[test]
fn an_update_names_the_version_it_read() {
    let stored = r#"{"id":"abc","version":4,
        "config":{"kind":"google","email":"someone@gmail.com"},
        "updatedAt":"2026-08-27T11:00:00.000Z"}"#;
    let fake = Fake::ok(200, stored);
    let config = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    };

    let account = service()
        .update_account(&fake, "tok", "abc", 3, &config)
        .unwrap();

    assert_eq!(account.version, 4, "the write returns what to read next");
    let seen = fake.seen.borrow();
    assert_eq!(seen[0].method, Method::Put);
    assert_eq!(seen[0].url, "https://allodia.example/api/v1/accounts/abc");
    assert!(seen[0].body.as_deref().unwrap().contains("\"version\":3"));
}

#[test]
fn a_stale_write_is_a_conflict_carrying_what_the_server_holds() {
    // The whole point of the version: a device that has been offline learns its edit lost, and
    // learns it with enough in hand to show both sides without a second round trip.
    //
    // The server holds settings that are **not** the ones being written, which is what makes this
    // a disagreement rather than a lost receipt, and what keeps the narrow rule below from
    // swallowing the case it resembles.
    let body = r#"{"defined":true,"code":"CONFLICT","status":409,
        "message":"This account was changed elsewhere since you last read it.",
        "data":{"current":{"id":"abc","version":9,
          "config":{"kind":"google","email":"somebody-else@gmail.com"},
          "updatedAt":"2026-08-27T12:00:00.000Z"}}}"#;
    let fake = Fake::ok(409, body);
    let config = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    };

    let error = service()
        .update_account(&fake, "tok", "abc", 3, &config)
        .unwrap_err();

    match error {
        Error::Conflict(Some(ConflictWith::Record(current))) => {
            assert_eq!(current.version, 9);
        }
        other => panic!("expected a conflict carrying the current record, got {other:?}"),
    }
}

#[test]
fn a_conflict_whose_body_cannot_be_read_is_still_a_conflict() {
    // Reporting it as malformed would send the caller down the "the service is broken" path when
    // the answer is "re-read and try again".
    let fake = Fake::ok(409, "not json at all");
    let config = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    };
    assert!(matches!(
        service().update_account(&fake, "tok", "abc", 3, &config),
        Err(Error::Conflict(None))
    ));
}

/// A write whose response was lost is not a disagreement to put in front of anybody.
///
/// The retry re-sends the same base version and is refused, and what the refusal carries is the
/// write it was making. The settings the caller wanted stored are stored; the only thing missing
/// was the receipt. Reporting it as a conflict would ask somebody to choose between a value and
/// itself.
#[test]
fn a_lost_write_that_already_landed_is_success_rather_than_a_conflict() {
    let intended = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    };
    let echo = r#"{"defined":true,"code":"CONFLICT","status":409,
        "message":"This account was changed elsewhere since you last read it.",
        "data":{"current":{"id":"abc","version":4,
          "config":{"kind":"google","email":"someone@gmail.com"},
          "updatedAt":"2026-08-27T12:00:00.000Z"}}}"#;
    let fake = Fake::ok(409, echo);

    let account = service()
        .update_account(&fake, "tok", "abc", 3, &intended)
        .expect("the write landed; only the answer was lost");
    assert_eq!(account.version, 4, "the caller learns the version to hold");
}

/// A create replayed onto a record that has since been deleted comes back as the tombstone.
///
/// The caller has to be able to tell "it moved" from "it is gone": storing it again would be the
/// resurrection bug, and re-sending under a new key would resurrect an account somebody removed.
/// Both shapes are parsed for exactly this.
#[test]
fn a_create_replayed_onto_a_deleted_record_comes_back_as_a_tombstone() {
    let tombstoned = r#"{"defined":true,"code":"CONFLICT","status":409,
        "data":{"current":{"id":"abc","version":6,
          "deletedAt":"2026-08-26T08:00:00.000Z"}}}"#;
    let fake = Fake::ok(409, tombstoned);
    let config = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    };

    match service().create_account(&fake, "tok", &config, "attempt-1") {
        Err(Error::Conflict(Some(ConflictWith::Tombstone(gone)))) => {
            assert_eq!(gone.version, 6);
            assert_eq!(gone.deleted_at, "2026-08-26T08:00:00.000Z");
        }
        other => panic!("expected a tombstone, got {other:?}"),
    }
    assert_eq!(
        fake.requests().len(),
        1,
        "one attempt: the key is the caller's"
    );
}

#[test]
fn deleting_something_already_tombstoned_is_success() {
    // The account is gone, which is what was asked for. Reporting a conflict would have a device
    // ask its owner about a removal that has already happened everywhere.
    let tombstoned = r#"{"defined":true,"code":"CONFLICT","status":409,
        "data":{"current":{"id":"abc","version":6,
          "deletedAt":"2026-08-26T08:00:00.000Z"}}}"#;
    let fake = Fake::ok(409, tombstoned);
    assert!(service().delete_account(&fake, "tok", "abc", 3).is_ok());
}

#[test]
fn deleting_puts_the_version_in_the_query_not_the_body() {
    // A DELETE body is legal and widely dropped, and this is a field the request cannot proceed
    // without.
    let fake = Fake::ok(200, "{}");
    service().delete_account(&fake, "tok", "abc", 7).unwrap();

    let seen = fake.seen.borrow();
    assert_eq!(seen[0].method, Method::Delete);
    assert_eq!(
        seen[0].url,
        "https://allodia.example/api/v1/accounts/abc?version=7"
    );
    assert!(seen[0].body.is_none());
}

#[test]
fn deleting_something_already_gone_is_success() {
    // The caller wanted the record gone and it is gone. Treating "already absent" as a failure
    // leaves a device retrying something that can never change.
    let fake = Fake::ok(404, r#"{"code":"NOT_FOUND"}"#);
    assert!(service().delete_account(&fake, "tok", "abc", 7).is_ok());
}

#[test]
fn an_expired_token_is_reported_as_such_rather_than_as_an_outage() {
    let fake = Fake::ok(401, "");
    assert!(matches!(
        service().list_accounts(&fake, "tok", None),
        Err(Error::Unauthorized)
    ));
}

#[test]
fn the_same_address_over_two_protocols_is_two_accounts() {
    // The matching rule decides whether a new device offers someone an account they already have,
    // and whether the service can spot a duplicate. Host is deliberately not in it: it is the field
    // that legitimately differs between networks, and the one autodetect races to different
    // answers for.
    let imap = SyncedConfig::Imap {
        email: "someone@example.com".to_owned(),
        imap: ImapEndpoint {
            host: "imap.example.com".to_owned(),
            port: 993,
            security: Security::ImplicitTls,
            username: "someone@example.com".to_owned(),
        },
        smtp: None,
        caldav: None,
    };
    let elsewhere = SyncedConfig::Imap {
        email: "SOMEONE@example.com".to_owned(),
        imap: ImapEndpoint {
            // The same mailbox, reached by the name a different strategy found.
            host: "mail.example.com".to_owned(),
            port: 993,
            security: Security::ImplicitTls,
            username: "someone@example.com".to_owned(),
        },
        smtp: None,
        caldav: None,
    };
    let jmap = SyncedConfig::Jmap {
        email: "someone@example.com".to_owned(),
        base_url: "https://jmap.example.com/.well-known/jmap".to_owned(),
        auth: JmapAuth::OAuth,
    };

    assert!(
        imap.is_same_account_as(&elsewhere),
        "a different host is the same mailbox"
    );
    assert!(
        !imap.is_same_account_as(&jmap),
        "two protocols are two accounts"
    );
}

#[test]
fn jmap_auth_and_security_use_the_service_s_own_spelling() {
    // Both are closed sets on the wire, and both would be silently wrong if this build spelled
    // them its own way: serde would refuse the payload and sync would look like an outage.
    let json = serde_json::to_string(&SyncedConfig::Jmap {
        email: "someone@example.com".to_owned(),
        base_url: "https://jmap.example.com/".to_owned(),
        auth: JmapAuth::Secret,
    })
    .unwrap();
    assert!(json.contains("\"auth\":\"secret\""), "{json}");
    assert!(json.contains("\"baseUrl\":"), "{json}");
    assert_eq!(
        serde_json::to_string(&Security::Starttls).unwrap(),
        "\"starttls\""
    );
    assert_eq!(
        serde_json::to_string(&Security::ImplicitTls).unwrap(),
        "\"implicit-tls\""
    );
}
