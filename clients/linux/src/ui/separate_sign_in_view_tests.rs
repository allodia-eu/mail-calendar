//! Setup's step and the account page's form, drawn. Functions rather than `#[test]`s, called
//! from the crate's single GTK test.

use adw::prelude::*;
use mailcal_bindings::{
    AccountCapability, AccountChangeRefusal, AccountSeparateSignIn, BrowserSignIn,
    BrowserSignInStage, ControlState,
};

use super::{Prompts, page_action};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        controls::tests::spins,
        mailbox::tests::rendered_labels,
        separate_sign_in::{
            SeparateFrom, SeparateInput,
            tests::{form, section},
        },
        setup::SetupWindow,
        setup_state::SetupState,
        setup_widget_tests::{descendants, visible_entries},
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

/// A button the control components drew, by the label they put inside it: a button of its own
/// label is not one.
fn button(root: &gtk::Widget, label: &str) -> gtk::Button {
    descendants::<gtk::Button>(root)
        .into_iter()
        .find(|button| {
            button.label().is_none()
                && descendants::<gtk::Label>(button.upcast_ref())
                    .iter()
                    .any(|text| text.label() == label)
        })
        .unwrap_or_else(|| panic!("a {label} button"))
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
    state.hold_separate(account, section(vec![AccountCapability::Calendar]));
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

    let connect = button(&child, l10n::action_connect());
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
    button(&child, l10n::separate_server_skip()).emit_clicked();
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
    let account = "alice@example.test@imap.example.test";
    let action = page_action(account, true, &sender);
    assert_eq!(
        action.title().as_str(),
        l10n::settings_account_use_separate_sign_in()
    );
    action.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(SeparateInput::Open { account: opened })))
            if opened == account
    ));
    // Held while a change runs on the page, as every control on it is.
    assert!(!page_action(account, false, &sender).is_sensitive());

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

    shown.section.connect = ControlState {
        enabled: false,
        running: true,
    };
    prompts.sync(None, Some(&shown), &parent, &sender);
    assert_eq!(
        prompts.form_window().as_ref(),
        Some(&window),
        "redrawn in place"
    );
    assert!(!password.is_sensitive(), "held while the connect runs");
    let connect = button(&child, l10n::action_connect());
    assert!(spins(&connect), "Connect spins while it runs");
    assert!(
        !button(&child, l10n::action_cancel()).is_sensitive(),
        "and Cancel waits for it"
    );

    shown.section.connect = ControlState {
        enabled: true,
        running: false,
    };
    shown.section.failed = Some(AccountChangeRefusal::SignInRefused);
    prompts.sync(None, Some(&shown), &parent, &sender);
    assert!(password.is_sensitive());
    assert!(!spins(&button(&child, l10n::action_connect())));
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

/// Setup's Skip spins while the core switches the uses off, and holds Connect.
pub(crate) fn the_setup_steps_skip_spins_while_it_runs(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let account = "alice@example.test@imap.example.test";
    let mut skipping = section(vec![AccountCapability::Calendar]);
    skipping.stop_using = ControlState {
        enabled: false,
        running: true,
    };
    skipping.connect.enabled = false;
    state.hold_separate(account, skipping);
    assert!(state.show_separate(account, account));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("the step");
    assert!(spins(&button(&child, l10n::separate_server_skip())));
    assert!(!button(&child, l10n::action_connect()).is_sensitive());
}

/// Whether `widget` and everything it sits in are shown.
fn shown(widget: &impl IsA<gtk::Widget>) -> bool {
    let mut at = Some(widget.clone().upcast::<gtk::Widget>());
    while let Some(widget) = at {
        if !widget.is_visible() {
            return false;
        }
        at = widget.parent();
    }
    true
}

/// Whether `first` is drawn before `second` under `root`.
fn precedes(root: &gtk::Widget, first: &gtk::Widget, second: &gtk::Widget) -> bool {
    let order = descendants::<gtk::Widget>(root);
    let at = |widget: &gtk::Widget| order.iter().position(|drawn| drawn == widget);
    at(first) < at(second)
}

