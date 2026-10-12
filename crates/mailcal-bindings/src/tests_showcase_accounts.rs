//! The showcase's **Settings → Accounts**: that both seeded accounts are listed, each used for
//! what its seed binds, and that the account-less boot lists none.
//!
//! Every client draws that category from `accounts_snapshot` alone (`docs/accounts.md` rule 11),
//! so a showcase that registered nothing photographs an empty category, and the client suites
//! that open an account's page could not run on the showcase at all.

use std::sync::mpsc;

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

fn state(entry: &AccountEntry, capability: AccountCapability) -> CapabilityState {
    entry
        .uses
        .iter()
        .find(|used| used.capability == capability)
        .map_or(CapabilityState::Off, |used| used.state)
}

#[test]
fn showcase_lists_both_seeded_accounts_used_for_what_their_seeds_bind() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let listed = app.accounts_snapshot().accounts;
    // The same accounts, in the same order, as the mail settings the page draws beneath its own
    // rows: an entry whose id the mail settings do not know would be a page without its mail.
    let mail: Vec<String> = app
        .sync_settings()
        .accounts
        .into_iter()
        .map(|account| account.account_id)
        .collect();
    let ids: Vec<&str> = listed.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, mail, "Settings → Accounts lists both seeded accounts");

    let (primary, secondary) = (&listed[0], &listed[1]);
    for entry in &listed {
        assert_eq!(
            entry.address, entry.id,
            "a showcase account is read by its address"
        );
        assert!(
            entry.endpoints.is_none(),
            "{}: no servers to edit, so nothing on its page dials",
            entry.address
        );
    }
    for capability in [
        AccountCapability::Mail,
        AccountCapability::Calendar,
        AccountCapability::Contacts,
    ] {
        assert_eq!(
            state(primary, capability),
            CapabilityState::On,
            "the primary seed binds mail, a calendar and contacts: {capability:?}"
        );
    }
    assert_eq!(
        state(secondary, AccountCapability::Mail),
        CapabilityState::On
    );
    assert_eq!(
        state(secondary, AccountCapability::Contacts),
        CapabilityState::On
    );
    assert_eq!(
        state(secondary, AccountCapability::Calendar),
        CapabilityState::Off,
        "the second seed binds no calendar, so its page does not claim one"
    );
}

#[test]
fn showcase_first_run_lists_no_account() {
    // An empty snapshot is what sends a client back to first-run setup, which is the screen this
    // boot exists to photograph.
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase_first_run(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    assert!(app.accounts_snapshot().accounts.is_empty());
}
