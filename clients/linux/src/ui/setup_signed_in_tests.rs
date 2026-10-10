use mailcal_bindings::{AccountCapability, SetupChoice};

use super::UsesStep;
use crate::ui::setup_state::{Phase, SetupState};

fn on(capability: AccountCapability) -> SetupChoice {
    SetupChoice {
        capability,
        on: true,
        server_found: true,
    }
}

/// What a JMAP session offering mail, calendar and contacts answers once signed in.
fn step() -> UsesStep {
    UsesStep::new(
        "alice@jmap".to_owned(),
        "alice@jmap".to_owned(),
        vec![
            on(AccountCapability::Mail),
            on(AccountCapability::Calendar),
            on(AccountCapability::Contacts),
        ],
    )
    .expect("three uses to choose from")
}

#[test]
fn nothing_to_choose_is_no_step_and_what_is_not_chosen_is_switched_off() {
    assert!(UsesStep::new("a".to_owned(), "a".to_owned(), Vec::new()).is_none());
    assert_eq!(
        step().dropped(&[AccountCapability::Mail, AccountCapability::Contacts]),
        [AccountCapability::Calendar]
    );
    assert!(
        step()
            .dropped(&[
                AccountCapability::Mail,
                AccountCapability::Calendar,
                AccountCapability::Contacts,
            ])
            .is_empty()
    );
}

/// The step answers once: a second Continue, or one from a window no longer on that step, finds
/// nothing to store; and a fresh flow forgets it.
#[test]
fn the_step_is_answered_once_and_a_fresh_flow_forgets_it() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_uses(step());
    assert_eq!(state.phase, Phase::Uses);
    assert!(state.choosing_uses());

    assert!(state.take_uses().is_some());
    assert!(state.take_uses().is_none(), "answered already");
    assert!(state.choosing_uses(), "waiting while the choice is stored");

    state.show_uses(step());
    state.open(false);
    assert!(state.uses.is_none() && !state.choosing_uses());
}

/// The step draws the found card's toggles from the core's choices, keeps one on, and continues
/// with what is chosen.
pub(in crate::ui) fn the_uses_step_offers_what_the_server_offers(window: &adw::ApplicationWindow) {
    use adw::prelude::*;

    use crate::{
        l10n,
        ui::{
            AppInput,
            setup::SetupWindow,
            setup_widget_tests::{check_button, descendant_button},
        },
    };
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_uses(step());
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("uses step content");

    let toggle = |label| check_button(&child, label).unwrap_or_else(|| panic!("{label}"));
    let mail = toggle(l10n::setup_detect_mail_enable());
    let calendar = toggle(l10n::setup_detect_calendar_enable());
    let contacts = toggle(l10n::setup_detect_contacts_enable());
    assert!(mail.is_active() && calendar.is_active() && contacts.is_active());
    calendar.set_active(false);
    contacts.set_active(false);
    assert!(!mail.is_sensitive(), "the last use on stays on");

    descendant_button(&child, l10n::setup_detect_action()).emit_clicked();
    match receiver.recv_sync() {
        Some(AppInput::SetupUsesChosen(chosen)) => {
            assert_eq!(chosen, [AccountCapability::Mail]);
        }
        other => panic!("expected the chosen uses, got {other:?}"),
    }

    state.take_uses();
    setup.render(&state, window, &sender);
    let saving = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("saving content");
    assert!(
        crate::ui::mailbox::tests::rendered_labels(&saving)
            .iter()
            .any(|text| text == l10n::setup_signed_in_uses_saving()),
        "the step waits while the choice is stored"
    );
}
