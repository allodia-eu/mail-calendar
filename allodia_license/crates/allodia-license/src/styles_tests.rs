// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Every answer each writing-style route can give, against a transport that never opens a socket.
//!
//! The wire shape is pinned as well as the behaviour: the service's schema and these types are two
//! descriptions of one payload, and a field renamed on either side stops parsing.

use std::cell::RefCell;

use super::*;
use crate::{
    AccountService, Error, Method, Refusal, Request, Response, SyncedCollection, Transport,
};

/// A transport that answers once from a script and records what it was asked.
struct Fake {
    answer: RefCell<Option<Result<Response, String>>>,
    seen: RefCell<Vec<Request>>,
}

impl Fake {
    fn answering(status: u16, body: &str) -> Self {
        Self {
            answer: RefCell::new(Some(Ok(Response {
                status,
                body: body.to_owned(),
            }))),
            seen: RefCell::new(Vec::new()),
        }
    }

    fn only_request(&self) -> Request {
        let seen = self.seen.borrow();
        assert_eq!(seen.len(), 1, "one request, never a retry");
        seen[0].clone()
    }
}

impl Transport for Fake {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.borrow_mut().push(request.clone());
        self.answer
            .borrow_mut()
            .take()
            .unwrap_or_else(|| panic!("no scripted answer for {}", request.url))
    }
}

fn service() -> AccountService {
    AccountService::new("https://allodia.example/")
}

fn styles(service: &AccountService) -> SyncedCollection<'_, StyleRecord> {
    SyncedCollection::new(service)
}

/// A guide as another, newer device might have written it: every level carries a field this build
/// does not model.
const GUIDE: &str = r#"{"schema_version":1,
    "languages":{"en":{"greetings":[{"text":"Hi","share":80}],"typical_words":60,
                       "a_newer_field":true}},
    "notes":"Keep it short.",
    "learned":{"oldest":1,"newest":2,"messages_per_language":{"en":12},"learned_at":3},
    "a_future_field":{"kept":true}}"#;

fn record_json(id: &str, version: u64, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","version":{version},"style":{{"name":"{name}","guide":{GUIDE}}},
            "updatedAt":"2026-09-23T10:00:00.000Z"}}"#
    )
}

fn style(name: &str) -> SyncedStyle {
    SyncedStyle {
        name: name.to_owned(),
        guide: serde_json::from_str(GUIDE).unwrap(),
    }
}

fn conflict_json(current: &str) -> String {
    format!(
        r#"{{"defined":true,"code":"CONFLICT","status":409,
            "message":"This writing style was changed elsewhere since you last read it.",
            "data":{{"current":{current}}}}}"#
    )
}

const TOMBSTONE: &str = r#"{"id":"abc","version":6,"deletedAt":"2026-09-22T08:00:00.000Z"}"#;

const TOO_MANY: &str = r#"{"defined":true,"code":"BAD_REQUEST","status":400,
    "message":"Too many writing styles.","data":{"code":"too_many_styles"}}"#;

#[test]
fn the_style_list_parses_as_the_service_writes_it_and_keeps_what_it_does_not_model() {
    let body = format!(
        r#"{{"styles":[{}],"deleted":[{TOMBSTONE}],"syncedAt":"2026-09-23T10:05:00.000Z"}}"#,
        record_json("rec-1", 3, "Work")
    );
    let fake = Fake::answering(200, &body);

    let list = styles(&service()).list(&fake, "tok", None).unwrap();

    let sent = fake.only_request();
    assert_eq!(sent.method, Method::Get);
    assert_eq!(sent.url, "https://allodia.example/api/v1/writing-styles");
    assert_eq!(sent.bearer, "tok");
    assert!(sent.body.is_none());
    assert_eq!(list.synced_at, "2026-09-23T10:05:00.000Z");
    let held = &list.styles[0];
    assert_eq!((held.id.as_str(), held.version), ("rec-1", 3));
    assert_eq!(held.style.name, "Work");
    assert_eq!(held.style.guide.notes, "Keep it short.");
    assert_eq!(held.style.guide.languages["en"].typical_words, 60);
    assert!(held.style.guide.unknown.contains_key("a_future_field"));
    assert!(
        held.style.guide.languages["en"]
            .unknown
            .contains_key("a_newer_field")
    );
    assert_eq!(list.deleted[0].id, "abc");
    assert_eq!(list.deleted[0].version, 6);
}

