//! Widget-level regressions for the account-setup window: which surface each route renders,
//! and what it must never put on screen.
//!
//! Functions rather than `#[test]`s, called from the crate's single GTK test: see
//! [`super::mailbox::thread_tests`] for why there is exactly one.

use adw::prelude::*;
use gtk::glib::types::StaticType;
use mailcal_bindings::{ConnectionSecurity, DetectedServerRow, SetupRecommendation};

use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::rendered_labels,
        setup::{SetupState, SetupWindow},
        setup_model::recommendation_form,
    },
};

pub(super) fn the_setup_window_offers_each_route_its_own_surface() {
    let application = adw::Application::builder()
        .application_id(format!("{}.setup-test", crate::l10n::APP_ID))
        .build();
    application
        .register(None::<&gtk::gio::Cancellable>)
        .expect("register test application");
    let window = adw::ApplicationWindow::new(&application);

    super::setup_manual_tests::a_guarded_welcome_dismisses_on_consent(&window);
    super::setup_manual_tests::required_phases_swap_content_instead_of_stacking(&window);
    an_oauth_route_never_asks_for_a_password(&window);
    a_detected_imap_card_confirms_servers_rather_than_asking_for_them(&window);
    an_untrusted_card_holds_connect_until_it_is_approved(&window);
    super::setup_manual_tests::the_manual_form_switches_account_type(&window);
    super::setup_manual_tests::a_miss_explains_itself_on_the_manual_form(&window);
    super::setup_manual_tests::a_dismissible_window_cancels_the_flow(&window);
}

