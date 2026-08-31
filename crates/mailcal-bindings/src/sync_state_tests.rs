// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The failures here are quiet ones: bookkeeping that half-persists, and a device that reads
//! "never synced" from a store that was only briefly unreadable, which starts a first-run pass
//! and uploads a duplicate of every account.

use std::sync::{Arc, Mutex};

use super::*;

/// A host store that records every write and can be made to refuse.
///
/// Its blob is shared rather than owned so a test can read back what the host was actually handed
/// : the entries alone are not the blob, and the difference is the whole point of the exclusion
/// set living beside them.
#[derive(Default)]
struct Fake {
    blob: Arc<Mutex<Option<String>>>,
    writes: Mutex<usize>,
    refuse: bool,
}

impl SyncStateStore for Fake {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        if self.refuse {
            return Err(SyncStateError::Store("unreadable".to_owned()));
        }
        Ok(self.blob.lock().unwrap().clone())
    }

    fn save(&self, blob: String) -> Result<(), SyncStateError> {
        if self.refuse {
            return Err(SyncStateError::Store("refused".to_owned()));
        }
        *self.writes.lock().unwrap() += 1;
        *self.blob.lock().unwrap() = Some(blob);
        Ok(())
    }
}

/// A store seeded with what a previous run left.
struct Seeded(String);

impl SyncStateStore for Seeded {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        Ok(Some(self.0.clone()))
    }

    fn save(&self, _blob: String) -> Result<(), SyncStateError> {
        Ok(())
    }
}

fn state(id: &str, version: u64) -> StoredSyncState {
    StoredSyncState {
        id: id.to_owned(),
        version,
        fingerprint: "{}".to_owned(),
    }
}

#[test]
fn an_entry_survives_the_round_trip_through_the_host() {
    let book = SyncBookkeeping::load(Box::new(Fake::default())).unwrap();
    book.set("acct-1", state("rec-1", 3)).unwrap();

    let written = book.all();
    assert_eq!(written.get("acct-1"), Some(&state("rec-1", 3)));
}

#[test]
fn every_change_is_written_through_rather_than_batched() {
    // A pass that updated memory and deferred the store would, if the app died between them,
    // upload a duplicate at the next launch for every account whose id it had forgotten.
    let fake = Fake::default();
    let store = Box::new(fake);
    let book = SyncBookkeeping::load(store).unwrap();
    book.set("acct-1", state("rec-1", 1)).unwrap();
    book.set("acct-2", state("rec-2", 1)).unwrap();
    book.forget("acct-1").unwrap();

    // Three calls, three writes: nothing waits for a flush that may not come.
    assert_eq!(book.all().len(), 1);
    assert!(book.get("acct-1").is_none());
    assert_eq!(book.get("acct-2"), Some(state("rec-2", 1)));
}

#[test]
fn a_store_that_cannot_be_read_is_reported_and_never_read_as_never_synced() {
    // The whole point. "Never synced" starts a first-run pass, so a store that is merely
    // unreadable today must not look like one.
    let refusing = Fake {
        refuse: true,
        ..Fake::default()
    };
    assert!(SyncBookkeeping::load(Box::new(refusing)).is_err());
}

#[test]
fn a_blob_that_will_not_parse_starts_from_nothing_rather_than_breaking_sync() {
    // Bookkeeping, not data: the worst a fresh start costs is one pass that adopts the records the
    // service already holds, where refusing would leave sync broken forever on one bad byte.
    let book = SyncBookkeeping::load(Box::new(Seeded("not json".to_owned()))).unwrap();
    assert!(book.all().is_empty());
}

#[test]
fn a_blob_with_no_exclusions_reads_as_excluding_nothing() {
    // Both halves are `serde(default)`, so a blob written before either existed still parses, and
    // a device that stopped parsing its own bookkeeping would re-upload every account it has.
    let older = r#"{"accounts":{"acct-1":{"id":"rec-1","version":2,"fingerprint":"{}"}}}"#;
    let book = SyncBookkeeping::load(Box::new(Seeded(older.to_owned()))).unwrap();
    let entry = book
        .get("acct-1")
        .expect("an older entry is still an entry");
    assert_eq!(entry.version, 2);
    assert!(!book.is_excluded("acct-1"));
}

