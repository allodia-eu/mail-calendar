//! What the "your name" row draws, and which edits are changes.
//!
//! The row is the one control on the account card whose effect a stranger can see, and it has
//! no Save button: it commits on Return and on losing focus, neither of which AT-SPI can drive,
//! so the acceptance suite cannot reach it and these assertions are the only ones that do.
//!
//! Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::{AccountSyncRow, SyncStrategyKind};

use crate::{
    l10n,
    ui::{AppInput, mailbox::tests::rendered_labels},
};

const ACCOUNT: &str = "acct-1";

fn account_row(sender_name: &str, editable: bool) -> AccountSyncRow {
    AccountSyncRow {
        account_id: ACCOUNT.to_owned(),
        email: "someone@example.test".to_owned(),
        sender_name: sender_name.to_owned(),
        sender_name_editable: editable,
        idle_supported: true,
        strategy: SyncStrategyKind::Push,
        poll_interval_mins: 15,
        sync_depth_months: 3,
        message_size_limit_mb: 0,
        at_push_limit: false,
        folders: Vec::new(),
    }
}

fn group_for(row: &AccountSyncRow, sender: &relm4::Sender<AppInput>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    super::add_row(&group, row, sender);
    group
}

fn field(group: &adw::PreferencesGroup) -> Option<gtk::Entry> {
    crate::ui::setup_widget_tests::descendants::<gtk::Entry>(group.upcast_ref::<gtk::Widget>())
        .into_iter()
        .next()
}

/// The field is seeded with the stored name, and Return asks for what was typed.
///
/// Seeding matters as much as the write: a field that opened empty over a name already set
/// would read as *no name*, and the obvious next action, typing one, would be the user
/// restoring what they already had.
pub(crate) fn the_field_shows_the_stored_name_and_return_asks_for_the_typed_one() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&account_row("Ada Lovelace", true), &sender);
    let entry = field(&group).expect("an editable account offers a field");
    assert_eq!(
        entry.text(),
        "Ada Lovelace",
        "the field opens on what is set"
    );

    entry.set_text("Ada King");
    entry.emit_activate();
    match receiver.recv_sync() {
        Some(AppInput::SetAccountSenderName { account, name }) => {
            assert_eq!(
                account, ACCOUNT,
                "the edit names the account it was made on"
            );
            assert_eq!(name, "Ada King");
        }
        other => panic!("Return commits the typed name, got {other:?}"),
    }
}

/// Nothing is sent unchanged, so tabbing through the card costs no write.
///
/// The row commits on losing focus as well as on Return, so every pass through the card would
/// otherwise re-assert the name: a provider round trip and a snapshot rebuild that reseeds the
/// entry under whoever is typing in it.
pub(crate) fn re_asserting_the_stored_name_asks_for_nothing() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&account_row("Ada Lovelace", true), &sender);
    field(&group)
        .expect("an editable account offers a field")
        .emit_activate();
    // A sentinel behind it, because the channel has no non-blocking read: anything the commit
    // emitted is already queued, so the sentinel arriving first is the assertion.
    sender.emit(AppInput::ReadAccountsSynced);

    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::ReadAccountsSynced)),
        "an unchanged name is not an edit"
    );
}

/// An empty name is a real answer: it clears, and is not confused with "unchanged".
pub(crate) fn clearing_the_field_asks_to_send_as_the_address_alone() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&account_row("Ada Lovelace", true), &sender);
    let entry = field(&group).expect("an editable account offers a field");
    entry.set_text("");
    entry.emit_activate();

    match receiver.recv_sync() {
        Some(AppInput::SetAccountSenderName { account, name }) => {
            assert_eq!(account, ACCOUNT);
            assert!(name.is_empty(), "clearing is an answer, not a no-op");
        }
        other => panic!("an emptied field clears the name, got {other:?}"),
    }
}

/// Where the provider owns the name, the card states it and offers no editor.
///
/// An entry nobody can make stick is worse than none: the edit reads as accepted, the next
/// snapshot puts the directory's name back, and the app looks like it lost the change.
pub(crate) fn a_provider_held_name_is_shown_without_a_field() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let group = group_for(&account_row("Directory Name", false), &sender);

    assert!(
        field(&group).is_none(),
        "no editor for a name we cannot set"
    );
    let shown = rendered_labels(group.upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|text| text == "Directory Name"),
        "the name is still shown: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::settings_sender_name_managed()),
        "and the card says why there is no field: {shown:?}"
    );
}

/// A name nobody has set yet falls back to the address, rather than to an empty line.
pub(crate) fn a_provider_held_account_with_no_name_shows_its_address() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let group = group_for(&account_row("", false), &sender);

    let shown = rendered_labels(group.upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|text| text == "someone@example.test"),
        "an unnamed managed account shows its address: {shown:?}"
    );
}