/// Microsoft retired Basic auth and Google is a native-API integration, so neither route may
/// put a password box on screen; nor a server field there is nothing to type into.
fn an_oauth_route_never_asks_for_a_password(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);

    for (recommendation, action) in [
        (
            SetupRecommendation::Google {
                email: "person@gmail.com".to_owned(),
            },
            l10n::setup_google_signin(),
        ),
        (
            SetupRecommendation::Microsoft {
                email: "person@outlook.com".to_owned(),
            },
            l10n::setup_microsoft_signin(),
        ),
    ] {
        state.show_form(recommendation_form(recommendation, String::new()));
        setup.render(&state, window, &sender);
        let child = setup
            .current_window()
            .and_then(|window| window.child())
            .expect("detected OAuth content");
        assert!(
            descendant_has_button(&child, action),
            "{action} must be the way in"
        );
        assert_eq!(
            descendant_count::<gtk::Entry>(&child),
            0,
            "{action}: an OAuth route must never expose server, username, or password fields"
        );
    }

    // While the pre-flight is still asking, the card offers neither: a secret field that
    // appears and is then taken away reads as the app changing its mind.
    state.show_form(recommendation_form(
        SetupRecommendation::Jmap {
            email: "alice@example.test".to_owned(),
            server_url: "https://jmap.example.test".to_owned(),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("JMAP card mid-probe");
    assert_eq!(descendant_count::<gtk::Entry>(&child), 0);
    assert!(!descendant_has_button(
        &child,
        l10n::setup_jmap_signin_button()
    ));
    let shown = rendered_labels(&child);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_jmap_signin_checking()),
        "the card says what it is waiting for: {shown:?}"
    );

    // A detected JMAP server that advertises sign-in takes the secret away entirely; a failed
    // sign-in hands it back; and only it, since detection already found the server.
    state.show_form(recommendation_form(
        SetupRecommendation::Jmap {
            email: "alice@example.test".to_owned(),
            server_url: "https://jmap.example.test".to_owned(),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
    assert!(state.jmap_oauth_available("alice@example.test", "https://jmap.example.test", true));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("JMAP setup content");
    assert!(descendant_has_button(
        &child,
        l10n::setup_jmap_signin_button()
    ));
    assert_eq!(
        descendant_count::<gtk::Entry>(&child),
        0,
        "a detected JMAP OAuth offer replaces the secret field"
    );
    let shown = rendered_labels(&child);
    assert!(
        shown.iter().any(|text| text == "jmap.example.test"),
        "the discovered server must be shown for the user to recognize: {shown:?}"
    );

    state.jmap_sign_in_failed();
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("failed JMAP setup content");
    assert!(descendant_has_button(
        &child,
        l10n::setup_jmap_signin_button()
    ));
    assert_eq!(
        descendant_count::<gtk::Entry>(&child),
        1,
        "a failed provider sign-in must restore the secret field, and only it"
    );
}

/// The detected IMAP card asks for the one thing detection cannot find; the password; and
/// shows the servers it did find rather than making the user retype them.
fn a_detected_imap_card_confirms_servers_rather_than_asking_for_them(
    window: &adw::ApplicationWindow,
) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Imap {
            email: "alice@example.test".to_owned(),
            imap_host: "imap.example.test:993".to_owned(),
            smtp_host: Some("smtp.example.test:465".to_owned()),
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            incoming: server_row("IMAP", "imap.example.test", 993),
            outgoing: Some(server_row("SMTP", "smtp.example.test", 465)),
            caldav_url: Some("https://caldav.example.test/dav".to_owned()),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content");

    let shown = rendered_labels(&child);
    for expected in [
        "imap.example.test:993 · TLS",
        "smtp.example.test:465 · TLS",
        "caldav.example.test",
    ] {
        assert!(
            shown.iter().any(|text| text == expected),
            "the card must show {expected}: {shown:?}"
        );
    }
    assert!(
        entries(&child).iter().all(|field| field.text().is_empty()),
        "a detected server belongs on a row, not in a field to retype"
    );
    let calendar = check_button(&child, l10n::setup_detect_calendar_enable())
        .expect("a discovered calendar must be offered");
    assert!(
        calendar.is_active(),
        "a discovered calendar is opt-out, not opt-in"
    );

    // Nothing discovered: the offer is opt-in, and the box to type an endpoint into stays
    // hidden until it is accepted; an empty field beside an unticked toggle reads as a
    // calendar the account has and we failed to fill in.
    state.show_form(recommendation_form(
        SetupRecommendation::Imap {
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
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content without a calendar");
    let calendar = check_button(&child, l10n::setup_detect_calendar_add())
        .expect("a calendar must still be offerable by hand");
    assert!(!calendar.is_active());
    let on_screen = visible_entries(&child);
    assert_eq!(
        on_screen.len(),
        1,
        "the manual calendar field must stay hidden until the offer is accepted"
    );
    assert!(
        // `EntryExt::is_visible` is the masking property, not the widget's visibility.
        !gtk::prelude::EntryExt::is_visible(&on_screen[0]),
        "the one field on screen is the password"
    );
    calendar.set_active(true);
    assert_eq!(
        visible_entries(&child).len(),
        2,
        "accepting it reveals the field to type an endpoint into"
    );
}

/// An untrusted recommendation may not send a credential until the user says the server names
/// are right; and the button says so, rather than going quiet when pressed.
fn an_untrusted_card_holds_connect_until_it_is_approved(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Imap {
            email: "alice@example.test".to_owned(),
            imap_host: "imap.example.test:993".to_owned(),
            smtp_host: None,
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            incoming: server_row("IMAP", "imap.example.test", 993),
            outgoing: None,
            caldav_url: None,
            is_trusted: false,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("untrusted IMAP content");

    let connect = descendants::<gtk::Button>(&child)
        .into_iter()
        .find(|button| button.label().as_deref() == Some(l10n::action_connect()))
        .expect("a Connect button");
    assert!(!connect.is_sensitive(), "an unapproved card cannot connect");
    let approval = check_button(&child, l10n::setup_detect_trust_confirm())
        .expect("the approval must be on screen");
    assert!(approval.is_visible() && !approval.is_active());
    approval.set_active(true);
    assert!(connect.is_sensitive(), "approving it opens Connect");
}

pub(super) fn server_row(protocol: &str, hostname: &str, port: u16) -> DetectedServerRow {
    DetectedServerRow {
        protocol: protocol.to_owned(),
        hostname: hostname.to_owned(),
        port,
        security: "TLS".to_owned(),
        username: "alice@example.test".to_owned(),
    }
}

pub(super) fn descendant_count<T: IsA<gtk::Widget> + StaticType>(root: &gtk::Widget) -> usize {
    let mut count = usize::from(root.type_().is_a(T::static_type()));
    let mut child = root.first_child();
    while let Some(widget) = child {
        count += descendant_count::<T>(&widget);
        child = widget.next_sibling();
    }
    count
}

pub(super) fn descendants<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    if let Some(widget) = root.downcast_ref::<T>() {
        found.push(widget.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.extend(descendants::<T>(&widget));
        child = widget.next_sibling();
    }
    found
}

pub(super) fn entries(root: &gtk::Widget) -> Vec<gtk::Entry> {
    descendants::<gtk::Entry>(root)
}

/// The entries actually on screen. `gtk::Entry` has two `is_visible`es: `WidgetExt`'s, which
/// is what "on screen" means, and `EntryExt`'s, which is whether the text is masked.
pub(super) fn visible_entries(root: &gtk::Widget) -> Vec<gtk::Entry> {
    entries(root)
        .into_iter()
        .filter(gtk::prelude::WidgetExt::is_visible)
        .collect()
}

pub(super) fn drop_down(root: &gtk::Widget) -> Option<gtk::DropDown> {
    descendants::<gtk::DropDown>(root).into_iter().next()
}

pub(super) fn check_button(root: &gtk::Widget, label: &str) -> Option<gtk::CheckButton> {
    descendants::<gtk::CheckButton>(root)
        .into_iter()
        .find(|button| button.label().as_deref() == Some(label))
}

pub(super) fn descendant_has_button(root: &gtk::Widget, label: &str) -> bool {
    descendants::<gtk::Button>(root)
        .iter()
        .any(|button| button.label().as_deref() == Some(label))
}
