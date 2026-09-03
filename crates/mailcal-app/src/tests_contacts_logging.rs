//! What the contacts paths write to the diagnostic log, asserted against a capturing logger.
//!
//! These exist because the log is the *only* instrument for a live contacts session; there is
//! no way to ask a running client why its list is empty, and an instrument nobody checks is
//! one that quietly stops working. Two claims are load-bearing enough to gate:
//!
//! 1. **Every stage announces itself.** Bind, sync, rebuild, detail, autosuggest: each writes a
//!    line with a count and a duration, so an empty Contacts list can be traced to the stage that
//!    produced nothing rather than guessed at.
//! 2. **No card content ever reaches the log.** `docs/logging.md` makes this a hard rule and calls
//!    contacts the most identifying data the app holds. Nothing enforced it. A name or an address
//!    leaking into a rotating file a user is invited to attach to a support report is a privacy
//!    incident, not a cosmetic bug, and it is a one-word edit away at every call site, which is
//!    exactly the kind of rule that needs a machine watching it.
//!
//! The capture filters on the two contacts modules by `target()`, so a parallel test logging
//! from elsewhere in the crate can neither satisfy nor break either claim.

use std::sync::{Arc, Mutex, OnceLock};

use super::*;

/// A card whose every field is a nonsense token, so "did this leak?" is answerable by a
/// substring search that cannot collide with an unrelated log line.
const NAME: &str = "Zelphina Quorrix";
const EMAIL: &str = "zelphina@carbuncle.test";

/// Collects the contacts modules' log records.
struct Capture(Arc<Mutex<Vec<String>>>);

impl log::Log for Capture {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        if matches!(
            record.target(),
            "mailcal_app::contacts" | "mailcal_app::contacts_write" | "mailcal_app::recipients"
        ) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// The process-wide capture buffer, installed on first use.
///
/// `log` allows exactly one logger per process and the test binary runs its cases in parallel,
/// so this is a `OnceLock` rather than per-test setup; every case reads the same buffer, which
/// is why each asserts on lines it can identify rather than on the buffer's whole contents.
fn captured() -> &'static Arc<Mutex<Vec<String>>> {
    static LINES: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
    LINES.get_or_init(|| {
        let lines = Arc::new(Mutex::new(Vec::new()));
        log::set_boxed_logger(Box::new(Capture(Arc::clone(&lines))))
            .expect("this test binary installs no other logger");
        log::set_max_level(log::LevelFilter::Debug);
        lines
    })
}

/// Every captured line so far.
fn lines() -> Vec<String> {
    captured().lock().unwrap().clone()
}

/// Asserts some captured line contains `needle`, printing the buffer when it does not: a bare
/// `assert!(any(...))` on a missing log line is otherwise undebuggable.
fn assert_logged(needle: &str) {
    let lines = lines();
    assert!(
        lines.iter().any(|line| line.contains(needle)),
        "no contacts log line contains {needle:?}; captured:\n{}",
        lines.join("\n"),
    );
}

#[tokio::test]
async fn every_contacts_stage_logs_a_count_and_a_duration() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![card("c1", "work-book", NAME, EMAIL)],
            ))],
        )],
        &surfaces,
    );
    let _ = captured();

    app.dispatch(crate::Intent::RefreshContacts).await;
    // The per-source line: which account, which source, and what it actually applied. Without
    // the counts, "synced" and "synced nothing" are the same line.
    assert_logged("contacts[a0]: source[0] +1 -0 card(s)");
    assert_logged("1 source(s) on 1 of 1 account(s); 1 synced, 0 unavailable, 0 failed");

    let row = app.contacts().rows.remove(0);
    app.dispatch(crate::Intent::SearchContacts {
        query: "zel".to_owned(),
    })
    .await;
    // The length, never the term; `query_chars=3` for "zel".
    assert_logged("rebuild_contacts: 1 row(s), query_chars=3");

    app.contact_detail(&row.id).await.expect("detail resolves");
    assert_logged("resolved over 1 account(s)");

    app.recipient_suggestions("zel").await;
    assert_logged("recipient_suggestions: 1 match(es) (1 saved) for a 3-char token");

    // A host that hands back something that was never a person id gets a line saying so,
    // rather than an indistinguishable silent `None`.
    assert!(app.contact_detail("not-a-number").await.is_none());
    assert_logged("host passed an id that is not a person id");
}

#[tokio::test]
async fn the_contacts_log_never_carries_a_name_or_an_address() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(
                FakeContacts::new("work-book", vec![card("c1", "work-book", NAME, EMAIL)])
                    .writable(),
            )],
        )],
        &surfaces,
    );
    let _ = captured();

    // Drive every path that touches a card, including the three that take user-typed text;
    // a search term, a composer token and an editor's fields are themselves names and
    // addresses.
    app.dispatch(crate::Intent::RefreshContacts).await;
    let row = app.contacts().rows.remove(0);
    app.dispatch(crate::Intent::SearchContacts {
        query: NAME.to_owned(),
    })
    .await;
    app.contact_detail(&row.id).await;
    app.recipient_suggestions(EMAIL).await;
    // The writes, including the two refusals: a validation message is the likeliest place for
    // a value to be quoted back, because quoting it is what makes such a message helpful.
    let typed = mailcal_account::ContactEdit {
        given_name: "Zelphina".into(),
        surname: "Quorrix".into(),
        emails: vec![EMAIL.to_owned()],
        ..mailcal_account::ContactEdit::default()
    };
    app.dispatch(crate::Intent::CreateContact {
        account: None,
        address_book: None,
        edit: typed.clone(),
    })
    .await;
    app.dispatch(crate::Intent::CreateContact {
        account: None,
        address_book: None,
        edit: mailcal_account::ContactEdit {
            emails: vec![NAME.to_owned()],
            ..mailcal_account::ContactEdit::default()
        },
    })
    .await;
    app.dispatch(crate::Intent::UpdateContact {
        person: row.id.clone(),
        account: "work".to_owned(),
        card: "c1".to_owned(),
        edit: mailcal_account::ContactEdit {
            given_name: "Zelphina".into(),
            surname: "Quorrix-Vane".into(),
            ..typed
        },
    })
    .await;

    for line in lines() {
        for secret in ["Zelphina", "Quorrix", "zelphina", "carbuncle"] {
            assert!(
                !line.contains(secret),
                "a contacts log line leaked card content ({secret:?}): {line}",
            );
        }
    }
}

#[tokio::test]
async fn an_account_with_no_contact_sources_says_so_rather_than_logging_nothing() {
    // The first line to read when a live session reports an empty Contacts list: it separates
    // "we bound no sources" (a connect problem, diagnosed from the `connection_info` lines)
    // from "we synced sources that returned nothing" (a server or permission problem).
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("work", Vec::new())], &surfaces);
    let _ = captured();

    app.dispatch(crate::Intent::RefreshContacts).await;

    assert_logged("contacts[a0]: skipped; no contact sources bound");
    assert_logged("refresh_contacts: no contact sources on any of 1 account(s)");
}
