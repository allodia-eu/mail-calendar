//! What the per-account sharing control draws, and which presses are changes.
//!
//! Rendering is asserted on the **rendered** labels rather than on `ActionRow::title()`, which
//! reads back the string it was handed whatever became of the label; see
//! [`crate::ui::mailbox::tests::rendered_labels`]. Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::{
    AccountPageIntent, AccountSharing, AllodiaAccountSyncMode, AllodiaSyncFailure, ControlState,
};

use super::synced_group;
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        controls::tests::spins,
        mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
    },
};

const ACCOUNT: &str = "someone@example.test";

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn sharing(mode: AllodiaAccountSyncMode) -> AccountSharing {
    AccountSharing {
        mode,
        control: FREE,
        failed: None,
    }
}

fn toggles(root: &impl IsA<gtk::Widget>) -> Vec<gtk::ToggleButton> {
    crate::ui::setup_widget_tests::descendants::<gtk::ToggleButton>(root.as_ref())
}

fn toggle(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::ToggleButton {
    toggles(root)
        .into_iter()
        .find(|button| button.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no {label:?} toggle: {:?}", rendered_labels(root.as_ref())))
}

fn label_for(mode: AllodiaAccountSyncMode) -> &'static str {
    match mode {
        AllodiaAccountSyncMode::On => l10n::settings_account_sync_on(),
        AllodiaAccountSyncMode::Paused => l10n::settings_account_sync_paused(),
        AllodiaAccountSyncMode::Off => l10n::settings_account_sync_off(),
    }
}

const EVERY_MODE: [AllodiaAccountSyncMode; 3] = [
    AllodiaAccountSyncMode::On,
    AllodiaAccountSyncMode::Paused,
    AllodiaAccountSyncMode::Off,
];

/// All three positions are always on offer, and exactly the one in force is held down.
///
/// The three are one choice, so a state where none or two read as selected is not a cosmetic
/// problem; it is the control failing to say what the account is doing.
pub(crate) fn the_control_offers_three_positions_and_holds_the_one_in_force() {
    let (sender, _receiver) = relm4::channel::<AppInput>();

    for mode in EVERY_MODE {
        let group = synced_group(&sender, ACCOUNT, &sharing(mode));
        let shown = rendered_labels(group.upcast_ref::<gtk::Widget>());
        for option in EVERY_MODE {
            assert!(
                shown.iter().any(|text| text == label_for(option)),
                "every position stays on offer: {shown:?}"
            );
        }
        let held: Vec<_> = toggles(&group)
            .into_iter()
            .filter(gtk::prelude::ToggleButtonExt::is_active)
            .filter_map(|button| button.label().map(|label| label.to_string()))
            .collect();
        assert_eq!(
            held,
            vec![label_for(mode).to_owned()],
            "exactly the position in force is held down"
        );
    }
}

/// The description says what the position in force means, and only that one.
///
/// Three meanings at once is a paragraph nobody reads, and the one that matters is the one the
/// account is actually in.
pub(crate) fn the_description_explains_only_the_position_in_force() {
    let (sender, _receiver) = relm4::channel::<AppInput>();

    for mode in EVERY_MODE {
        let hint = match mode {
            AllodiaAccountSyncMode::On => l10n::settings_account_sync_on_hint(),
            AllodiaAccountSyncMode::Paused => l10n::settings_account_sync_paused_hint(),
            AllodiaAccountSyncMode::Off => l10n::settings_account_sync_off_hint(),
        };
        let shown =
            rendered_labels(synced_group(&sender, ACCOUNT, &sharing(mode)).upcast_ref::<gtk::Widget>());
        assert!(
            shown.iter().any(|text| text == hint),
            "the position in force explains itself: {shown:?}"
        );
    }
}

/// Choosing another position asks for it, naming the account; and asks **once**.
///
/// The three share a group, so selecting one *releases* another and both fire `toggled`. Read
/// naively that is two answers to one question, the second of them the position the person just
/// left: the account would be set to the new mode and then back again.
pub(crate) fn choosing_a_position_asks_for_it_once() {
    for from in EVERY_MODE {
        for to in EVERY_MODE {
            if from == to {
                continue;
            }
            let (sender, receiver) = relm4::channel::<AppInput>();
            let group = synced_group(&sender, ACCOUNT, &sharing(from));
            toggle(&group, label_for(to)).set_active(true);
            // A sentinel behind it, because the channel has no non-blocking read: anything the
            // release emitted is already queued, so what follows the choice tells us how many
            // answers one press produced.
            sender.emit(AppInput::CollectNewMail);

            match receiver.recv_sync() {
                Some(AppInput::Accounts(AccountsInput::Change(AccountPageIntent::SetSharing {
                    account,
                    mode,
                }))) => {
                    assert_eq!(account, ACCOUNT);
                    assert_eq!(mode, to, "the position pressed is the one asked for");
                }
                other => panic!("choosing {to:?} from {from:?} must ask for it, got {other:?}"),
            }
            assert!(
                matches!(receiver.recv_sync(), Some(AppInput::CollectNewMail)),
                "releasing {from:?} is not a second answer"
            );
        }
    }
}

/// Pressing the position already in force asks for nothing.
///
/// This asserts the outcome rather than the guard that produces it: a grouped toggle already
/// swallows a press on the active member, so the code would have to change shape before this
/// could fail. It is here because the outcome is the promise; a press that spends a round trip
/// to set what is already set is a bug whatever swallows it.
pub(crate) fn pressing_the_position_already_in_force_asks_for_nothing() {
    for mode in EVERY_MODE {
        let (sender, receiver) = relm4::channel::<AppInput>();
        let group = synced_group(&sender, ACCOUNT, &sharing(mode));
        let held = toggle(&group, label_for(mode));
        assert!(held.is_active(), "it is the one in force");
        held.emit_clicked();
        // A sentinel behind it, because the channel has no non-blocking read: whatever the press
        // emitted must arrive before this, so the sentinel arriving first is the assertion.
        sender.emit(AppInput::CollectNewMail);

        assert!(
            matches!(receiver.recv_sync(), Some(AppInput::CollectNewMail)),
            "the position already in force is not a change"
        );
    }
}

/// The control lives on a row, and a row belongs in a list or the keyboard skips it.
pub(crate) fn the_control_is_reachable_from_the_keyboard() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let group = synced_group(&sender, ACCOUNT, &sharing(AllodiaAccountSyncMode::On));
    every_row_belongs_to_a_list(group.upcast_ref::<gtk::Widget>());
}

/// While a change runs the control spins beside itself, drawn where it is going, and cannot be
/// pressed; one that did not take says so under it.
pub(crate) fn a_running_change_spins_and_a_failed_one_says_so() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let running = AccountSharing {
        mode: AllodiaAccountSyncMode::Paused,
        control: ControlState {
            enabled: false,
            running: true,
        },
        failed: None,
    };
    let group = synced_group(&sender, ACCOUNT, &running);
    assert!(spins(&group), "the change runs");
    assert!(
        toggles(&group).iter().all(|button| !button.is_sensitive()),
        "a running control cannot be pressed"
    );
    assert!(toggle(&group, label_for(AllodiaAccountSyncMode::Paused)).is_active());

    let failed = AccountSharing {
        failed: Some(AllodiaSyncFailure::Unavailable),
        ..sharing(AllodiaAccountSyncMode::On)
    };
    let shown = rendered_labels(synced_group(&sender, ACCOUNT, &failed).upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|text| text == l10n::settings_account_sync_failed()),
        "a change that did not take is said: {shown:?}"
    );
}