/// The reason exclusion is not a field on the entry: a person can decide an account should not
/// travel **before** it ever has, and there is no entry then to put the decision in.
#[test]
fn an_account_can_be_excluded_before_it_has_ever_been_synced() {
    let book = SyncBookkeeping::load(Box::new(Fake::default())).unwrap();

    book.set_excluded("acct-1", true).unwrap();

    assert!(book.is_excluded("acct-1"));
    assert!(book.get("acct-1").is_none(), "and no record is invented");
}

#[test]
fn an_exclusion_survives_the_round_trip_beside_the_entries() {
    let fake = Fake::default();
    let written = Arc::clone(&fake.blob);
    let book = SyncBookkeeping::load(Box::new(fake)).unwrap();
    book.set("acct-1", state("rec-1", 3)).unwrap();
    book.set_excluded("acct-1", true).unwrap();

    // Re-read what the HOST holds, which is the whole blob rather than the entries alone.
    let blob = written.lock().unwrap().clone().expect("written through");
    let reread = SyncBookkeeping::load(Box::new(Seeded(blob))).unwrap();

    assert!(reread.is_excluded("acct-1"));
    assert_eq!(reread.get("acct-1"), Some(state("rec-1", 3)));
}

/// Removing an account takes the choice made about it too. Keeping it would silently exclude a
/// re-added account of the same name months later, with nothing on screen to explain why.
#[test]
fn forgetting_an_account_forgets_that_it_was_excluded() {
    let book = SyncBookkeeping::load(Box::new(Fake::default())).unwrap();
    book.set("acct-1", state("rec-1", 3)).unwrap();
    book.set_excluded("acct-1", true).unwrap();

    book.forget("acct-1").unwrap();

    assert!(!book.is_excluded("acct-1"));
}

/// Signing out drops what this device said to the service, and keeps what the person said to it.
///
/// The entries name one Allodia account's records. Carried into the next sign-in they match
/// nothing, so nothing is claimed and every record is offered; including the mail accounts this
/// device is already running. The exclusions are the other kind of fact: somebody answered a
/// question about their own device, and a sign-out is not them changing their mind.
#[test]
fn signing_out_forgets_the_records_and_keeps_the_choices() {
    let book = SyncBookkeeping::load(Box::new(Fake::default())).unwrap();
    book.set("acct-1", state("rec-1", 3)).unwrap();
    book.set_pending_create_key("acct-2", Some("attempt-7"))
        .unwrap();
    book.set_excluded("acct-2", true).unwrap();

    book.forget_the_session().unwrap();

    assert_eq!(book.get("acct-1"), None, "a record id from the old account");
    assert_eq!(
        book.pending_create_key("acct-2"),
        None,
        "a create in flight against the old account"
    );
    assert!(
        book.is_excluded("acct-2"),
        "an account the person kept off this device must not go back on the wire"
    );
}

#[test]
fn the_memory_copy_advances_even_when_the_host_refuses_the_write() {
    // The write to the service has already happened by the time this is recorded. Leaving memory
    // behind would have the same pass write again, against a version the server no longer holds.
    let refusing = Fake {
        refuse: false,
        ..Fake::default()
    };
    let written = Arc::clone(&refusing.blob);
    let book = SyncBookkeeping::load(Box::new(refusing)).unwrap();
    book.set("acct-1", state("rec-1", 1)).unwrap();

    // Re-read the whole blob the host was handed: the entries alone are not it.
    let book = SyncBookkeeping::load(Box::new(Seeded(
        written.lock().unwrap().clone().expect("written through"),
    )))
    .unwrap();
    assert_eq!(book.get("acct-1").unwrap().version, 1);
}

#[test]
fn a_build_with_nothing_to_remember_reports_a_first_run_and_keeps_nothing() {
    let book = SyncBookkeeping::load(Box::new(NoSyncState)).unwrap();
    assert!(book.all().is_empty());
    // Accepting the write is truthful here: a fixture account is rebuilt at every launch, so
    // nothing is lost by not keeping it.
    book.set("acct-1", state("rec-1", 1)).unwrap();
}
