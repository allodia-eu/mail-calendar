//! The kept rows: a read changes the page only when it found something else, an older read never
//! replaces a newer one, one read is out at a time, and a removed account keeps nothing.

use mailcal_viewmodel::{AccountSyncRow, SyncStrategyKind};

use super::MailRows;

const ID: &str = "alice@example.org@localhost";

fn row(minutes: u16) -> AccountSyncRow {
    AccountSyncRow {
        account_id: ID.to_owned(),
        email: "alice@example.org".to_owned(),
        sender_name: String::new(),
        sender_name_editable: true,
        idle_supported: false,
        strategy: SyncStrategyKind::Poll,
        poll_interval_mins: minutes,
        sync_depth_months: 3,
        message_size_limit_mb: 25,
        at_push_limit: false,
        folders: Vec::new(),
    }
}

#[test]
fn a_read_changes_the_page_only_when_it_found_something_else() {
    let rows = MailRows::default();
    assert_eq!(rows.row(ID), None, "nothing before the first read");
    let read = rows.begin();
    assert!(rows.found(ID, read, Some(row(15))));
    assert_eq!(rows.row(ID), Some(row(15)));
    let read = rows.begin();
    assert!(!rows.found(ID, read, Some(row(15))), "the same row again");

    let other = MailRows::default();
    let read = other.begin();
    assert!(
        !other.found(ID, read, None),
        "an account not used for mail is drawn as before its first read"
    );
}

#[test]
fn an_older_read_never_replaces_a_newer_one() {
    let rows = MailRows::default();
    let before_the_change = rows.begin();
    let after_the_change = rows.begin();
    assert!(rows.found(ID, after_the_change, Some(row(30))));
    assert!(!rows.found(ID, before_the_change, Some(row(15))));
    assert_eq!(rows.row(ID), Some(row(30)));
}

#[test]
fn one_read_is_out_at_a_time_and_an_ask_meanwhile_reads_once_more() {
    let rows = MailRows::default();
    assert!(rows.ask(ID));
    assert!(!rows.ask(ID), "one is out");
    assert!(!rows.ask(ID));
    assert!(rows.landed(ID), "asked again meanwhile: once more");
    assert!(!rows.landed(ID), "nothing asked since");
    assert!(rows.ask(ID), "the next ask starts one");
}

#[test]
fn a_removed_account_keeps_nothing_a_read_begun_before_brings_back() {
    let rows = MailRows::default();
    let read = rows.begin();
    assert!(rows.found(ID, read, Some(row(15))));
    let late = rows.begin();
    rows.forget(ID);
    assert_eq!(rows.row(ID), None);
    assert!(!rows.found(ID, late, Some(row(15))));
    assert_eq!(rows.row(ID), None);
}