#[test]
fn a_since_timestamp_is_encoded_so_its_offset_survives() {
    let fake = Fake::answering(200, r#"{"styles":[],"deleted":[],"syncedAt":"x"}"#);
    styles(&service())
        .list(&fake, "tok", Some("2026-09-23T10:05:00+02:00"))
        .unwrap();
    let url = fake.only_request().url;
    assert!(
        url.ends_with("/writing-styles?since=2026-09-23T10%3A05%3A00%2B02%3A00"),
        "{url}"
    );
}

#[test]
fn an_unreadable_style_list_is_malformed_rather_than_empty() {
    let fake = Fake::answering(200, "<html>a proxy's page</html>");
    assert!(matches!(
        styles(&service()).list(&fake, "tok", None),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn a_style_create_carries_a_key_the_style_and_no_version() {
    let fake = Fake::answering(200, &record_json("rec-2", 1, "Work"));

    let stored = styles(&service())
        .create(&fake, "tok", &style("Work"), "attempt-1")
        .unwrap();

    assert_eq!((stored.id.as_str(), stored.version), ("rec-2", 1));
    let sent = fake.only_request();
    assert_eq!(sent.method, Method::Post);
    assert_eq!(sent.url, "https://allodia.example/api/v1/writing-styles");
    assert_eq!(sent.idempotency_key.as_deref(), Some("attempt-1"));
    let body: serde_json::Value = serde_json::from_str(sent.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["style"]["name"], "Work");
    assert_eq!(body["style"]["guide"]["a_future_field"]["kept"], true);
    assert!(body.get("version").is_none(), "a create names no version");
}

/// Twenty live styles is all the service keeps. Reported by its code, so a pass can stop sending
/// rather than knock on the same door once per style.
#[test]
fn a_style_past_the_cap_is_refused_by_its_code_on_create_and_on_revival() {
    let fake = Fake::answering(400, TOO_MANY);
    assert_eq!(
        styles(&service()).create(&fake, "tok", &style("Work"), "attempt-1"),
        Err(Error::Refused(Refusal::TooManyStyles))
    );
    fake.only_request();

    // Writing to a forgotten style revives it, which counts against the same cap.
    let fake = Fake::answering(400, TOO_MANY);
    assert_eq!(
        styles(&service()).update(&fake, "tok", "abc", 6, &style("Work")),
        Err(Error::Refused(Refusal::TooManyStyles))
    );
}

/// A payload the service's schema refuses carries no refusal code of its own: it is a bare `400`,
/// never mistaken for the cap.
#[test]
fn a_style_the_service_will_not_accept_is_a_bare_400() {
    let invalid = r#"{"defined":false,"code":"BAD_REQUEST","status":400,
        "message":"Input validation failed","data":{"issues":[{"path":["style","name"]}]}}"#;
    let fake = Fake::answering(400, invalid);
    assert_eq!(
        styles(&service()).create(&fake, "tok", &style(""), "attempt-1"),
        Err(Error::Unexpected { status: 400 })
    );
}

/// A create replayed onto a style that has since been forgotten answers with the tombstone:
/// storing it again would bring back a style somebody forgot.
#[test]
fn a_style_create_replayed_onto_a_forgotten_style_comes_back_as_a_tombstone() {
    let fake = Fake::answering(409, &conflict_json(TOMBSTONE));
    match styles(&service()).create(&fake, "tok", &style("Work"), "attempt-1") {
        Err(Error::Conflict(Some(ConflictWith::Tombstone(gone)))) => assert_eq!(gone.version, 6),
        other => panic!("expected a tombstone, got {other:?}"),
    }
}

#[test]
fn a_style_update_names_the_version_it_read() {
    let fake = Fake::answering(200, &record_json("abc", 4, "Office"));

    let stored = styles(&service())
        .update(&fake, "tok", "abc", 3, &style("Office"))
        .unwrap();

    assert_eq!(stored.version, 4, "the write returns what to read next");
    let sent = fake.only_request();
    assert_eq!(sent.method, Method::Put);
    assert_eq!(
        sent.url,
        "https://allodia.example/api/v1/writing-styles/abc"
    );
    assert!(sent.idempotency_key.is_none());
    let body: serde_json::Value = serde_json::from_str(sent.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["version"], 3);
    assert_eq!(body["style"]["name"], "Office");
}

#[test]
fn a_stale_style_write_is_a_conflict_carrying_the_stored_style() {
    let fake = Fake::answering(
        409,
        &conflict_json(&record_json("abc", 9, "Changed elsewhere")),
    );
    match styles(&service()).update(&fake, "tok", "abc", 3, &style("Office")) {
        Err(Error::Conflict(Some(ConflictWith::Style(current)))) => {
            assert_eq!(current.version, 9);
            assert_eq!(current.style.name, "Changed elsewhere");
        }
        other => panic!("expected a conflict carrying the stored style, got {other:?}"),
    }
}

/// A retry whose first attempt landed is refused with exactly what it was writing, which is a
/// lost receipt rather than a disagreement to put in front of anybody.
#[test]
fn a_lost_style_write_that_already_landed_is_success() {
    let fake = Fake::answering(409, &conflict_json(&record_json("abc", 4, "Office")));
    let stored = styles(&service())
        .update(&fake, "tok", "abc", 3, &style("Office"))
        .expect("the write landed; only the answer was lost");
    assert_eq!(stored.version, 4);
}

#[test]
fn a_style_write_onto_a_forgotten_style_is_a_conflict_carrying_the_tombstone() {
    let fake = Fake::answering(409, &conflict_json(TOMBSTONE));
    assert!(matches!(
        styles(&service()).update(&fake, "tok", "abc", 3, &style("Office")),
        Err(Error::Conflict(Some(ConflictWith::Tombstone(_))))
    ));
}

#[test]
fn a_style_write_the_service_has_no_record_of_is_a_404() {
    let fake = Fake::answering(
        404,
        r#"{"code":"NOT_FOUND","message":"No synced writing style with that id."}"#,
    );
    assert_eq!(
        styles(&service()).update(&fake, "tok", "abc", 3, &style("Office")),
        Err(Error::Unexpected { status: 404 })
    );
}

#[test]
fn forgetting_a_style_puts_the_version_in_the_query_not_the_body() {
    let fake = Fake::answering(
        200,
        r#"{"id":"abc","version":8,"deletedAt":"2026-09-23T10:00:00Z"}"#,
    );
    styles(&service()).delete(&fake, "tok", "abc", 7).unwrap();
    let sent = fake.only_request();
    assert_eq!(sent.method, Method::Delete);
    assert_eq!(
        sent.url,
        "https://allodia.example/api/v1/writing-styles/abc?version=7"
    );
    assert!(sent.body.is_none());
}

/// Gone already, by any of the three ways the service says so, is what was asked for.
#[test]
fn forgetting_a_style_that_is_already_gone_is_success() {
    for (status, body) in [
        (
            200,
            r#"{"id":"abc","version":null,"deletedAt":null}"#.to_owned(),
        ),
        (404, r#"{"code":"NOT_FOUND"}"#.to_owned()),
        (409, conflict_json(TOMBSTONE)),
    ] {
        let fake = Fake::answering(status, &body);
        assert_eq!(
            styles(&service()).delete(&fake, "tok", "abc", 7),
            Ok(()),
            "status {status}"
        );
    }
}

#[test]
fn forgetting_a_style_changed_elsewhere_is_a_conflict() {
    let fake = Fake::answering(
        409,
        &conflict_json(&record_json("abc", 9, "Changed elsewhere")),
    );
    assert!(matches!(
        styles(&service()).delete(&fake, "tok", "abc", 7),
        Err(Error::Conflict(Some(ConflictWith::Style(_))))
    ));
}

/// Every route, every answer that says nothing about styles: a refused token is reported as such,
/// and anything else unexpected keeps its status.
#[test]
fn a_refused_token_and_an_unexpected_status_read_the_same_on_every_style_route() {
    type Call = fn(&SyncedCollection<'_, StyleRecord>, &Fake) -> Result<(), Error>;
    let calls: [(&str, Call); 4] = [
        ("list", |c, t| c.list(t, "tok", None).map(drop)),
        ("create", |c, t| {
            c.create(t, "tok", &style("W"), "k").map(drop)
        }),
        ("update", |c, t| {
            c.update(t, "tok", "abc", 3, &style("W")).map(drop)
        }),
        ("delete", |c, t| c.delete(t, "tok", "abc", 3)),
    ];
    let service = service();
    for (route, call) in calls {
        for (status, expected) in [
            (401, Error::Unauthorized),
            (403, Error::Unauthorized),
            (500, Error::Unexpected { status: 500 }),
            (503, Error::Unexpected { status: 503 }),
        ] {
            let fake = Fake::answering(status, "{}");
            assert_eq!(
                call(&styles(&service), &fake),
                Err(expected),
                "{route} {status}"
            );
        }
        let unreachable = Fake {
            answer: RefCell::new(Some(Err("connection refused".to_owned()))),
            seen: RefCell::new(Vec::new()),
        };
        assert!(
            matches!(
                call(&styles(&service), &unreachable),
                Err(Error::Transport(_))
            ),
            "{route} unreachable"
        );
    }
}
