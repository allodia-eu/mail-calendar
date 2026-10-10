//! The link step of a JMAP account whose server has no calendar: offered even with nothing to
//! pick, with the calendar server found beside the JMAP server as an account of its own, and the
//! account added from it picked when the step comes back.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountUse, AccountsSnapshot,
    CapabilityState, LinkCandidates, LinkSlot, LinkedAccount,
};

use super::{Beside, LinkStep};
use crate::ui::setup_state::{Phase, SetupState};

const JMAP: &str = "alice@jmap:example.org";

fn jmap(calendar: CapabilityState, candidates: Vec<LinkedAccount>) -> AccountsSnapshot {
    let use_ = |capability, state| AccountUse { capability, state };
    AccountsSnapshot {
        accounts: vec![AccountEntry {
            id: JMAP.to_owned(),
            address: "alice@example.org".to_owned(),
            kind: AccountKind::Jmap,
            uses: vec![
                use_(AccountCapability::Mail, CapabilityState::On),
                use_(AccountCapability::Calendar, calendar),
                use_(AccountCapability::Contacts, CapabilityState::On),
            ],
            links: AccountLinksView::default(),
            linked_from: Vec::new(),
            link_candidates: LinkCandidates {
                calendar: candidates,
                ..LinkCandidates::default()
            },
            endpoints: None,
        }],
    }
}

fn beside() -> Beside {
    Beside {
        account: JMAP.to_owned(),
        email: "alice@example.org".to_owned(),
        caldav_url: Some("https://dav.example.org/.well-known/caldav".to_owned()),
        carddav_url: None,
    }
}

fn cloud() -> LinkedAccount {
    LinkedAccount {
        id: "alice@dav:dav.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
    }
}

#[test]
fn a_server_without_a_calendar_is_offered_the_step_and_the_server_beside_it() {
    let lacking = jmap(CapabilityState::NotOffered, Vec::new());
    let step = LinkStep::for_account(&lacking, JMAP, None).expect("offered with nothing to pick");
    assert!(step.pickers.is_empty() && step.beside.is_none());
    let step = LinkStep::for_account(&lacking, JMAP, Some(&beside())).unwrap();
    assert_eq!(step.beside, Some(beside()));

    let offering = jmap(CapabilityState::On, Vec::new());
    assert!(
        LinkStep::for_account(&offering, JMAP, Some(&beside())).is_none(),
        "a server with a calendar of its own needs nothing beside it"
    );
}

#[test]
fn the_server_beside_comes_back_as_an_account_picked_on_the_step() {
    let mut state = SetupState::closed();
    state.open(false);
    state.added.push(JMAP.to_owned());
    state.beside.push(beside());
    let lacking = jmap(CapabilityState::NotOffered, Vec::new());
    state.show_links(LinkStep::for_account(&lacking, JMAP, state.beside_for(JMAP)).unwrap());

    let used = state.add_beside().expect("the step offered it");
    assert_eq!(used, beside());
    assert_eq!(state.returning_to.as_deref(), Some(JMAP));
    assert_eq!(state.start_email, "alice@example.org");
    assert!(state.beside_for(JMAP).is_none(), "offered once");
    assert_eq!(state.added, [JMAP]);

    let later = jmap(CapabilityState::NotOffered, vec![cloud()]);
    state.show_links_again(
        LinkStep::for_account(&later, JMAP, None).unwrap(),
        Some("alice@dav:dav.example.org"),
    );
    assert_eq!(state.phase, Phase::Links);
    assert_eq!(
        state.links.as_ref().unwrap().links(),
        [(LinkSlot::Calendar, "alice@dav:dav.example.org".to_owned())]
    );
}

/// The step names the server found and offers it as a button of its own.
pub(in crate::ui) fn the_step_offers_the_server_beside(window: &adw::ApplicationWindow) {
    use adw::prelude::*;

    use crate::{
        l10n,
        ui::{AppInput, setup::SetupWindow, setup_widget_tests::descendant_button},
    };
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let lacking = jmap(CapabilityState::NotOffered, Vec::new());
    state.show_links(LinkStep::for_account(&lacking, JMAP, Some(&beside())).unwrap());
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("link step content");

    descendant_button(&child, &l10n::setup_links_beside("dav.example.org")).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetupBesideAccount)
    ));
}