/// What the step or form sent about its browser sign-in: a press of it (`true`) or of its
/// Cancel (`false`), and from where.
fn sign_in_sent(
    sender: &relm4::Sender<AppInput>,
    receiver: &relm4::Receiver<AppInput>,
) -> Option<(SeparateFrom, bool)> {
    match sent(sender, receiver)? {
        AppInput::Accounts(AccountsInput::Separate(SeparateInput::SignIn { from })) => {
            Some((from, true))
        }
        AppInput::Accounts(AccountsInput::Separate(SeparateInput::CancelSignIn { from })) => {
            Some((from, false))
        }
        _ => panic!("an input that is not the sign-in's"),
    }
}

const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};

const RUNNING: ControlState = ControlState {
    enabled: false,
    running: true,
};

/// The core's sign-in waiting on the browser, `redirect_uri` `None` for Nextcloud's.
fn in_browser(redirect_uri: Option<&str>) -> BrowserSignIn {
    BrowserSignIn {
        attempt: 7,
        account: "alice@example.test@imap.example.test".to_owned(),
        stage: BrowserSignInStage::InBrowser {
            url: "https://dav.example.test/authorize".to_owned(),
            redirect_uri: redirect_uri.map(str::to_owned),
        },
        cancel: ControlState {
            enabled: true,
            running: false,
        },
    }
}

/// The step stands as drawn until the server answers; an answer adds the sign-in above the
/// fields, in place, so a password being typed stands. It runs as the core says, and while the
/// browser holds it a line and its Cancel are drawn, the fields held as the core holds Connect,
/// and a fresh press is still sent.
pub(crate) fn the_setup_step_offers_the_sign_in_in_place(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let account = "alice@example.test@imap.example.test";
    state.hold_separate(account, section(vec![AccountCapability::Calendar]));
    assert!(state.show_separate(account, account));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("the step");
    let sign_in = || button(&child, l10n::setup_imap_signin_button());
    assert!(!shown(&sign_in()), "nothing is offered before the answer");
    let fields = visible_entries(&child);
    let username = entry_for(&fields, l10n::setup_field_username()).clone();
    let password = entry_for(&fields, l10n::setup_field_password()).clone();
    password.set_text("half typed");

    let step = state.separate.as_mut().expect("the step");
    step.section.sign_in = Some(AccountSeparateSignIn::Provider);
    state.report();
    setup.render(&state, window, &sender);
    assert_eq!(
        setup.current_window().and_then(|window| window.child()),
        Some(child.clone()),
        "updated in place"
    );
    assert_eq!(password.text(), "half typed", "what was typed stands");
    assert!(shown(&sign_in()) && shown(&password));
    assert!(precedes(
        &child,
        sign_in().upcast_ref(),
        username.upcast_ref()
    ));
    assert!(
        sign_in().has_css_class("suggested-action"),
        "the sign-in leads"
    );
    assert!(
        rendered_labels(&child)
            .iter()
            .any(|text| text == l10n::setup_imap_signin_note())
    );
    let cancel = || {
        descendants::<gtk::Button>(&child)
            .into_iter()
            .find(|button| {
                descendants::<gtk::Label>(button.upcast_ref())
                    .iter()
                    .any(|text| text.label() == l10n::action_cancel())
            })
    };
    assert!(cancel().is_none(), "nothing to cancel yet");
    sign_in().emit_clicked();
    assert_eq!(
        sign_in_sent(&sender, &receiver),
        Some((SeparateFrom::Setup, true))
    );

    let step = state.separate.as_mut().expect("the step");
    step.section.sign_in_control = RUNNING;
    step.section.connect = HELD;
    state.report();
    setup.render(&state, window, &sender);
    assert!(spins(&sign_in()) && !sign_in().is_sensitive());
    assert!(!password.is_sensitive(), "held as the core holds Connect");

    let step = state.separate.as_mut().expect("the step");
    step.section.sign_in_control = ControlState {
        enabled: true,
        running: false,
    };
    step.section.browser = Some(in_browser(Some("http://127.0.0.1:1/")));
    state.report();
    setup.render(&state, window, &sender);
    assert!(
        rendered_labels(&child)
            .iter()
            .any(|text| text == &l10n::settings_account_signin_waiting("alice")),
        "the line says it waits on the browser"
    );
    assert!(
        sign_in().is_sensitive(),
        "pressable again while the browser waits"
    );
    cancel().expect("the sign-in's Cancel").emit_clicked();
    assert_eq!(
        sign_in_sent(&sender, &receiver),
        Some((SeparateFrom::Setup, false))
    );
    sign_in().emit_clicked();
    assert_eq!(
        sign_in_sent(&sender, &receiver),
        Some((SeparateFrom::Setup, true)),
        "a fresh press is sent while the browser waits"
    );
}

