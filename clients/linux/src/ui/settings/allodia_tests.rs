//! What the Allodia-account card draws in each of its states, and what it must never draw.
//!
//! Rendering is asserted on the **rendered** labels rather than on `ActionRow::title()`, which
//! reads back the string it was handed whatever became of the label; see
//! [`crate::ui::mailbox::tests::rendered_labels`]. Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::{
    AccountChangeRefusal, AllodiaCard, AllodiaCardState, AllodiaFailure, AllodiaOnboarding,
    BrowserSignIn, BrowserSignInStage, ControlState,
};

use super::{
    allodia, create_button, failure_row, failure_said, session_ended_row, sign_in_button,
    signed_in_row, signing_in_row,
};
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::{every_row_belongs_to_a_list, glib_records, rendered_labels},
    },
};

fn account(email: &str, name: Option<&str>) -> mailcal_bindings::AllodiaAccount {
    mailcal_bindings::AllodiaAccount {
        // None, because nothing this screen draws reads it: the id is what a store purchase is
        // tagged with, and Linux sells through Allodia's own checkout rather than a store.
        id: None,
        email: email.to_owned(),
        name: name.map(str::to_owned),
    }
}

/// A control that can be pressed and runs nothing.
const IDLE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn shown(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    rendered_labels(widget.as_ref())
}

/// The address identifies the account and the name is a courtesy the service may not hold, so the
/// address is always the title and the name only ever the subtitle.
pub(crate) fn the_card_names_the_account_by_address_and_offers_a_way_out() {
    let (sender, receiver) = relm4::channel();

    let row = signed_in_row(
        &sender,
        &account("someone@allodia.test", Some("A Person")),
        IDLE,
    );
    let labels = shown(&row);
    assert!(
        labels
            .iter()
            .any(|label| label.contains("someone@allodia.test")),
        "the address is what identifies the account: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "A Person"),
        "the name rides along as the subtitle: {labels:?}"
    );

    let sign_out = descendant_button(&row, l10n::settings_allodia_sign_out());
    sign_out.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SignOutOfAllodia)
    ));
}

/// A service that holds no name, and one that holds an empty string, are the same thing: a row
/// with an address and no subtitle. An empty subtitle would leave a blank second line.
pub(crate) fn a_nameless_account_gets_no_empty_second_line() {
    let (sender, _receiver) = relm4::channel();

    for name in [None, Some("")] {
        let row = signed_in_row(&sender, &account("someone@allodia.test", name), IDLE);
        assert!(
            row.subtitle().is_none_or(|subtitle| subtitle.is_empty()),
            "a missing name must not become a blank line: {name:?}"
        );
    }
}

/// The signed-out state is a button and no row; the pending state is a row with a way out and no
/// button to press again.
pub(crate) fn each_state_offers_exactly_one_action() {
    let (sender, receiver) = relm4::channel();

    let sign_in = sign_in_button(&sender, IDLE);
    assert_eq!(
        text_of(&sign_in).as_deref(),
        Some(l10n::settings_allodia_sign_in())
    );
    sign_in.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));

    let pending = signing_in_row(&sender, IDLE);
    let labels = shown(&pending);
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::settings_allodia_signing_in()),
        "the pending row says so: {labels:?}"
    );
    // A dismissed browser gives the listener nothing until its five-minute cap, so the only way
    // out of this state is the row's own button.
    let cancel = descendant_button(&pending, l10n::action_cancel());
    cancel.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::CancelAllodiaSignIn)
    ));
}

/// Signed out offers both routes, and they are different routes.
///
/// Someone who has no account and someone returning to one need different pages, and a lone "Sign
/// in" sends the first of them through a form asking for a password they never set. The failure
/// this pins is the cheap one: both buttons wired to the same input, which looks right on screen
/// and lands everybody on the sign-in page.
pub(crate) fn signed_out_offers_creating_as_well_as_signing_in() {
    let (sender, receiver) = relm4::channel();

    let create = create_button(&sender, IDLE);
    assert_eq!(
        text_of(&create).as_deref(),
        Some(l10n::settings_allodia_create())
    );
    create.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaRegistration)
    ));
}

