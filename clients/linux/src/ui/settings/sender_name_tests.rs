//! What the "your name" row draws, and which edits are changes.
//!
//! The row has no Save button: it commits on Return and on losing focus, neither of which AT-SPI
//! can drive, so the acceptance suite cannot reach it and these assertions are the only ones that
//! do.
//!
//! Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::{AccountPageIntent, AccountSenderName, ControlState};

use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, mailbox::tests::rendered_labels},
};

const ACCOUNT: &str = "acct-1";

fn name(name: &str, editable: bool) -> AccountSenderName {
    AccountSenderName {
        name: name.to_owned(),
        editable,
        control: ControlState {
            enabled: editable,
            running: false,
        },
    }
}

fn group_for(name: &AccountSenderName, sender: &relm4::Sender<AppInput>) -> adw::PreferencesGroup {
    super::group(ACCOUNT, name, sender)
}

fn field(group: &adw::PreferencesGroup) -> Option<gtk::Entry> {
    crate::ui::setup_widget_tests::descendants::<gtk::Entry>(group.upcast_ref::<gtk::Widget>())
        .into_iter()
        .next()
}

/// The name the change asks for, or a panic naming what came instead.
fn asked(receiver: &relm4::Receiver<AppInput>) -> String {
    match receiver.recv_sync() {
        Some(AppInput::Accounts(AccountsInput::Change(AccountPageIntent::SetSenderName {
            account,
            name,
        }))) => {
            assert_eq!(
                account, ACCOUNT,
                "the edit names the account it was made on"
            );
            name
        }
        other => panic!("the edit asks the core for the name, got {other:?}"),
    }
}

/// The field is seeded with the stored name, and Return asks for what was typed.
///
/// Seeding matters as much as the write: a field that opened empty over a name already set
/// would read as *no name*, and the obvious next action, typing one, would be the user
/// restoring what they already had.
pub(crate) fn the_field_shows_the_stored_name_and_return_asks_for_the_typed_one() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&name("Ada Lovelace", true), &sender);
    let entry = field(&group).expect("an editable account offers a field");
    assert_eq!(
        entry.text(),
        "Ada Lovelace",
        "the field opens on what is set"
    );

    entry.set_text("Ada King");
    entry.emit_activate();
    assert_eq!(asked(&receiver), "Ada King");
}

/// Nothing is sent unchanged, so tabbing through the page costs no write.
///
/// The row commits on losing focus as well as on Return, so every pass through the page would
/// otherwise re-assert the name: a provider round trip and a redraw that reseeds the entry under
/// whoever is typing in it.
pub(crate) fn re_asserting_the_stored_name_asks_for_nothing() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&name("Ada Lovelace", true), &sender);
    field(&group)
        .expect("an editable account offers a field")
        .emit_activate();
    // A sentinel behind it, because the channel has no non-blocking read: anything the commit
    // emitted is already queued, so the sentinel arriving first is the assertion.
    sender.emit(AppInput::CollectNewMail);

    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::CollectNewMail)),
        "an unchanged name is not an edit"
    );
}

/// An empty name is a real answer: it clears, and is not confused with "unchanged".
pub(crate) fn clearing_the_field_asks_to_send_as_the_address_alone() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = group_for(&name("Ada Lovelace", true), &sender);
    let entry = field(&group).expect("an editable account offers a field");
    entry.set_text("");
    entry.emit_activate();
    assert!(
        asked(&receiver).is_empty(),
        "clearing is an answer, not a no-op"
    );
}

/// Where the provider owns the name, the page states it and offers no editor.
///
/// An entry nobody can make stick is worse than none: the edit reads as accepted, the next
/// snapshot puts the directory's name back, and the app looks like it lost the change.
pub(crate) fn a_provider_held_name_is_shown_without_a_field() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let group = group_for(&name("Directory Name", false), &sender);

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
        "and the page says why there is no field: {shown:?}"
    );
}

/// While a change runs on the page, the field is held.
pub(crate) fn the_field_is_held_while_a_change_runs() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let held = AccountSenderName {
        control: ControlState {
            enabled: false,
            running: false,
        },
        ..name("Ada Lovelace", true)
    };
    let group = group_for(&held, &sender);
    assert!(!field(&group).expect("still drawn").is_sensitive());
}
