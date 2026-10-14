//! An invitee's answer in an event's detail: what they answered, and Accept, Maybe and Decline to
//! change it.
//!
//! Offered only where the core sets `EventDetail::my_answer`: the account is invited, does not
//! organise the meeting, and has a way to tell the organiser. The answer is for the whole meeting,
//! and goes the way the invitation card's does (`docs/calendar.md`, "Answering from the calendar").
//! The words are the card's, so the two places a meeting is answered read the same.
//!
//! Accept and Maybe go at once and the detail stays open; Decline asks first, because a declined
//! meeting leaves the calendar and the detail with it.

use std::rc::Rc;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{InvitationResponse, ResponseStatus};

use crate::{
    l10n,
    ui::invitation::{reply_subject, response},
};

/// Called with the answer chosen and the localised subject for a reply the core emails itself.
pub(super) type OnAnswer = Rc<dyn Fn(InvitationResponse, String)>;

/// The answer under its own heading, which says what was answered so far. While `saving`, no
/// button can be pressed: an answer is on its way, and a second would race it.
pub(super) fn answer_group(
    current: ResponseStatus,
    title: &str,
    saving: bool,
    on_answer: &OnAnswer,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(response(current))
        .build();
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    for (answer, label, spoken) in [
        (
            InvitationResponse::Accept,
            l10n::invitation_accept(),
            l10n::a11y_invitation_accept(),
        ),
        (
            InvitationResponse::Tentative,
            l10n::invitation_tentative(),
            l10n::a11y_invitation_tentative(),
        ),
        (
            InvitationResponse::Decline,
            l10n::invitation_decline(),
            l10n::a11y_invitation_decline(),
        ),
    ] {
        let button = gtk::Button::with_label(label);
        // As on the card: the name is the verb, the description what it acts on.
        button.update_property(&[AccessibleProperty::Description(spoken)]);
        if is_current(current, answer) {
            // The answer already given: sending it again changes nothing, so the button says so by
            // being the one that cannot be pressed.
            button.add_css_class("suggested-action");
            button.set_sensitive(false);
        }
        if saving {
            button.set_sensitive(false);
        }
        let on_answer = Rc::clone(on_answer);
        let subject = reply_subject(answer, title);
        button.connect_clicked(move |button| {
            if matches!(answer, InvitationResponse::Decline) {
                let (on_answer, subject) = (Rc::clone(&on_answer), subject.clone());
                confirm_decline(button, move || on_answer(answer, subject.clone()));
            } else {
                on_answer(answer, subject.clone());
            }
        });
        buttons.append(&button);
    }
    group.add(&buttons);
    group
}

/// Asks before a decline, which takes the meeting out of the calendar; `on_confirm` sends it.
pub(super) fn confirm_decline(
    parent: &impl IsA<gtk::Widget>,
    on_confirm: impl Fn() + 'static,
) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(
        Some(l10n::event_decline_confirm_title()),
        Some(l10n::event_decline_confirm_body()),
    );
    dialog.add_response(CANCEL, l10n::action_cancel());
    dialog.add_response(DECLINE, l10n::invitation_decline());
    dialog.set_response_appearance(DECLINE, adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some(CANCEL));
    dialog.set_close_response(CANCEL);
    dialog.connect_response(None, move |_, response| {
        if response == DECLINE {
            on_confirm();
        }
    });
    dialog.present(Some(parent));
    dialog
}

pub(super) const DECLINE: &str = "decline";
const CANCEL: &str = "cancel";

/// Whether `answer` is the one already given.
pub(super) const fn is_current(current: ResponseStatus, answer: InvitationResponse) -> bool {
    matches!(
        (current, answer),
        (ResponseStatus::Accepted, InvitationResponse::Accept)
            | (ResponseStatus::Tentative, InvitationResponse::Tentative)
            | (ResponseStatus::Declined, InvitationResponse::Decline)
    )
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{InvitationResponse, ResponseStatus};

    use super::is_current;

    #[test]
    fn only_the_answer_already_given_is_current() {
        assert!(is_current(
            ResponseStatus::Accepted,
            InvitationResponse::Accept
        ));
        assert!(!is_current(
            ResponseStatus::Accepted,
            InvitationResponse::Decline
        ));
        for answer in [
            InvitationResponse::Accept,
            InvitationResponse::Tentative,
            InvitationResponse::Decline,
        ] {
            assert!(
                !is_current(ResponseStatus::NeedsAction, answer),
                "an unanswered meeting offers all three"
            );
        }
    }
}
