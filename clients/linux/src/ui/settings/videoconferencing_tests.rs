//! What the Jitsi server field shows, what it stores and what it says about an address the core
//! refuses; and what each account's default video meeting offers and stores.
//!
//! The row commits on apply (Return or its apply button) and on losing focus; these assertions
//! drive the apply directly. Called from the crate's single `gtk::init` test.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    CapabilityState, LinkedAccount, MeetingAccount, MeetingDefault, MeetingOption,
};

use super::{default_meeting_group, jitsi_server_group, meeting_accounts_group, state_word};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput},
};

/// A stand-in for the core's store: it keeps an `https` address with its trailing `/` dropped,
/// and refuses anything else.
fn store() -> Rc<RefCell<Option<String>>> {
    Rc::new(RefCell::new(None))
}

fn group_over(held: &Rc<RefCell<Option<String>>>) -> adw::PreferencesGroup {
    let read = Rc::clone(held);
    let write = Rc::clone(held);
    jitsi_server_group(
        move || read.borrow().clone(),
        move |typed| {
            let typed = typed.trim().to_owned();
            if typed.is_empty() {
                *write.borrow_mut() = None;
                return true;
            }
            if !typed.starts_with("https://") || typed.contains(['?', '#']) {
                return false;
            }
            *write.borrow_mut() = Some(typed.trim_end_matches('/').to_owned());
            true
        },
    )
}

fn parts(group: &adw::PreferencesGroup) -> (adw::EntryRow, gtk::Label) {
    let widgets = crate::ui::setup_widget_tests::descendants::<gtk::Widget>(
        group.upcast_ref::<gtk::Widget>(),
    );
    let entry = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<adw::EntryRow>().ok())
        .expect("the group holds the field");
    let refused = widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk::Label>().ok())
        .find(|label| label.text() == l10n::settings_jitsi_server_invalid())
        .expect("the group holds the refusal");
    (entry, refused)
}

pub(crate) fn the_jitsi_server_field_stores_what_the_core_accepts_and_says_when_it_refuses() {
    let held = store();
    let group = group_over(&held);
    let (entry, refused) = parts(&group);
    assert_eq!(
        entry.text(),
        "",
        "the field is empty until the person fills it"
    );
    assert!(!refused.is_visible());

    entry.set_text("meet.example.org");
    entry.emit_by_name::<()>("apply", &[]);
    assert!(refused.is_visible(), "an address the core refuses is said");
    assert_eq!(*held.borrow(), None, "and changes nothing");

    entry.set_text("https://meet.example.org/");
    assert!(!refused.is_visible(), "typing again takes the refusal away");
    entry.emit_by_name::<()>("apply", &[]);
    assert_eq!(held.borrow().as_deref(), Some("https://meet.example.org"));
    assert_eq!(
        entry.text(),
        "https://meet.example.org",
        "the stored form is shown"
    );
    assert!(!refused.is_visible());

    let reopened = group_over(&held);
    assert_eq!(parts(&reopened).0.text(), "https://meet.example.org");

    entry.set_text("");
    entry.emit_by_name::<()>("apply", &[]);
    assert_eq!(*held.borrow(), None, "an empty field forgets the server");
}

#[test]
fn a_meeting_account_says_its_use_in_the_words_its_page_would() {
    assert_eq!(state_word(CapabilityState::On), l10n::settings_toggle_on());
    assert_eq!(
        state_word(CapabilityState::Off),
        l10n::settings_toggle_off()
    );
    assert_eq!(
        state_word(CapabilityState::NeedsPermission),
        l10n::settings_meeting_account_needs_permission()
    );
    assert_eq!(
        state_word(CapabilityState::NotOffered),
        l10n::settings_meeting_account_not_offered()
    );
}

#[test]
fn no_account_that_makes_meetings_draws_no_group() {
    let (sender, _receiver) = relm4::channel();
    assert!(meeting_accounts_group(&[], &sender).is_none());
}