/// Signed in, the account page is reachable and deletion is named on the screen.
///
/// "Manage account" is not the word anybody looks for when they want out, and an app that lets
/// someone create an account has to offer deletion somewhere findable. Both reach the same page.
pub(crate) fn a_signed_in_account_can_be_managed_deleted_and_left() {
    let (sender, receiver) = relm4::channel();
    let row = signed_in_row(&sender, &account("someone@example.com", None), IDLE);

    descendant_button(&row, l10n::settings_allodia_manage()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ManageAllodiaAccount)
    ));

    descendant_button(&row, l10n::settings_allodia_delete()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ManageAllodiaAccount)
    ));

    descendant_button(&row, l10n::settings_allodia_sign_out()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SignOutOfAllodia)
    ));
}

/// A sign-in the service refused names the account and says it ended, and offers the two ways on:
/// signing in again and signing out. Nothing in it may claim the person is still signed in, and
/// the account page is not offered.
pub(crate) fn a_refused_sign_in_says_it_ended_and_offers_both_ways() {
    let (sender, receiver) = relm4::channel();
    let row = session_ended_row(
        &sender,
        &account("someone@allodia.test", Some("A Person")),
        IDLE,
        IDLE,
    );

    let labels = shown(&row);
    let ended = l10n::settings_allodia_session_ended("someone@allodia.test");
    assert!(
        labels.contains(&ended),
        "the row names the address whose sign-in ended: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::settings_allodia_session_ended_hint()),
        "and says what signing in again is for: {labels:?}"
    );
    let signed_in = l10n::settings_allodia_signed_in("someone@allodia.test");
    assert!(
        !labels.contains(&signed_in),
        "a refused grant is not a signed-in account: {labels:?}"
    );

    let buttons: Vec<String> =
        crate::ui::setup_widget_tests::descendants::<gtk::Button>(row.upcast_ref())
            .iter()
            .filter_map(text_of)
            .collect();
    assert_eq!(
        buttons,
        [
            l10n::settings_allodia_reauth_action(),
            l10n::settings_allodia_sign_out()
        ],
        "exactly the two ways on, and no Manage account"
    );

    descendant_button(&row, l10n::settings_allodia_reauth_action()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));
    descendant_button(&row, l10n::settings_allodia_sign_out()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SignOutOfAllodia)
    ));
}

/// The address comes from the account service and the failure text from the core, so both are
/// strings this client did not write. A bare ampersand renders the row **blank** when it is parsed
/// as markup, and a markup-shaped one is *applied* rather than shown.
pub(crate) fn neither_the_address_nor_the_failure_is_parsed_as_markup() {
    let (sender, _receiver) = relm4::channel();
    let hostile = "ada&grace@allodia.test";

    let (rows, records) = glib_records(|| {
        [
            signed_in_row(&sender, &account(hostile, Some("Bell & Co")), IDLE),
            session_ended_row(&sender, &account(hostile, None), IDLE, IDLE),
            failure_row("Signing in didn't work: bad & wrong"),
        ]
    });

    // Rendering alone cannot see this half: libadwaita re-applies the labels when the flag flips,
    // so a row built the wrong way still reads correctly and only warns.
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "an address or a failure must not be parsed as markup: {records:?}"
    );
    for row in &rows {
        assert!(!row.uses_markup());
    }
    let labels: Vec<String> = rows.iter().flat_map(shown).collect();
    assert!(
        labels.iter().any(|label| label.contains(hostile)),
        "an address with an ampersand still reaches the screen: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label.contains("Bell & Co")),
        "so does a name with one: {labels:?}"
    );
}

