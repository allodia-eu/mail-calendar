//! What the Shared mailboxes section says for what the core decided, without a display.

use mailcal_bindings::{AccountSharedMailbox, ControlState, SharedMailboxesNote};

use super::{note, subtitle};
use crate::l10n;

fn mailbox(label: Option<&str>, withdrawn: bool) -> AccountSharedMailbox {
    AccountSharedMailbox {
        handle: "f".to_owned(),
        title: "support@example.org".to_owned(),
        label: label.map(str::to_owned),
        withdrawn,
        on: true,
        control: ControlState {
            enabled: true,
            running: false,
        },
    }
}

#[test]
fn a_withdrawn_mailbox_says_so_in_place_of_its_label() {
    assert_eq!(
        subtitle(&mailbox(Some("Support Team"), false)),
        Some("Support Team")
    );
    assert_eq!(subtitle(&mailbox(None, false)), None);
    assert_eq!(
        subtitle(&mailbox(Some("Support Team"), true)),
        Some(l10n::settings_account_shared_withdrawn())
    );
}

#[test]
fn each_line_under_the_switches_is_said_in_words() {
    assert_eq!(
        note(SharedMailboxesNote::NoneShared),
        l10n::settings_account_shared_none()
    );
    assert_eq!(
        note(SharedMailboxesNote::ByAddress),
        l10n::settings_account_shared_by_address()
    );
    assert_eq!(
        note(SharedMailboxesNote::NotConnected),
        l10n::settings_account_shared_not_connected()
    );
    assert_eq!(
        note(SharedMailboxesNote::Unreachable),
        l10n::settings_account_shared_unreachable()
    );
}
