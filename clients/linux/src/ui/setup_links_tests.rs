use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountUse, AccountsSnapshot,
    CapabilityState, LinkCandidates, LinkSlot, LinkedAccount,
};

use super::LinkStep;

fn linked(id: &str) -> LinkedAccount {
    LinkedAccount {
        id: id.to_owned(),
        address: id.to_owned(),
    }
}

/// A mail account with no calendar of its own, two calendars it may use, the second suggested.
fn snapshot() -> AccountsSnapshot {
    AccountsSnapshot {
        accounts: vec![AccountEntry {
            id: "alice@imap".to_owned(),
            address: "alice@example.org".to_owned(),
            kind: AccountKind::Imap,
            uses: vec![AccountUse {
                capability: AccountCapability::Mail,
                state: CapabilityState::On,
            }],
            links: AccountLinksView::default(),
            linked_from: Vec::new(),
            link_candidates: LinkCandidates {
                calendar: vec![linked("bob@dav"), linked("alice@dav")],
                contacts: Vec::new(),
                mail: Vec::new(),
                suggested: vec!["alice@dav".to_owned()],
            },
            endpoints: None,
        }],
    }
}

#[test]
fn a_suggestion_starts_picked_and_continuing_links_it() {
    let step = LinkStep::for_account(&snapshot(), "alice@imap").expect("a calendar to link");
    assert_eq!(step.pickers.len(), 1, "only the slot with candidates");
    assert_eq!(step.picked, [Some(1)]);
    assert_eq!(step.links(), [(LinkSlot::Calendar, "alice@dav".to_owned())]);
}

#[test]
fn nothing_to_link_is_no_step() {
    assert!(LinkStep::for_account(&snapshot(), "someone-else").is_none());
    // A provider account is linked from Settings, not offered it at setup.
    let mut provider = snapshot();
    provider.accounts[0].kind = AccountKind::Google;
    assert!(LinkStep::for_account(&provider, "alice@imap").is_none());
    let mut lone = snapshot();
    lone.accounts[0].link_candidates = LinkCandidates::default();
    assert!(LinkStep::for_account(&lone, "alice@imap").is_none());
}

#[test]
fn a_pick_survives_a_late_suggestion_and_none_links_nothing() {
    let mut first = snapshot();
    first.accounts[0].link_candidates.suggested.clear();
    let mut step = LinkStep::for_account(&first, "alice@imap").unwrap();
    assert_eq!(step.picked, [None]);
    assert!(step.links().is_empty(), "none picked, nothing linked");

    // Untouched, the step takes the suggestion that arrived.
    let fresh = LinkStep::for_account(&snapshot(), "alice@imap").unwrap();
    let step_now = step.refreshed(fresh);
    assert_eq!(step_now.picked, [Some(1)]);

    // Touched, it keeps what the person picked.
    step = step_now;
    step.pick(0, Some(0));
    let fresh = LinkStep::for_account(&snapshot(), "alice@imap").unwrap();
    let kept = step.refreshed(fresh);
    assert_eq!(kept.links(), [(LinkSlot::Calendar, "bob@dav".to_owned())]);
}

/// "Add another account" runs setup again from the address and comes back to this step, and the
/// accounts the flow added are kept for their names; a fresh flow keeps neither.
#[test]
fn another_account_comes_back_to_the_step_it_was_added_from() {
    use crate::ui::setup_state::{Phase, SetupState};
    let mut state = SetupState::closed();
    state.open(false);
    state.added.push("alice@imap".to_owned());
    state.show_links(LinkStep::for_account(&snapshot(), "alice@imap").unwrap());
    assert_eq!(state.phase, Phase::Links);
    assert_eq!(state.linking(), Some("alice@imap"));

    state.add_linked();
    assert_eq!(state.phase, Phase::Email);
    assert_eq!(state.returning_to.as_deref(), Some("alice@imap"));
    assert_eq!(state.added, ["alice@imap"]);
    assert!(state.linking().is_none());

    // Back inside the new setup still comes back to the step.
    state.back_to_address(false);
    assert_eq!(state.returning_to.as_deref(), Some("alice@imap"));

    state.open(false);
    assert!(state.returning_to.is_none() && state.added.is_empty());
}

/// A Settings signal reads the step again on every pass; only a step that changed is redrawn, so
/// an open dropdown is not closed under the person, and a closed flow is linking nothing.
#[test]
fn an_unchanged_step_is_not_redrawn_and_a_closed_flow_links_nothing() {
    use crate::ui::setup_state::SetupState;
    let mut first = snapshot();
    first.accounts[0].link_candidates.suggested.clear();
    let mut state = SetupState::closed();
    state.open(false);
    state.show_links(LinkStep::for_account(&first, "alice@imap").unwrap());

    let drawn = state.generation;
    state.refresh_links(LinkStep::for_account(&first, "alice@imap").unwrap());
    assert_eq!(state.generation, drawn, "nothing changed, nothing redrawn");

    state.refresh_links(LinkStep::for_account(&snapshot(), "alice@imap").unwrap());
    assert_ne!(state.generation, drawn, "a late suggestion is drawn");
    assert_eq!(state.links.as_ref().unwrap().picked, [Some(1)]);

    state.complete();
    assert!(state.linking().is_none());
}

/// The step draws Settings' pickers with the suggestion picked, and says what the person does.
pub(in crate::ui) fn the_link_step_offers_the_accounts_it_can_use(window: &adw::ApplicationWindow) {
    use adw::prelude::*;

    use crate::{
        l10n,
        ui::{
            AppInput,
            setup::SetupWindow,
            setup_state::SetupState,
            setup_widget_tests::{descendant_button, descendants},
        },
    };
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_links(LinkStep::for_account(&snapshot(), "alice@imap").unwrap());
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("link step content");

    let picker = descendants::<gtk::DropDown>(&child)
        .into_iter()
        .next()
        .expect("the calendar picker");
    assert_eq!(
        picker.selected(),
        2,
        "the suggestion, after None and the first calendar"
    );
    picker.set_selected(1);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetupLinkPicked(0, Some(0)))
    ));
    descendant_button(&child, l10n::setup_links_add()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetupAddLinkedAccount)
    ));
    descendant_button(&child, l10n::setup_detect_action()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetupLinksDone(true))
    ));
}

/// A pick made before "Add another account" survives it: the step comes back with the account
/// just added among the options and what was picked still picked.
#[test]
fn a_pick_survives_adding_another_account() {
    use crate::ui::setup_state::{Phase, SetupState};
    let mut state = SetupState::closed();
    state.open(false);
    state.show_links(LinkStep::for_account(&snapshot(), "alice@imap").unwrap());
    state.pick_link(0, Some(0));
    state.add_linked();

    let mut later = snapshot();
    later.accounts[0]
        .link_candidates
        .calendar
        .insert(0, linked("carol@dav"));
    state.show_links_again(LinkStep::for_account(&later, "alice@imap").unwrap());
    assert_eq!(state.phase, Phase::Links);
    let step = state.links.as_ref().unwrap();
    assert_eq!(
        step.pickers[0].options.len(),
        3,
        "the account just added is offered"
    );
    assert_eq!(step.links(), [(LinkSlot::Calendar, "bob@dav".to_owned())]);
}
