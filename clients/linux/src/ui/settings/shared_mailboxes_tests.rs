//! What the Shared mailboxes section draws for each answer, without a display.

use mailcal_bindings::{SharedMailboxEntry, SharedMailboxSupport, SharedMailboxesView};

use super::{SharedRow, note, row};
use crate::l10n;

fn mailbox(address: Option<&str>, name: &str, selected: bool, listed: bool) -> SharedMailboxEntry {
    SharedMailboxEntry {
        handle: "f".to_owned(),
        name: name.to_owned(),
        address: address.map(str::to_owned),
        selected,
        listed,
    }
}

#[test]
fn a_mailbox_is_named_by_its_address_with_the_servers_label_beside_it_when_it_differs() {
    assert_eq!(
        row(&mailbox(
            Some("support@example.org"),
            "Support Team",
            true,
            true
        )),
        SharedRow {
            title: "support@example.org".to_owned(),
            subtitle: Some("Support Team".to_owned()),
            active: true,
        }
    );
    assert_eq!(
        row(&mailbox(
            Some("support@example.org"),
            "support@example.org",
            false,
            true
        ))
        .subtitle,
        None
    );
    assert_eq!(row(&mailbox(None, "team", false, true)).title, "team");
}

#[test]
fn a_chosen_mailbox_the_server_stopped_listing_says_so() {
    assert_eq!(
        row(&mailbox(
            Some("support@example.org"),
            "Support",
            true,
            false
        ))
        .subtitle,
        Some(l10n::settings_account_shared_withdrawn().to_owned())
    );
}

#[test]
fn the_line_under_the_switches_says_why_there_is_nothing_more_to_choose() {
    let view = |support, mailboxes| SharedMailboxesView { support, mailboxes };
    assert_eq!(
        note(&view(SharedMailboxSupport::Listed, Vec::new())),
        Some(l10n::settings_account_shared_none())
    );
    assert_eq!(
        note(&view(
            SharedMailboxSupport::Listed,
            vec![mailbox(None, "team", false, true)]
        )),
        None
    );
    assert_eq!(
        note(&view(SharedMailboxSupport::NotConnected, Vec::new())),
        Some(l10n::settings_account_shared_not_connected())
    );
    assert_eq!(
        note(&view(SharedMailboxSupport::Unreachable, Vec::new())),
        Some(l10n::settings_account_shared_unreachable())
    );
}
