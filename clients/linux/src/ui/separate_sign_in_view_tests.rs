//! Setup's step and the account page's form, drawn. Functions rather than `#[test]`s, called
//! from the crate's single GTK test.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, UseProblem};

use super::{Prompts, page_action};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        mailbox::tests::rendered_labels,
        separate_sign_in::{
            SeparateFrom, SeparateInput,
            tests::{form, mailbox, server},
        },
        setup::SetupWindow,
        setup_state::SetupState,
        setup_widget_tests::{descendant_button, visible_entries},
    },
};

/// The next message sent, or `None` when nothing was.
fn sent(
    sender: &relm4::Sender<AppInput>,
    receiver: &relm4::Receiver<AppInput>,
) -> Option<AppInput> {
    sender.emit(AppInput::KeepEditing);
    match receiver.recv_sync()? {
        AppInput::KeepEditing => None,
        message => {
            assert!(matches!(receiver.recv_sync(), Some(AppInput::KeepEditing)));
            Some(message)
        }
    }
}

fn entry_for<'a>(fields: &'a [gtk::Entry], placeholder: &str) -> &'a gtk::Entry {
    fields
        .iter()
        .find(|field| field.placeholder_text().as_deref() == Some(placeholder))
        .unwrap_or_else(|| panic!("no {placeholder} field"))
}

/// After the add and before the link step: the server by its host, the login prefilled, a
/// password to type, Connect and Skip.
pub(crate) fn the_setup_step_asks_for_the_servers_password(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let account = "alice@example.test@imap.example.test";
    state.hold_separate(account, server(vec![AccountCapability::Calendar]));
    assert!(state.show_separate(account, account));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("the step");
    let shown = rendered_labels(&child);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::separate_server_title_calendar()),
        "{shown:?}"
    );
    let body = l10n::separate_server_body("dav.example.test");
    assert!(shown.contains(&body), "the server is named: {shown:?}");

    let fields = visible_entries(&child);
    let username = entry_for(&fields, l10n::setup_field_username());
    let password = entry_for(&fields, l10n::setup_field_password());
    assert_eq!(username.text(), "alice");
    assert!(
        !gtk::prelude::EntryExt::is_visible(password),
        "the password is hidden"
    );

    let connect = descendant_button(&child, l10n::action_connect());
    connect.emit_clicked();
    assert!(
        sent(&sender, &receiver).is_none(),
        "nothing is sent without a password"
    );
    password.set_text("secret");
    connect.emit_clicked();
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(SeparateInput::Connect {
            from: SeparateFrom::Setup,
            username,
            password,
        }))) if username == "alice" && password.0 == "secret"
    ));
    descendant_button(&child, l10n::separate_server_skip()).emit_clicked();
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(
            SeparateInput::Skip
        )))
    ));
}

/// The page's action opens the form; the form opens over Settings, holds its controls while a
/// connect runs and says a failure under the fields without being rebuilt, and closes when the
/// model lets it go. Closing it by hand says so.
pub(crate) fn the_page_form_opens_over_settings_and_updates_in_place() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let entry = mailbox(
        Some(UseProblem::NeedsOwnSignIn),
        Some(server(vec![AccountCapability::Calendar])),
    );
    let action = page_action(&entry, &sender).expect("the action");
    assert_eq!(
        action.title().as_str(),
        l10n::settings_account_use_separate_sign_in()
    );
    action.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(SeparateInput::Open { account, .. })))
            if account == entry.id
    ));
    assert!(page_action(&mailbox(None, None), &sender).is_none());

    let parent = gtk::Window::new();
    let mut prompts = Prompts::default();
    let mut shown = form(vec![AccountCapability::Calendar]);
    prompts.sync(None, Some(&shown), &parent, &sender);
    let window = prompts.form_window().expect("the form");
    assert!(window.is_visible());
    assert_eq!(window.transient_for().as_ref(), Some(&parent));
    assert_eq!(
        window.title().as_deref(),
        Some(l10n::separate_server_title_calendar())
    );
    let child = window.child().expect("the form's content");
    let fields = visible_entries(&child);
    let password = entry_for(&fields, l10n::setup_field_password()).clone();
    password.set_text("typed");

    shown.busy = true;
    prompts.sync(None, Some(&shown), &parent, &sender);
    assert_eq!(
        prompts.form_window().as_ref(),
        Some(&window),
        "redrawn in place"
    );
    assert!(!password.is_sensitive(), "held while the connect runs");

    shown.busy = false;
    shown.error = Some(l10n::signin_refused().to_owned());
    prompts.sync(None, Some(&shown), &parent, &sender);
    assert!(password.is_sensitive());
    assert_eq!(password.text(), "typed", "what was typed stands");
    assert!(
        rendered_labels(&child)
            .iter()
            .any(|text| text == l10n::signin_refused())
    );

    prompts.sync(None, None, &parent, &sender);
    assert!(!window.is_visible(), "closed once the model lets it go");
    assert!(
        sent(&sender, &receiver).is_none(),
        "a close the model asked for is not reported back"
    );

    shown.id = 2;
    prompts.sync(None, Some(&shown), &parent, &sender);
    let window = prompts.form_window().expect("the form again");
    window.close();
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(
            SeparateInput::Close
        )))
    ));
}
