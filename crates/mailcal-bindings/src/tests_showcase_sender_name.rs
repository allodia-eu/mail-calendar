//! The showcase dataset's **sender names**: that each seeded account really sends under the name
//! its own mail addresses, and that every locale's seed carries one.
//!
//! Split from `tests_showcase.rs` for the 500-line limit, and because these fail in a way a
//! screenshot cannot show: an account with no name is a legitimate state, so a capture of the
//! composer's From reading a bare address looks exactly like a capture of one that works
//! (`docs/sending.md`).

use std::sync::mpsc;

use super::*;
use crate::{
    tests::{ChannelObserver, NullLogger},
    tests_showcase::ALL_SHOWCASE_LOCALES,
};

#[test]
fn showcase_seeds_the_name_each_account_sends_under() {
    // Without this both accounts send as a bare address, and the composer's From would show one
    // for the very person the seeded Inbox greets by name. Like the signature library, it is built
    // by *calling* a use case at boot, so nothing in the seed data says whether it actually
    // happened (`docs/sending.md`).
    let (tx, rx) = mpsc::channel();
    drop(rx);
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let accounts = app.sync_settings().accounts;
    assert_eq!(accounts.len(), 2, "both showcase accounts carry a card");
    for account in &accounts {
        assert!(
            !account.sender_name.is_empty(),
            "{} sends under no name",
            account.email
        );
        assert!(
            account.sender_name_editable,
            "{}'s provider keeps no name of its own, so the card offers the field",
            account.email
        );
    }
    // Read against the seed rather than a literal: the name an account sends under is the same
    // one its own mail addresses, so a seed that renamed this person in its messages and not in
    // its account would put two people on one screenshot.
    let seeded = crate::showcase_data::primary(ShowcaseLocale::En, time::OffsetDateTime::now_utc())
        .sender_name;
    let names: Vec<&str> = accounts.iter().map(|a| a.sender_name.as_str()).collect();
    assert!(
        names.contains(&seeded.as_str()),
        "the primary card carries the name its own seed gave it ({seeded}), got {names:?}"
    );
}

#[test]
fn every_showcase_locale_seeds_a_name_for_both_accounts() {
    // `tests_showcase.rs` checks that a locale added later carries the same messages and folders
    // as English. That says nothing about the account's own name: a seed that left `me.0` out
    // would ship a nameless account in that language alone, visible only in a capture nobody
    // compares.
    let now = time::OffsetDateTime::now_utc();
    for locale in ALL_SHOWCASE_LOCALES {
        for seed in [
            crate::showcase_data::primary(locale, now),
            crate::showcase_data::secondary(locale, now),
        ] {
            assert!(
                !seed.sender_name.trim().is_empty(),
                "{locale:?}: {} seeds no sender name",
                seed.identity
            );
        }
    }
}
