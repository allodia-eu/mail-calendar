//! What a found card offers the account for, as the person sees and changes it.

use adw::prelude::*;
use mailcal_bindings::{
    AccountCapability, ConnectionSecurity, DetectedSetup, MissReason, SetupChoice,
    SetupRecommendation,
};

use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::rendered_labels,
        setup::SetupWindow,
        setup_model::{
            AccountKind, AccountSubmission, ManualForm, SetupForm, detected_form,
            recommendation_form,
        },
        setup_state::SetupState,
        setup_widget_tests::{check_button, descendant_button, server_row, visible_entries},
    },
};

/// Mail, calendar and contacts each a choice, on where their server was found; the mail servers
/// follow the mail choice; the last use on cannot be switched off; and what is chosen is what the
/// account is set up with.
pub(super) fn the_found_card_offers_each_use_and_keeps_one_on(window: &adw::ApplicationWindow) {
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
            caldav_url: Some("https://caldav.example.test/dav".to_owned()),
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

    let toggle = |label| check_button(&child, label).unwrap_or_else(|| panic!("{label}"));
    let mail = toggle(l10n::setup_detect_mail_enable());
    let calendar = toggle(l10n::setup_detect_calendar_enable());
    let contacts = toggle(l10n::setup_detect_contacts_enable());
    assert!(mail.is_active() && calendar.is_active() && contacts.is_active());
    // Contacts found through the calendar's server name that server.
    let hosts = rendered_labels(&child)
        .into_iter()
        .filter(|text| text == "caldav.example.test")
        .count();
    assert_eq!(hosts, 2, "under the calendar and under contacts");

    let incoming = gtk::Label::new(None);
    let server = crate::ui::setup_widget_tests::descendants::<gtk::Label>(&child)
        .into_iter()
        .find(|label| label.label() == "imap.example.test:993 · TLS")
        .unwrap_or(incoming);
    mail.set_active(false);
    assert!(
        !server.is_visible(),
        "the mail servers follow the mail choice"
    );
    calendar.set_active(false);
    assert!(
        !contacts.is_sensitive(),
        "the last use on cannot be switched off"
    );
    assert!(mail.is_sensitive() && calendar.is_sensitive());

    let password = visible_entries(&child)
        .into_iter()
        .next()
        .expect("the password field");
    password.set_text("secret");
    descendant_button(&child, l10n::action_connect()).emit_clicked();
    let Some(AppInput::SubmitAccount(submission)) = receiver.recv_sync() else {
        panic!("Connect submits the account");
    };
    let AccountSubmission::Imap(submission) = *submission else {
        panic!("a password account");
    };
    assert_eq!(submission.uses, Some(vec![AccountCapability::Contacts]));
    // The calendar is off, and its server is where contacts are looked for.
    assert_eq!(submission.caldav_url, "https://caldav.example.test/dav");
}

/// A domain with a calendar and address book and no mail server draws no mail choice and no
/// mail server, only the two it found and the password.
pub(super) fn a_calendar_and_contacts_card_asks_only_for_the_password(
    window: &adw::ApplicationWindow,
) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let found = |capability| SetupChoice {
        capability,
        on: true,
        server_found: true,
    };
    state.show_form(detected_form(
        DetectedSetup {
            recommendation: SetupRecommendation::Manual {
                reason: MissReason::NothingFound,
            },
            calendar_and_contacts: true,
            caldav_url: Some("https://cloud.example.test/dav".to_owned()),
            carddav_url: Some("https://cloud.example.test/dav".to_owned()),
            choices: vec![
                found(AccountCapability::Calendar),
                found(AccountCapability::Contacts),
            ],
        },
        "alice@cloud.example.test".to_owned(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("calendar-and-contacts content");

    assert!(
        rendered_labels(&child)
            .iter()
            .any(|text| text == l10n::setup_detect_dav_note())
    );
    assert!(check_button(&child, l10n::setup_detect_mail_enable()).is_none());
    assert!(check_button(&child, l10n::setup_detect_calendar_enable()).is_some());
    assert!(check_button(&child, l10n::setup_detect_contacts_enable()).is_some());
    assert_eq!(
        visible_entries(&child).len(),
        1,
        "the password, and only it"
    );
}

/// Back from the second step goes to the address, which stays as it was typed
/// (`docs/account-autodetect.md` rule 12).
#[test]
fn back_keeps_the_address() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Google {
            email: "person@gmail.com".to_owned(),
        },
        String::new(),
    ));
    state.back_to_address(false);
    assert_eq!(state.start_email, "person@gmail.com");
    assert!(state.form.is_none());
    assert!(!matches!(state.form, Some(SetupForm::Detected(_))));
}

/// The manual form's calendar-and-contacts type: no mail server, the address book and the
/// calendar typed, and an account set up for those alone.
pub(super) fn the_manual_form_sets_up_calendar_and_contacts_alone(window: &adw::ApplicationWindow) {
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

    assert!(
        rendered_labels(&child)
            .iter()
            .any(|text| text == l10n::setup_dav_note())
    );
    let fields = visible_entries(&child);
    let placeholder = |field: &gtk::Entry| field.placeholder_text().unwrap_or_default();
    assert_eq!(
        fields.iter().map(placeholder).collect::<Vec<_>>(),
        [
            l10n::setup_field_username(),
            l10n::setup_field_password(),
            l10n::setup_field_caldav_optional(),
            l10n::setup_field_carddav_optional(),
        ],
        "no mail server is asked for"
    );
    assert_eq!(fields[0].text(), "alice@cloud.example.test");
    fields[1].set_text("secret");
    fields[3].set_text("https://cloud.example.test/dav");
    descendant_button(&child, l10n::action_connect()).emit_clicked();
    let Some(AppInput::SubmitAccount(submission)) = receiver.recv_sync() else {
        panic!("Connect submits the account");
    };
    let AccountSubmission::Imap(submission) = *submission else {
        panic!("a password account");
    };
    assert_eq!(submission.uses, Some(vec![AccountCapability::Contacts]));
    assert_eq!(submission.carddav_url, "https://cloud.example.test/dav");
    assert!(submission.imap_host.is_empty());
    let config = AccountSubmission::Imap(submission)
        .config_toml()
        .expect("an address book alone is an account");
    assert!(config.contains("[carddav]") && !config.contains("[imap]"));
}

/// The manual IMAP form takes an address book of its own beside the mail server.
pub(super) fn the_manual_mail_form_takes_an_address_book(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.select_account_kind(ManualForm {
        email: "alice@example.test".to_owned(),
        ..ManualForm::default()
    });
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("manual IMAP content");
    let field = |placeholder: &str| {
        visible_entries(&child)
            .into_iter()
            .find(|field| field.placeholder_text().as_deref() == Some(placeholder))
            .unwrap_or_else(|| panic!("{placeholder}"))
    };
    field(l10n::setup_field_mail_server()).set_text("imap.example.test");
    field(l10n::setup_field_password()).set_text("secret");
    field(l10n::setup_field_carddav_optional()).set_text("https://contacts.example.test");
    descendant_button(&child, l10n::action_connect()).emit_clicked();
    let Some(AppInput::SubmitAccount(submission)) = receiver.recv_sync() else {
        panic!("Connect submits the account");
    };
    let AccountSubmission::Imap(submission) = *submission else {
        panic!("a password account");
    };
    assert_eq!(submission.carddav_url, "https://contacts.example.test");
}
