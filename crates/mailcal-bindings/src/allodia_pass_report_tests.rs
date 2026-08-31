//! What a pass hands back for a person to answer.
//!
//! The other half of [`allodia_pass_tests`](super), which is about what it writes. Its fixtures are
//! shared rather than repeated: a transport that never opens a socket, and a bookkeeping store in
//! memory.

use std::sync::Mutex;

use allodia_license::{AccountList, LocalAccount, Response, SyncState, SyncedConfig, fingerprint};

use super::{
    AllodiaAccountKind, ConnectionSecurity, Prefs, Scripted, SyncBookkeeping, book, google, held,
    pass, record, service, stored_json,
};

#[test]
fn an_account_from_another_device_comes_back_as_an_offer_with_the_typing_done() {
    let transport = Scripted::ok(&[]);
    let service = service();
    let bookkeeping = book();

    let report = pass(&service, &transport, &bookkeeping).apply(
        &[],
        &held(vec![record(
            "abc",
            2,
            SyncedConfig::Imap {
                email: "someone@example.com".to_owned(),
                imap: allodia_license::ImapEndpoint {
                    host: "imap.example.com".to_owned(),
                    port: 993,
                    security: allodia_license::Security::ImplicitTls,
                    username: "someone@example.com".to_owned(),
                },
                smtp: Some(allodia_license::SmtpEndpoint {
                    host: "smtp.example.com".to_owned(),
                    port: 587,
                    security: allodia_license::Security::Starttls,
                }),
                caldav: None,
            },
        )]),
    );

    assert_eq!(report.offers.len(), 1);
    let offer = &report.offers[0];
    assert_eq!(offer.id, "abc");
    assert_eq!(offer.email, "someone@example.com");
    assert_eq!(offer.kind, AllodiaAccountKind::Imap);
    assert_eq!(offer.host.as_deref(), Some("imap.example.com"));
    assert_eq!(offer.port, Some(993));
    assert_eq!(offer.security, Some(ConnectionSecurity::ImplicitTls));
    assert_eq!(offer.smtp_host.as_deref(), Some("smtp.example.com"));
    assert_eq!(
        offer.smtp_security,
        Some(ConnectionSecurity::StartTls),
        "each endpoint keeps its own security"
    );
}

#[test]
fn a_disagreement_reaches_a_person_and_a_one_sided_change_says_so() {
    let transport = Scripted::ok(&[]);
    let service = service();
    let bookkeeping = book();
    let synced = |fingerprint: &str| {
        Some(SyncState {
            id: "abc".to_owned(),
            version: 3,
            fingerprint: fingerprint.to_owned(),
            detached: false,
        })
    };
    let mine = google("someone@gmail.com");

    // Untouched here, moved there.
    let quiet = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: mine.clone(),
        sync: synced(&fingerprint(&mine)),
    }];
    let remote = held(vec![record("abc", 9, google("someone@gmail.com"))]);
    let report = pass(&service, &transport, &bookkeeping).apply(&quiet, &remote);
    assert_eq!(report.changed_elsewhere.len(), 1);
    assert!(!report.changed_elsewhere[0].also_changed_here);
    assert_eq!(report.changed_elsewhere[0].email, "someone@gmail.com");

    // Moved on both sides.
    let both = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: mine,
        sync: synced("what it used to be"),
    }];
    let report = pass(&service, &transport, &bookkeeping).apply(&both, &remote);
    assert_eq!(report.changed_elsewhere.len(), 1);
    assert!(report.changed_elsewhere[0].also_changed_here);
    assert!(
        transport.requests().is_empty(),
        "neither side is written over while the person has not answered"
    );
}

#[test]
fn an_account_removed_elsewhere_is_reported_by_name_and_not_removed_here() {
    let transport = Scripted::ok(&[]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: Some(SyncState {
            id: "abc".to_owned(),
            version: 3,
            fingerprint: fingerprint(&google("someone@gmail.com")),
            detached: false,
        }),
    }];
    let remote = AccountList {
        accounts: Vec::new(),
        deleted: vec![allodia_license::DeletedAccount {
            id: "abc".to_owned(),
            version: 4,
            deleted_at: "2026-08-26T08:00:00.000Z".to_owned(),
        }],
        synced_at: "2026-08-27T12:00:00.000Z".to_owned(),
    };

    let report = pass(&service, &transport, &bookkeeping).apply(&local, &remote);

    assert_eq!(report.removed_elsewhere.len(), 1);
    assert_eq!(report.removed_elsewhere[0].email, "someone@gmail.com");
    assert!(transport.requests().is_empty());
}

/// One refusal is one account's problem. A pass that gave up on the first would leave a person
/// whose second account is fine unable to sync it, for a reason about the first.
#[test]
fn one_account_the_service_refuses_does_not_stop_the_others() {
    let transport = Scripted::new(vec![
        Response {
            status: 500,
            body: "{}".to_owned(),
        },
        Response {
            status: 200,
            body: stored_json("def", 1, "second@gmail.com"),
        },
    ]);
    let service = service();
    let bookkeeping = book();
    let local = vec![
        LocalAccount {
            account_id: "first@gmail.com".to_owned(),
            config: google("first@gmail.com"),
            sync: None,
        },
        LocalAccount {
            account_id: "second@gmail.com".to_owned(),
            config: google("second@gmail.com"),
            sync: None,
        },
    ];

    let report = pass(&service, &transport, &bookkeeping).apply(&local, &held(Vec::new()));

    assert_eq!(report.sent, 1);
    assert!(bookkeeping.get("first@gmail.com").is_none());
    assert!(bookkeeping.get("second@gmail.com").is_some());
}

/// The write landed and the note about it did not. Counting it as sent would tell a person their
/// accounts are in step when the next pass is about to offer one of them back.
#[test]
fn a_write_whose_note_could_not_be_stored_is_not_counted_as_sent() {
    let transport = Scripted::ok(&[&stored_json("abc", 1, "someone@gmail.com")]);
    let service = service();
    let bookkeeping = SyncBookkeeping::load(Box::new(Prefs {
        blob: Mutex::new(None),
        refuse: true,
    }))
    .expect("an empty store");
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: None,
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(&local, &held(Vec::new()));

    assert_eq!(report.sent, 0);
}

/// An account the person kept to this device is invisible in both directions, and its id stays
/// claimed, or the record would come straight back as an offer.
#[test]
fn an_excluded_account_is_neither_pushed_nor_offered_back() {
    let transport = Scripted::ok(&[]);
    let service = service();
    let bookkeeping = book();
    let local = vec![LocalAccount {
        account_id: "someone@gmail.com".to_owned(),
        config: google("someone@gmail.com"),
        sync: Some(SyncState {
            id: "abc".to_owned(),
            version: 3,
            fingerprint: "long since diverged".to_owned(),
            detached: true,
        }),
    }];

    let report = pass(&service, &transport, &bookkeeping).apply(
        &local,
        &held(vec![record("abc", 9, google("someone@gmail.com"))]),
    );

    assert_eq!(report.sent, 0);
    assert!(report.offers.is_empty());
    assert!(report.changed_elsewhere.is_empty());
    assert!(transport.requests().is_empty());
}
