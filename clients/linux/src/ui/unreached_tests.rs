//! Which set gets which words, whether a switch asks, and what each answer tells the core. The
//! dialog's own check is called from the crate's single GTK test.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, UnreachedUses};

use super::{Answer, CANCEL, KEEP, STOP, asks, question, separate_title, switch_title};
use crate::l10n;

const CALENDAR: AccountCapability = AccountCapability::Calendar;
const CONTACTS: AccountCapability = AccountCapability::Contacts;

#[test]
fn each_set_of_uses_gets_the_title_that_names_it() {
    assert_eq!(
        switch_title(&[CALENDAR]),
        l10n::signin_unreached_title_calendar()
    );
    assert_eq!(
        switch_title(&[CONTACTS]),
        l10n::signin_unreached_title_contacts()
    );
    assert_eq!(
        switch_title(&[CALENDAR, CONTACTS]),
        l10n::signin_unreached_title_both()
    );
    assert_eq!(
        separate_title(&[CALENDAR]),
        l10n::separate_server_title_calendar()
    );
    assert_eq!(
        separate_title(&[CONTACTS]),
        l10n::separate_server_title_contacts()
    );
    assert_eq!(
        separate_title(&[CALENDAR, CONTACTS]),
        l10n::separate_server_title_both()
    );
}

#[test]
fn a_switch_asks_only_when_it_leaves_a_calendar_or_contacts_behind() {
    assert!(!asks(&[]));
    assert!(!asks(&[AccountCapability::Mail]));
    assert!(asks(&[CALENDAR]));
    assert!(asks(&[CONTACTS]));
}

#[test]
fn each_answer_tells_the_core_what_it_chose_and_cancel_tells_it_nothing() {
    assert_eq!(
        Answer::from_response(KEEP).choice(),
        Some(UnreachedUses::KeepOnPassword)
    );
    assert_eq!(
        Answer::from_response(STOP).choice(),
        Some(UnreachedUses::StopUsing)
    );
    assert_eq!(Answer::from_response(CANCEL).choice(), None);
    // Anything the dialog was not built with abandons the switch rather than guessing.
    assert_eq!(Answer::from_response("").choice(), None);
}

/// Keeping is the default, stopping reads as destructive, and Escape cancels.
pub(crate) fn the_question_defaults_to_keeping_and_escape_cancels() {
    let dialog = question(&[CALENDAR]);
    assert_eq!(
        dialog.heading().as_deref(),
        Some(l10n::signin_unreached_title_calendar())
    );
    assert_eq!(dialog.body().as_str(), l10n::signin_unreached_body());
    assert_eq!(dialog.default_response().as_deref(), Some(KEEP));
    assert_eq!(dialog.close_response().as_str(), CANCEL);
    assert_eq!(
        dialog.response_appearance(STOP),
        adw::ResponseAppearance::Destructive
    );
    assert_eq!(
        dialog.response_appearance(KEEP),
        adw::ResponseAppearance::Suggested
    );
    for (response, label) in [
        (KEEP, l10n::signin_unreached_keep()),
        (STOP, l10n::signin_unreached_stop()),
        (CANCEL, l10n::action_cancel()),
    ] {
        assert_eq!(dialog.response_label(response).as_str(), label);
    }
}
