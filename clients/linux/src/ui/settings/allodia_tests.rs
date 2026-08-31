//! What the Allodia-account card draws in each of its three states, and what it must never draw.
//!
//! Rendering is asserted on the **rendered** labels rather than on `ActionRow::title()`, which
//! reads back the string it was handed whatever became of the label; see
//! [`crate::ui::mailbox::tests::rendered_labels`]. Called from the crate's single `gtk::init` test.

use adw::prelude::*;

use super::{create_button, failure_row, sign_in_button, signed_in_row, signing_in_row};
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::{every_row_belongs_to_a_list, glib_records, rendered_labels},
    },
};

fn account(email: &str, name: Option<&str>) -> mailcal_bindings::AllodiaAccount {
    mailcal_bindings::AllodiaAccount {
        email: email.to_owned(),
        name: name.map(str::to_owned),
    }
}

fn shown(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    rendered_labels(widget.as_ref())
}

/// The address identifies the account and the name is a courtesy the service may not hold, so the
/// address is always the title and the name only ever the subtitle.
pub(crate) fn the_card_names_the_account_by_address_and_offers_a_way_out() {
    let (sender, receiver) = relm4::channel();

    let row = signed_in_row(&sender, &account("someone@allodia.test", Some("A Person")));
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
        let row = signed_in_row(&sender, &account("someone@allodia.test", name));
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

    let sign_in = sign_in_button(&sender);
    assert_eq!(
        sign_in.label().as_deref(),
        Some(l10n::settings_allodia_sign_in())
    );
    sign_in.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));

    let pending = signing_in_row(&sender);
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

    let create = create_button(&sender);
    assert_eq!(
        create.label().as_deref(),
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
    let row = signed_in_row(&sender, &account("someone@example.com", None));

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

/// The address comes from the account service and the failure text from the core, so both are
/// strings this client did not write. A bare ampersand renders the row **blank** when it is parsed
/// as markup, and a markup-shaped one is *applied* rather than shown.
pub(crate) fn neither_the_address_nor_the_failure_is_parsed_as_markup() {
    let (sender, _receiver) = relm4::channel();
    let hostile = "ada&grace@allodia.test";

    let (rows, records) = glib_records(|| {
        [
            signed_in_row(&sender, &account(hostile, Some("Bell & Co"))),
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
    ));
    group.add(&failure_row("Signing in didn't work"));
    group.set_header_suffix(Some(&sign_in_button(&sender)));

    every_row_belongs_to_a_list(group.upcast_ref::<gtk::Widget>());
}

fn descendant_button(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::Button {
    crate::ui::setup_widget_tests::descendants::<gtk::Button>(root.as_ref())
        .into_iter()
        .find(|button| button.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no button labelled {label:?}"))
}
