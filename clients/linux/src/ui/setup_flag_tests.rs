//! A form that cannot go on says which field it needs, rather than failing in the core's words or
//! not at all.

use adw::prelude::*;
use mailcal_bindings::{ConnectionSecurity, SetupRecommendation};

use crate::{
    l10n,
    ui::{
        AppInput,
        setup::SetupWindow,
        setup_model::{AccountKind, ManualForm, recommendation_form},
        setup_state::SetupState,
        setup_widget_tests::{check_button, descendant_button, server_row, visible_entries},
    },
};

/// Sends a sentinel after the click under test and reads what arrives first: the sentinel means
/// the click sent nothing.
fn sent_nothing(sender: &relm4::Sender<AppInput>, receiver: &relm4::Receiver<AppInput>) -> bool {
    sender.emit(AppInput::KeepEditing);
    matches!(receiver.recv_sync(), Some(AppInput::KeepEditing))
}

fn field<'a>(fields: &'a [gtk::Entry], placeholder: &str) -> &'a gtk::Entry {
    fields
        .iter()
        .find(|field| field.placeholder_text().as_deref() == Some(placeholder))
        .unwrap_or_else(|| panic!("{placeholder}"))
}

/// A calendar switched on with no server found and none typed holds the connect at its field.
pub(super) fn a_use_switched_on_without_a_server_asks_for_one(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Imap {
            oauth_issuer: None,
            email: "alice@example.test".to_owned(),
            imap_host: "imap.example.test:993".to_owned(),
            smtp_host: None,
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            incoming: server_row("IMAP", "imap.example.test", 993),
            outgoing: None,
            caldav_url: None,
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
    super::setup_signin_tests::answer_password(
        &mut state,
        "alice@example.test",
        "imap.example.test:993",
    );
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content");

    check_button(&child, l10n::setup_detect_calendar_add())
        .expect("the calendar choice")
        .set_active(true);
    let fields = visible_entries(&child);
    field(&fields, l10n::setup_field_password()).set_text("secret");
    descendant_button(&child, l10n::action_connect()).emit_clicked();
    assert!(sent_nothing(&sender, &receiver), "nothing is submitted");
    let calendar = field(&fields, l10n::setup_hint_caldav());
    assert!(
        calendar.has_css_class("error"),
        "the calendar's field is marked"
    );
    calendar.set_text("cal.example.test");
    assert!(!calendar.has_css_class("error"), "typing clears the mark");
}

/// The manual calendar-and-contacts form with no server typed says so at the calendar's field.
pub(super) fn a_manual_calendar_form_without_a_server_asks_for_one(
    window: &adw::ApplicationWindow,
) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.select_account_kind(ManualForm {
        kind: AccountKind::Dav,
        email: "alice@cloud.example.test".to_owned(),
        ..ManualForm::default()
    });
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("manual calendar-and-contacts content");
    let fields = visible_entries(&child);
    field(&fields, l10n::setup_field_password()).set_text("secret");
    descendant_button(&child, l10n::action_connect()).emit_clicked();
    assert!(sent_nothing(&sender, &receiver));
    assert!(field(&fields, l10n::setup_field_caldav_optional()).has_css_class("error"));
}
