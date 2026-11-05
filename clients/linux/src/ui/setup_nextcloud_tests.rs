//! Setup's sign-in with Nextcloud: which cards offer it, what the manual pane asks, and what a
//! press sends. The rules are plain `#[test]`s; the drawn cards are functions called from the
//! crate's single GTK test.

use adw::prelude::*;
use mailcal_bindings::{
    AccountCapability, DetectedSetup, MissReason, SetupChoice, SetupRecommendation,
};

use super::SetupNextcloudInput;
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::rendered_labels,
        nextcloud_flight::Grant,
        setup::SetupWindow,
        setup_links::Found,
        setup_model::{
            AccountKind, DetectedForm, ManualForm, SetupForm, detected_form, edit_manually,
        },
        setup_state::{Phase, SetupState},
        setup_widget_tests::{descendant_button, visible_entries},
    },
};

const CLOUD: &str = "https://cloud.example.test/remote.php/dav";

fn calendar_and_contacts(nextcloud_sign_in: bool) -> DetectedSetup {
    let found = |capability| SetupChoice {
        capability,
        on: true,
        server_found: true,
    };
    DetectedSetup {
        recommendation: SetupRecommendation::Manual {
            reason: MissReason::NothingFound,
        },
        calendar_and_contacts: true,
        caldav_url: Some(CLOUD.to_owned()),
        carddav_url: Some(CLOUD.to_owned()),
        choices: vec![
            found(AccountCapability::Calendar),
            found(AccountCapability::Contacts),
        ],
        nextcloud_sign_in,
        beside_account: false,
    }
}

fn manual_dav(caldav_url: &str) -> ManualForm {
    ManualForm {
        kind: AccountKind::Dav,
        email: "alice".to_owned(),
        caldav_url: caldav_url.to_owned(),
        ..ManualForm::default()
    }
}

#[test]
fn detection_s_answer_reaches_the_card_and_survives_editing_it_by_hand() {
    let SetupForm::Detected(detected) = detected_form(
        calendar_and_contacts(true),
        "alice@cloud.example.test".to_owned(),
    ) else {
        panic!("a detected card");
    };
    assert!(matches!(&detected, DetectedForm::Dav(form) if form.nextcloud));
    let SetupForm::Manual(manual) = edit_manually(&detected) else {
        panic!("the manual form");
    };
    assert_eq!(manual.nextcloud.as_deref(), Some(CLOUD));

    let SetupForm::Detected(plain) = detected_form(
        calendar_and_contacts(false),
        "alice@cloud.example.test".to_owned(),
    ) else {
        panic!("a detected card");
    };
    let SetupForm::Manual(manual) = edit_manually(&plain) else {
        panic!("the manual form");
    };
    assert_eq!(manual.nextcloud, None);
}

#[test]
fn servers_found_beside_a_jmap_server_carry_the_answer_to_the_link_step() {
    let SetupForm::Detected(detected) = detected_form(
        DetectedSetup {
            recommendation: SetupRecommendation::Jmap {
                email: "alice@example.test".to_owned(),
                server_url: "https://jmap.example.test".to_owned(),
                is_trusted: true,
                source: "https://example.test/.well-known/jmap".to_owned(),
            },
            calendar_and_contacts: false,
            ..calendar_and_contacts(true)
        },
        "alice@example.test".to_owned(),
    ) else {
        panic!("a detected card");
    };
    let beside = Found::of(&detected)
        .expect("servers beside")
        .offer_for("jmap-account");
    assert!(beside.nextcloud);
}

#[test]
fn the_manual_pane_asks_once_per_server_and_draws_only_the_answer_for_the_one_it_names() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(SetupForm::Manual(manual_dav("")));
    let shown = state.form_generation;
    assert_eq!(
        state.adopt_manual_dav(manual_dav("")),
        None,
        "nothing to ask yet"
    );
    assert_eq!(
        state.adopt_manual_dav(manual_dav(CLOUD)).as_deref(),
        Some(CLOUD)
    );
    assert_eq!(
        state.adopt_manual_dav(manual_dav(CLOUD)),
        None,
        "leaving the field again asks nothing"
    );
    assert_eq!(state.form_generation, shown, "asking redraws nothing");

    state.nextcloud_answered("https://elsewhere.example.test/dav", true);
    assert_eq!(state.form_generation, shown, "an answer for another server");
    state.nextcloud_answered(CLOUD, false);
    assert_eq!(
        state.form_generation, shown,
        "a no changes nothing on screen"
    );
    state.nextcloud_answered(CLOUD, true);
    assert_ne!(state.form_generation, shown, "a yes draws the offer");
    assert!(matches!(
        &state.form,
        Some(SetupForm::Manual(form)) if form.nextcloud.as_deref() == Some(CLOUD)
    ));
}

