//! What the Shared mailboxes and calendars section says for what the core decided, without a
//! display.

use mailcal_bindings::{AccountSharedStore, ControlState, SharedStoreHolds, SharedStoresNote};

use super::{holds, note, subtitle};
use crate::l10n;

fn store(label: Option<&str>, holds: SharedStoreHolds, withdrawn: bool) -> AccountSharedStore {
    AccountSharedStore {
        handle: "f".to_owned(),
        title: "support@example.org".to_owned(),
        label: label.map(str::to_owned),
        holds,
        withdrawn,
        on: true,
        control: ControlState {
            enabled: true,
            running: false,
        },
    }
}

#[test]
fn a_switch_says_its_label_and_what_it_holds_and_a_withdrawn_one_says_so_instead() {
    let both = SharedStoreHolds::MailAndCalendar;
    assert_eq!(
        subtitle(&store(Some("Support Team"), both, false)),
        format!(
            "Support Team · {}",
            l10n::settings_account_shared_holds_both()
        )
    );
    assert_eq!(
        subtitle(&store(None, SharedStoreHolds::Calendar, false)),
        l10n::settings_account_shared_holds_calendar()
    );
    assert_eq!(
        subtitle(&store(Some("Support Team"), both, true)),
        l10n::settings_account_shared_withdrawn()
    );
}

#[test]
fn what_a_store_holds_is_said_in_words() {
    assert_eq!(
        holds(SharedStoreHolds::Mail),
        l10n::settings_account_shared_holds_mail()
    );
    assert_eq!(
        holds(SharedStoreHolds::Calendar),
        l10n::settings_account_shared_holds_calendar()
    );
    assert_eq!(
        holds(SharedStoreHolds::MailAndCalendar),
        l10n::settings_account_shared_holds_both()
    );
}

#[test]
fn each_line_under_the_switches_is_said_in_words() {
    assert_eq!(
        note(SharedStoresNote::NoneShared),
        l10n::settings_account_shared_none()
    );
    assert_eq!(
        note(SharedStoresNote::ByAddress),
        l10n::settings_account_shared_by_address()
    );
    assert_eq!(
        note(SharedStoresNote::NotConnected),
        l10n::settings_account_shared_not_connected()
    );
    assert_eq!(
        note(SharedStoresNote::Unreachable),
        l10n::settings_account_shared_unreachable()
    );
}
