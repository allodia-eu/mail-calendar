//! The link step of a JMAP account whose server has no calendar: offered even with nothing to
//! pick, with the calendar server found beside the JMAP server as an account of its own, and the
//! account added from it picked when the step comes back.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountUse, AccountsSnapshot,
    CapabilityState, LinkCandidates, LinkSlot, LinkedAccount,
};

use super::{Beside, Fills, LinkStep};
use crate::ui::setup_state::{Phase, SetupState};

const JMAP: &str = "alice@jmap:example.org";

fn jmap(calendar: CapabilityState, candidates: Vec<LinkedAccount>) -> AccountsSnapshot {
    let use_ = |capability, state| AccountUse {
        capability,
        state,
        problem: None,
    };
    AccountsSnapshot {
        accounts: vec![AccountEntry {
            id: JMAP.to_owned(),
            address: "alice@example.org".to_owned(),
            kind: AccountKind::Jmap,
            sign_in: mailcal_bindings::SignInMethod::Password,
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
            separate_server: None,
            distinction: None,
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
        distinction: None,
    }
}

#[test]
fn a_server_without_a_calendar_is_offered_the_step_and_the_server_beside_it() {
    let lacking = jmap(CapabilityState::NotOffered, Vec::new());
    let step = LinkStep::for_account(&lacking, JMAP, None).expect("offered with nothing to pick");
    assert!(step.pickers.is_empty() && step.beside.is_none());
    let step = LinkStep::for_account(&lacking, JMAP, Some(&beside())).unwrap();
    assert_eq!(step.beside, Some((beside(), Fills::Calendar)));

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

    descendant_button(
        &child,
        &l10n::setup_links_beside_calendar("dav.example.org"),
    )
    .emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetupBesideAccount)
    ));
}

fn detected_jmap(email: &str) -> crate::ui::setup_model::SetupForm {
    use crate::ui::setup_model::{DetectedForm, JmapForm, JmapSignIn, SetupForm};
    SetupForm::Detected(DetectedForm::Jmap(JmapForm {
        email: email.to_owned(),
        server_url: "https://jmap.example.org".to_owned(),
        trusted: true,
        sign_in: JmapSignIn::Offered,
        caldav_url: Some("https://dav.example.org/.well-known/caldav".to_owned()),
        carddav_url: None,
    }))
}

/// An API token goes in by hand: a server offering sign-in shows the offer in place of the
/// secret field, so "Set up manually" is the way to a token, and the servers found beside the
/// JMAP server must survive it.
#[test]
fn the_server_beside_survives_setting_the_account_up_by_hand() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(detected_jmap("alice@example.org"));
    state.edit_detected_manually();

    state.remember_beside(JMAP);

    assert_eq!(state.beside_for(JMAP), Some(&beside()));
}

/// What was found belongs to the address it was found for, never to another typed in its place.
#[test]
fn the_server_beside_is_not_offered_for_another_address() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(detected_jmap("alice@example.org"));
    state.show_form(crate::ui::setup_model::manual_form(
        "bob@example.net".to_owned(),
        None,
    ));

    state.remember_beside(JMAP);

    assert!(state.beside_for(JMAP).is_none());
}

/// The offer fills what the account lacks and names it: a JMAP session with contacts of its own
/// is offered the calendar beside it, never a second copy of its contacts.
#[test]
fn the_server_beside_is_offered_for_what_the_account_lacks() {
    let lacks = |calendar, contacts| {
        let mut snapshot = jmap(calendar, Vec::new());
        snapshot.accounts[0].uses[2].state = contacts;
        snapshot
    };
    let both = Beside {
        carddav_url: Some("https://dav.example.org/.well-known/carddav".to_owned()),
        ..beside()
    };
    let fills = |snapshot: &AccountsSnapshot, found: &Beside| {
        LinkStep::for_account(snapshot, JMAP, Some(found))
            .and_then(|step| step.beside)
            .map(|(_, fills)| fills)
    };
    let (on, off) = (CapabilityState::On, CapabilityState::NotOffered);

    assert_eq!(fills(&lacks(off, on), &both), Some(Fills::Calendar));
    assert_eq!(fills(&lacks(on, off), &both), Some(Fills::Contacts));
    assert_eq!(fills(&lacks(off, off), &both), Some(Fills::Both));
    assert_eq!(
        fills(&lacks(on, off), &beside()),
        Some(Fills::Contacts),
        "contacts are looked for at the calendar's server when no address book was found"
    );
}