#[test]
fn a_sign_in_that_ends_without_an_account_goes_back_to_the_form_saying_why() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(SetupForm::Manual(manual_dav(CLOUD)));
    state.nextcloud_signing_in();
    assert_eq!(state.phase, Phase::NextcloudSigningIn);
    state.nextcloud_ended(Some(l10n::account_signin_timeout().to_owned()));
    assert_eq!(state.phase, Phase::Form);
    assert_eq!(state.error.as_deref(), Some(l10n::account_signin_timeout()));
    state.nextcloud_signing_in();
    state.nextcloud_ended(None);
    assert_eq!(state.error, None, "a cancel says nothing");
}

#[test]
fn a_kept_grant_is_for_its_server_only() {
    let mut state = SetupState::closed();
    state
        .nextcloud
        .hold(CLOUD.to_owned(), Grant("granted".to_owned()));
    assert_eq!(
        state.nextcloud.held_for("https://elsewhere.example.test"),
        None
    );
    assert_eq!(
        state.nextcloud.held_for(CLOUD),
        Some(Grant("granted".to_owned()))
    );
    assert_eq!(state.nextcloud.take(), Some(Grant("granted".to_owned())));
    assert_eq!(state.nextcloud.held_for(CLOUD), None);
}

fn started(receiver: &relm4::Receiver<AppInput>) -> (String, Vec<AccountCapability>) {
    match receiver.recv_sync() {
        Some(AppInput::SetupNextcloud(SetupNextcloudInput::Start { server, uses })) => {
            (server, uses)
        }
        other => panic!("not a start: {other:?}"),
    }
}

/// A calendar and contacts card at a Nextcloud server leads with its sign-in, under the line that
/// explains it, and draws no field until the password is asked for (`docs/mail-oauth.md`
/// rule 2). The press signs in at the server found, for what the card chose.
pub(crate) fn a_nextcloud_card_leads_with_its_sign_in(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(detected_form(
        calendar_and_contacts(true),
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
            .any(|text| text == l10n::nextcloud_sign_in_note())
    );
    let sign_in = descendant_button(&child, l10n::nextcloud_sign_in_button());
    assert!(sign_in.has_css_class("suggested-action"));
    assert!(
        visible_entries(&child).is_empty(),
        "no field before it is asked for"
    );
    sign_in.emit_clicked();
    assert_eq!(
        started(&receiver),
        (
            CLOUD.to_owned(),
            vec![AccountCapability::Calendar, AccountCapability::Contacts]
        )
    );

    descendant_button(&child, l10n::setup_imap_signin_password_instead()).emit_clicked();
    assert_eq!(
        visible_entries(&child).len(),
        1,
        "the password, once asked for"
    );
    assert!(!sign_in.has_css_class("suggested-action"));
}

/// The manual pane draws the sign-in above its fields once the server typed answered as
/// Nextcloud, and the fields stay; the press signs in there for the servers typed.
pub(crate) fn the_manual_pane_offers_nextcloud_above_its_fields(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(SetupForm::Manual(ManualForm {
        nextcloud: Some(CLOUD.to_owned()),
        ..manual_dav(CLOUD)
    }));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("the manual form");
    assert_eq!(
        visible_entries(&child).len(),
        4,
        "login, password and both servers"
    );
    descendant_button(&child, l10n::nextcloud_sign_in_button()).emit_clicked();
    assert_eq!(
        started(&receiver),
        (
            CLOUD.to_owned(),
            vec![AccountCapability::Calendar, AccountCapability::Contacts]
        )
    );
}

/// The card the link step opens for servers beside a JMAP account says it adds to that account,
/// not that the address has no mail server, and says some servers want an app password.
pub(crate) fn the_card_beside_a_jmap_account_says_what_it_adds(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(detected_form(
        DetectedSetup {
            beside_account: true,
            ..calendar_and_contacts(false)
        },
        "alice@example.test".to_owned(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("calendar-and-contacts content");
    let labels = rendered_labels(&child);
    assert!(
        labels
            .iter()
            .any(|text| text == l10n::setup_detect_dav_beside_note())
    );
    assert!(
        !labels
            .iter()
            .any(|text| text == l10n::setup_detect_dav_note())
    );
    assert!(
        labels
            .iter()
            .any(|text| text == l10n::setup_detect_app_password_hint())
    );
}