/// Every row the card draws has to sit in a list box or the keyboard skips it; and the group is
/// what supplies the list, so a row added anywhere else still renders and is still unreachable.
pub(crate) fn every_card_row_is_reachable_from_the_keyboard() {
    let (sender, _receiver) = relm4::channel();
    let group = super::group(
        l10n::settings_allodia_heading(),
        l10n::settings_allodia_description(),
    );
    group.add(&signed_in_row(
        &sender,
        &account("someone@allodia.test", None),
        IDLE,
    ));
    group.add(&failure_row("Signing in didn't work"));
    group.set_header_suffix(Some(&sign_in_button(&sender, IDLE)));

    every_row_belongs_to_a_list(group.upcast_ref::<gtk::Widget>());
}

/// What a button says: its own label, or the label inside a control component's.
fn text_of(button: &gtk::Button) -> Option<String> {
    button.label().map(String::from).or_else(|| {
        crate::ui::setup_widget_tests::descendants::<gtk::Label>(button.upcast_ref())
            .first()
            .map(|label| label.label().to_string())
    })
}

fn descendant_button(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::Button {
    crate::ui::setup_widget_tests::descendants::<gtk::Button>(root.as_ref())
        .into_iter()
        .find(|button| text_of(button).as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no button labelled {label:?}"))
}

fn card(state: AllodiaCardState) -> AllodiaCard {
    AllodiaCard {
        state,
        browser: None,
        sign_in: IDLE,
        create: IDLE,
        sign_out: IDLE,
        failure: None,
        end_session: None,
        onboarding: AllodiaOnboarding::Offering,
    }
}

/// The card is the core's: a sign-in out is drawn in place of every button, whatever the state
/// under it, a refused sign-in as ended, and what the last one came to in words.
pub(crate) fn the_card_draws_what_the_core_says() {
    let (sender, _receiver) = relm4::channel();
    let ended = AllodiaCardState::Ended {
        account: account("someone@allodia.test", None),
    };

    let mut out = card(ended.clone());
    out.browser = Some(BrowserSignIn {
        attempt: 1,
        account: "allodia-account".to_owned(),
        stage: BrowserSignInStage::Starting,
        cancel: IDLE,
    });
    let labels = shown(&allodia(&out, &sender));
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::settings_allodia_signing_in()),
        "a sign-in out is drawn as signing in: {labels:?}"
    );
    assert!(
        !labels
            .iter()
            .any(|label| label == l10n::settings_allodia_reauth_action()),
        "with no button to press again: {labels:?}"
    );

    let labels = shown(&allodia(&card(ended), &sender));
    assert!(
        labels.contains(&l10n::settings_allodia_session_ended(
            "someone@allodia.test"
        )),
        "{labels:?}"
    );

    let mut failed = card(AllodiaCardState::SignedOut);
    failed.failure = Some(AllodiaFailure::SignIn {
        refusal: AccountChangeRefusal::NotFinished,
    });
    let labels = shown(&allodia(&failed, &sender));
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::settings_allodia_timeout()),
        "{labels:?}"
    );
}

/// Each way a sign-in can fail keeps the words the card has always had for it.
pub(crate) fn a_failure_is_said_in_the_cards_words() {
    let said = |refusal| failure_said(&AllodiaFailure::SignIn { refusal });
    assert_eq!(
        said(AccountChangeRefusal::StaleSignIn),
        l10n::signin_stale()
    );
    assert_eq!(
        said(AccountChangeRefusal::BrowserNotOpened),
        l10n::settings_allodia_browser_failed()
    );
    assert_eq!(
        said(AccountChangeRefusal::Other {
            detail: "the service said no".to_owned()
        }),
        l10n::settings_allodia_failed("the service said no")
    );
    assert_eq!(
        failure_said(&AllodiaFailure::SignOut {
            detail: "locked".to_owned()
        }),
        l10n::settings_allodia_failed("locked")
    );
}