/// Called from the crate's single `gtk::init` test.
pub(crate) fn a_meeting_account_row_names_the_service_and_account_and_opens_its_page() {
    let (sender, receiver) = relm4::channel();
    let account = MeetingAccount {
        account: LinkedAccount {
            id: "alice@contoso.example@microsoft".to_owned(),
            address: "alice@contoso.example".to_owned(),
            distinction: None,
        },
        service: "Microsoft Teams".to_owned(),
        state: CapabilityState::NeedsPermission,
    };
    let group = meeting_accounts_group(&[account], &sender).expect("one account draws the group");
    let widgets = crate::ui::setup_widget_tests::descendants::<gtk::Widget>(
        group.upcast_ref::<gtk::Widget>(),
    );
    let row = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<adw::ActionRow>().ok())
        .expect("the group holds the account's row");
    assert_eq!(row.title(), "Microsoft Teams");
    assert_eq!(row.subtitle().as_deref(), Some("alice@contoso.example"));
    assert!(row.is_activatable());
    assert!(
        widgets
            .iter()
            .filter_map(|widget| widget.clone().downcast::<gtk::Label>().ok())
            .any(|label| label.text() == l10n::settings_meeting_account_needs_permission()),
        "the row says the use's state"
    );
    assert!(
        !widgets
            .iter()
            .any(|widget| widget.is::<gtk::Switch>() || widget.is::<adw::SwitchRow>()),
        "the switch is on the account's page, not here"
    );
    row.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::Visit(id))) if id == "alice@contoso.example@microsoft"
    ));
}

#[test]
fn no_account_that_offers_a_meeting_draws_no_default_group() {
    assert!(default_meeting_group(&[], |_, _| {}).is_none());
}

/// Called from the crate's single `gtk::init` test.
pub(crate) fn an_accounts_default_meeting_opens_on_the_stored_choice_and_stores_a_new_one() {
    let default = MeetingDefault {
        account: "alice@cloud.example.org@dav".to_owned(),
        account_address: "alice@cloud.example.org".to_owned(),
        choices: vec![
            MeetingOption {
                id: "account:talk".to_owned(),
                name: "Nextcloud Talk".to_owned(),
                through: Some("alice@cloud.example.org".to_owned()),
            },
            MeetingOption {
                id: "jitsi".to_owned(),
                name: "Jitsi Meet".to_owned(),
                through: None,
            },
        ],
        chosen: Some("jitsi".to_owned()),
    };
    let stored: Rc<RefCell<Vec<(String, Option<String>)>>> = Rc::default();
    let sink = Rc::clone(&stored);
    let group = default_meeting_group(&[default], move |account, choice| {
        sink.borrow_mut().push((account, choice));
    })
    .expect("one account draws the group");
    let row = crate::ui::setup_widget_tests::descendants::<adw::ComboRow>(
        group.upcast_ref::<gtk::Widget>(),
    )
    .into_iter()
    .next()
    .expect("the group holds the account's row");
    assert_eq!(row.title(), "alice@cloud.example.org");
    let model = row
        .model()
        .and_downcast::<gtk::StringList>()
        .expect("the row draws a string list");
    assert_eq!(model.n_items(), 3);
    assert_eq!(
        model.string(0).as_deref(),
        Some(l10n::event_meeting_none()),
        "None comes first"
    );
    assert_eq!(
        model.string(1).as_deref(),
        Some(l10n::event_meeting_through("Nextcloud Talk", "alice@cloud.example.org").as_str())
    );
    assert_eq!(row.selected(), 2, "the row opens on the stored choice");
    assert!(stored.borrow().is_empty(), "drawing the row stores nothing");

    row.set_selected(1);
    row.set_selected(0);
    assert_eq!(
        *stored.borrow(),
        [
            (
                "alice@cloud.example.org@dav".to_owned(),
                Some("account:talk".to_owned())
            ),
            ("alice@cloud.example.org@dav".to_owned(), None),
        ]
    );
}
