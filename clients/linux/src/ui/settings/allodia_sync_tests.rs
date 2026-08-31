//! What a failed pass is allowed to put on screen, and the one thing it must never put there.
//!
//! The failure's own text reaches none of these functions, so the defect that started this,
//! `invalid_scope: unable to issue scope mailcal:accounts:read` in front of somebody: is a shape
//! the signatures forbid rather than one a test has to catch. What the tests pin is the half that
//! is still a choice: which sentence each state gets, and that each one carries its remedy.
//!
//! Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::AllodiaGrantHealth;

use super::add_failure;
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
    },
};

/// Renders one health into a group of its own and returns the group and the channel behind it.
fn drawn(health: AllodiaGrantHealth) -> (adw::PreferencesGroup, relm4::Receiver<AppInput>) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let section = adw::PreferencesGroup::new();
    add_failure(&section, &sender, health);
    (section, receiver)
}

fn shown(section: &adw::PreferencesGroup) -> Vec<String> {
    rendered_labels(section.upcast_ref::<gtk::Widget>())
}

fn press(section: &adw::PreferencesGroup, label: &str) {
    crate::ui::setup_widget_tests::descendants::<gtk::Button>(section.upcast_ref::<gtk::Widget>())
        .into_iter()
        .find(|button| button.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no {label:?} button: {:?}", shown(section)))
        .emit_clicked();
}

/// A grant that predates a permission is an **offer**: they are signed in, and one feature is
/// asleep. The remedy is the ordinary sign-in, which asks for the full current scope set.
pub(crate) fn a_grant_that_predates_the_feature_offers_the_one_thing_that_fixes_it() {
    let (section, receiver) = drawn(AllodiaGrantHealth::NeedsReauth);
    let labels = shown(&section);

    for expected in [
        l10n::settings_allodia_reauth(),
        l10n::settings_allodia_reauth_hint(),
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "the offer says what is asleep and what fixes it: {labels:?}"
        );
    }

    press(&section, l10n::settings_allodia_reauth_action());

    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));
}

/// A grant that is gone is a **statement** about the account; revoked here, or removed on another
/// device; and carries the same one way back.
pub(crate) fn a_revoked_grant_says_they_are_signed_out() {
    let (section, receiver) = drawn(AllodiaGrantHealth::SignedOut);
    let labels = shown(&section);

    for expected in [
        l10n::settings_allodia_signed_out_elsewhere(),
        l10n::settings_allodia_signed_out_elsewhere_hint(),
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "a revoked grant says so, and says the mail is untouched: {labels:?}"
        );
    }

    press(&section, l10n::settings_allodia_sign_in());

    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));
}

/// A failure that says nothing about the sign-in gets one plain sentence and no button.
///
/// `Ok` means the grant is fine and the pass failed for its own reasons: a dead network, a bad
/// afternoon at the service. Offering "sign in again" there sends somebody through a browser to
/// fix something that is not broken, and quoting the failure is how a generated OAuth field name
/// became product copy.
pub(crate) fn a_failure_that_says_nothing_about_the_grant_offers_no_remedy() {
    let (section, _receiver) = drawn(AllodiaGrantHealth::Ok);
    let labels = shown(&section);

    assert!(
        labels
            .iter()
            .any(|label| label == l10n::settings_allodia_sync_unavailable()),
        "one plain sentence: {labels:?}"
    );
    for absent in [
        l10n::settings_allodia_reauth(),
        l10n::settings_allodia_signed_out_elsewhere(),
    ] {
        assert!(
            !labels.iter().any(|label| label == absent),
            "nothing is broken, so nothing is offered: {labels:?}"
        );
    }
    assert!(
        crate::ui::setup_widget_tests::descendants::<gtk::Button>(
            section.upcast_ref::<gtk::Widget>()
        )
        .is_empty(),
        "and no button to press: {labels:?}"
    );
}

/// Every state's row belongs in a list, or the keyboard walks past the remedy.
pub(crate) fn every_health_row_is_reachable_from_the_keyboard() {
    for health in [
        AllodiaGrantHealth::NeedsReauth,
        AllodiaGrantHealth::SignedOut,
        AllodiaGrantHealth::Ok,
    ] {
        let (section, _receiver) = drawn(health);
        every_row_belongs_to_a_list(section.upcast_ref::<gtk::Widget>());
    }
}