/// The page's form offers the sign-in the same way, in place, and its Cancel and Connect stay.
pub(crate) fn the_page_form_offers_the_sign_in_in_place() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let parent = gtk::Window::new();
    let mut prompts = Prompts::default();
    let mut shown_form = form(vec![AccountCapability::Calendar]);
    prompts.sync(None, Some(&shown_form), &parent, &sender);
    let window = prompts.form_window().expect("the form");
    let child = window.child().expect("the form's content");
    let sign_in = || button(&child, l10n::setup_imap_signin_button());
    assert!(!shown(&sign_in()));

    shown_form.section.sign_in = Some(AccountSeparateSignIn::Provider);
    prompts.sync(None, Some(&shown_form), &parent, &sender);
    assert_eq!(prompts.form_window().as_ref(), Some(&window));
    assert!(shown(&sign_in()));
    let connect = button(&child, l10n::action_connect());
    assert!(shown(&connect) && !connect.has_css_class("suggested-action"));
    assert!(
        shown(&button(&child, l10n::action_cancel())),
        "the form's own Cancel stays"
    );
    sign_in().emit_clicked();
    assert_eq!(
        sign_in_sent(&sender, &receiver),
        Some((SeparateFrom::Page, true))
    );
    window.close();
    assert!(matches!(
        sent(&sender, &receiver),
        Some(AppInput::Accounts(AccountsInput::Separate(
            SeparateInput::Close
        )))
    ));
}

/// At a Nextcloud server the same place offers Nextcloud's own sign-in, in place, and says where
/// it waits while the browser holds it.
pub(crate) fn the_setup_step_offers_nextcloud_in_place(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    let account = "alice@example.test@imap.example.test";
    state.hold_separate(account, section(vec![AccountCapability::Calendar]));
    assert!(state.show_separate(account, account));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("the step");
    let password = entry_for(&visible_entries(&child), l10n::setup_field_password()).clone();
    password.set_text("half typed");

    let step = state.separate.as_mut().expect("the step");
    step.section.sign_in = Some(AccountSeparateSignIn::Nextcloud);
    state.report();
    setup.render(&state, window, &sender);
    assert_eq!(password.text(), "half typed", "what was typed stands");
    let sign_in = || button(&child, l10n::nextcloud_sign_in_button());
    assert!(shown(&sign_in()) && sign_in().has_css_class("suggested-action"));
    let labels = rendered_labels(&child);
    assert!(
        labels
            .iter()
            .any(|text| text == l10n::nextcloud_sign_in_note())
    );
    sign_in().emit_clicked();
    assert_eq!(
        sign_in_sent(&sender, &receiver),
        Some((SeparateFrom::Setup, true))
    );

    let step = state.separate.as_mut().expect("the step");
    step.section.browser = Some(in_browser(None));
    state.report();
    setup.render(&state, window, &sender);
    let labels = rendered_labels(&child);
    assert!(
        labels
            .iter()
            .any(|text| text == l10n::nextcloud_sign_in_waiting())
    );
    assert!(shown(&button(&child, l10n::action_cancel())));
}
