// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Every rule the sync design rests on, as a case.
//!
//! The ones worth having are the four that look alike from the outside and are not: the server
//! moved, this device moved, both moved, and neither did. Getting the third confused with the
//! first is how an "update" silently eats somebody's edit, and nothing downstream could tell.

use super::*;
use crate::accounts::{DeletedAccount, ImapEndpoint, Security};

fn imap(email: &str, host: &str) -> SyncedConfig {
    SyncedConfig::Imap {
        email: email.to_owned(),
        imap: ImapEndpoint {
            host: host.to_owned(),
            port: 993,
            security: Security::ImplicitTls,
            username: email.to_owned(),
        },
        smtp: None,
        caldav: None,
    }
}

fn record(id: &str, version: u64, config: SyncedConfig) -> SyncedAccount {
    SyncedAccount {
        id: id.to_owned(),
        version,
        config,
        updated_at: "2026-08-27T10:00:00.000Z".to_owned(),
    }
}

fn list(accounts: Vec<SyncedAccount>, deleted: Vec<DeletedAccount>) -> AccountList {
    AccountList {
        accounts,
        deleted,
        synced_at: "2026-08-27T10:05:00.000Z".to_owned(),
    }
}

fn gone(id: &str, version: u64) -> DeletedAccount {
    DeletedAccount {
        id: id.to_owned(),
        version,
        deleted_at: "2026-08-26T08:00:00.000Z".to_owned(),
    }
}

/// A local account already in step with the service.
fn synced(account_id: &str, config: SyncedConfig, id: &str, version: u64) -> LocalAccount {
    let fingerprint = fingerprint(&config);
    LocalAccount {
        account_id: account_id.to_owned(),
        config,
        sync: Some(SyncState {
            id: id.to_owned(),
            version,
            fingerprint,
            detached: false,
        }),
    }
}

#[test]
fn nothing_changed_on_either_side_is_nothing_to_do() {
    let config = imap("someone@example.com", "imap.example.com");
    let local = vec![synced("a", config.clone(), "rec-1", 4)];
    let remote = list(vec![record("rec-1", 4, config)], vec![]);
    assert!(reconcile(&local, &remote).is_empty());
}

#[test]
fn the_server_moved_and_this_device_did_not_is_an_update_to_offer() {
    let config = imap("someone@example.com", "imap.example.com");
    let local = vec![synced("a", config, "rec-1", 4)];
    let elsewhere = imap("someone@example.com", "mail.example.com");
    let remote = list(vec![record("rec-1", 5, elsewhere)], vec![]);

    match &reconcile(&local, &remote)[..] {
        [
            Decision::UpdateAvailable {
                account_id,
                current,
            },
        ] => {
            assert_eq!(account_id, "a");
            assert_eq!(current.version, 5);
        }
        other => panic!("expected an update to offer, got {other:?}"),
    }
}

#[test]
fn this_device_moved_and_the_server_did_not_is_a_push() {
    let config = imap("someone@example.com", "imap.example.com");
    let mut local = vec![synced("a", config.clone(), "rec-1", 4)];
    // Somebody corrected the hostname here.
    local[0].config = imap("someone@example.com", "corrected.example.com");
    let remote = list(vec![record("rec-1", 4, config)], vec![]);

    match &reconcile(&local, &remote)[..] {
        [
            Decision::Push {
                account_id,
                id,
                version,
            },
        ] => {
            assert_eq!(
                (account_id.as_str(), id.as_str(), *version),
                ("a", "rec-1", 4)
            );
        }
        other => panic!("expected a push, got {other:?}"),
    }
}

/// The case the whole three-way base exists for.
///
/// Both sides moved. Told apart from "the server moved" only by remembering what the config looked
/// like at the version last synced, and if it were not, applying the server's would throw away an
/// edit made here and call it an update.
#[test]
fn both_sides_moved_is_a_conflict_and_never_an_update() {
    let base = imap("someone@example.com", "imap.example.com");
    let mut local = vec![synced("a", base, "rec-1", 4)];
    local[0].config = imap("someone@example.com", "on-this-network.example.com");
    let remote = list(
        vec![record(
            "rec-1",
            5,
            imap("someone@example.com", "elsewhere.example.com"),
        )],
        vec![],
    );

    match &reconcile(&local, &remote)[..] {
        [
            Decision::Conflict {
                account_id,
                current,
            },
        ] => {
            assert_eq!(account_id, "a");
            assert_eq!(current.version, 5);
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[test]
fn an_account_the_service_has_never_seen_is_uploaded() {
    let local = vec![LocalAccount {
        account_id: "a".to_owned(),
        config: imap("someone@example.com", "imap.example.com"),
        sync: None,
    }];
    let remote = list(vec![], vec![]);
    assert_eq!(
        reconcile(&local, &remote),
        vec![Decision::Upload {
            account_id: "a".to_owned()
        }]
    );
}

/// Two devices set the same mailbox up independently, and the hostnames do not match.
///
/// This is the duplicate the opaque id was chosen to avoid, and it is avoided only if matching
/// ignores the host, which is exactly the field autodetect races to different answers for.
#[test]
fn an_account_the_service_already_holds_is_adopted_rather_than_uploaded_twice() {
    let local = vec![LocalAccount {
        account_id: "a".to_owned(),
        config: imap("someone@example.com", "imap.example.com"),
        sync: None,
    }];
    let remote = list(
        vec![record(
            "rec-1",
            2,
            imap("someone@example.com", "mail.example.com"),
        )],
        vec![],
    );

    match &reconcile(&local, &remote)[..] {
        [
            Decision::Adopt {
                account_id,
                current,
            },
        ] => {
            assert_eq!(account_id, "a");
            assert_eq!(current.id, "rec-1");
        }
        other => panic!("expected an adoption, got {other:?}"),
    }
}

#[test]
fn an_account_from_another_device_is_offered_and_never_applied() {
    let remote = list(
        vec![record(
            "rec-9",
            1,
            imap("work@example.com", "imap.example.com"),
        )],
        vec![],
    );
    match &reconcile(&[], &remote)[..] {
        [Decision::Offer { current }] => assert_eq!(current.id, "rec-9"),
        other => panic!("expected an offer, got {other:?}"),
    }
}

#[test]
fn an_account_removed_elsewhere_is_a_question_rather_than_a_removal() {
    // Removing an account from a phone to keep work mail off it is a local decision. Propagating
    // it silently would destroy a working setup on a machine nobody touched.
    let config = imap("someone@example.com", "imap.example.com");
    let local = vec![synced("a", config, "rec-1", 4)];
    let remote = list(vec![], vec![gone("rec-1", 5)]);
    assert_eq!(
        reconcile(&local, &remote),
        vec![Decision::RemovedElsewhere {
            account_id: "a".to_owned()
        }]
    );
}

/// A tombstone beats an offer in the same answer.
///
/// Without the ordering, an account somebody deleted on their laptop would be offered straight
/// back to them on their phone, which is the "I deleted this three times" bug, and it looks like
/// the app ignoring them rather than two rules disagreeing.
#[test]
fn an_account_deleted_elsewhere_is_not_offered_back_in_the_same_pass() {
    let remote = list(
        vec![record(
            "rec-9",
            1,
            imap("work@example.com", "imap.example.com"),
        )],
        vec![gone("rec-9", 2)],
    );
    assert!(reconcile(&[], &remote).is_empty());
}

/// Detached means fully local: no pull, and no push either.
///
/// Pushing would go on feeding the other devices a hostname that is right only on this network,
/// which is the noise detaching was meant to end.
#[test]
fn a_detached_account_neither_pulls_nor_pushes() {
    let base = imap("someone@example.com", "imap.example.com");
    let mut local = vec![synced("a", base, "rec-1", 4)];
    local[0].config = imap("someone@example.com", "on-this-network.example.com");
    local[0].sync.as_mut().unwrap().detached = true;
    let remote = list(
        vec![record(
            "rec-1",
            9,
            imap("someone@example.com", "elsewhere.example.com"),
        )],
        vec![],
    );
    assert!(
        reconcile(&local, &remote).is_empty(),
        "a detached account is invisible in both directions"
    );
}

/// And its record is not offered back as if this device had never seen it.
#[test]
fn a_detached_accounts_record_is_not_offered_as_a_new_one() {
    let config = imap("someone@example.com", "imap.example.com");
    let mut local = vec![synced("a", config.clone(), "rec-1", 4)];
    local[0].sync.as_mut().unwrap().detached = true;
    let remote = list(vec![record("rec-1", 4, config)], vec![]);
    assert!(reconcile(&local, &remote).is_empty());
}

/// A `since` delta carries only what changed, so an unchanged account is simply absent.
///
/// Reading that absence as a deletion would have every delta pull propose removing every account
/// that happened not to change: the loudest possible failure from the quietest possible cause.
#[test]
fn an_account_missing_from_a_delta_is_not_treated_as_deleted() {
    let config = imap("someone@example.com", "imap.example.com");
    let local = vec![synced("a", config, "rec-1", 4)];
    let remote = list(vec![], vec![]);
    assert!(reconcile(&local, &remote).is_empty());
}

#[test]
fn a_local_edit_still_pushes_when_the_delta_did_not_carry_the_record() {
    let base = imap("someone@example.com", "imap.example.com");
    let mut local = vec![synced("a", base, "rec-1", 4)];
    local[0].config = imap("someone@example.com", "corrected.example.com");
    let remote = list(vec![], vec![]);
    match &reconcile(&local, &remote)[..] {
        [Decision::Push { version, .. }] => assert_eq!(*version, 4),
        other => panic!("expected a push, got {other:?}"),
    }
}

#[test]
fn several_accounts_are_each_decided_on_their_own() {
    let settled = imap("a@example.com", "imap.example.com");
    let moved = imap("b@example.com", "imap.example.com");
    let mut local = vec![
        synced("a", settled.clone(), "rec-1", 4),
        synced("b", moved.clone(), "rec-2", 4),
        LocalAccount {
            account_id: "c".to_owned(),
            config: imap("c@example.com", "imap.example.com"),
            sync: None,
        },
    ];
    local[1].config = imap("b@example.com", "corrected.example.com");
    let remote = list(
        vec![
            record("rec-1", 4, settled),
            record("rec-2", 4, moved),
            record("rec-9", 1, imap("d@example.com", "imap.example.com")),
        ],
        vec![],
    );

    let decisions = reconcile(&local, &remote);
    assert_eq!(decisions.len(), 3, "{decisions:?}");
    assert!(
        decisions
            .iter()
            .any(|d| matches!(d, Decision::Push { account_id, .. } if account_id == "b"))
    );
    assert!(
        decisions
            .iter()
            .any(|d| matches!(d, Decision::Upload { account_id } if account_id == "c"))
    );
    assert!(
        decisions
            .iter()
            .any(|d| matches!(d, Decision::Offer { current } if current.id == "rec-9"))
    );
}
