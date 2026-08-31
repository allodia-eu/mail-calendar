//! Accept / Maybe / Decline, and the two controls that ride beside them on the transports that
//! have them.
//!
//! Its own file because it is the only part of the card that *writes*, and because everything in it
//! is conditional on what the account can actually do; the card itself stays a straight render of
//! what the core computed.
//!
//! # Three gates, none of them a disabled button
//!
//! - `can_respond`: the account's calendar cannot RSVP at all. The buttons are then *absent* and a
//!   sentence says why. A greyed-out Accept invites the user to try, wonder, and try again; "this
//!   account can't send a response" ends it.
//! - `can_comment`: the transport has nowhere to put a note (CalDAV, JMAP). The field is absent,
//!   because the core **refuses** a note it cannot carry rather than dropping it: an offered field
//!   would not merely lose the text, it would lose the whole answer.
//! - `can_choose_notify`: the server sends the reply the moment the status changes and no client
//!   can stop it. The toggle is absent for the same reason: one that emails the organiser anyway is
//!   worse than none.
//!
//! On a CalDAV account this is three buttons and nothing else; the truth of the transport, not a
//! missing feature. A JMAP account keeps the notify tick: the engine's session advertises
//! `suppress_notification`, which the core maps to `can_choose_notify` (`docs/invitations.md`).

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{InvitationCard, InvitationResponse};

use super::{InvitationAnswer, InvitationCardView, caption, reply_subject};
use crate::{l10n, ui::AppInput};

impl InvitationCardView {
    pub(super) fn append_respond_row(
        &self,
        card: &InvitationCard,
        summary: &str,
        sender: &relm4::Sender<AppInput>,
    ) {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
        row.set_margin_top(4);
        if !card.can_respond {
            // Absent with an explanation, never present and disabled: a button that appears to work
            // but tells nobody is worse than no button.
            row.append(&caption(l10n::invitation_cannot_respond()));
            self.body.append(&row);
            return;
        }

        let comment = card.can_comment.then(|| {
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some(l10n::invitation_message_to_organizer()));
            entry.update_property(&[AccessibleProperty::Label(
                l10n::invitation_message_to_organizer(),
            )]);
            row.append(&entry);
            self.answers.borrow_mut().push(entry.clone().upcast());
            entry
        });
        let notify = card.can_choose_notify.then(|| {
            // Ticked by default, mirroring RFC 5546: an invitation asks for a reply, so answering
            // sends one. The user has to say otherwise.
            let check = gtk::CheckButton::with_label(l10n::invitation_notify_organizer());
            check.set_active(true);
            row.append(&check);
            self.answers.borrow_mut().push(check.clone().upcast());
            check
        });

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        for (response, label, spoken, suggested) in [
            (
                InvitationResponse::Accept,
                l10n::invitation_accept(),
                l10n::a11y_invitation_accept(),
                true,
            ),
            (
                InvitationResponse::Tentative,
                l10n::invitation_tentative(),
                l10n::a11y_invitation_tentative(),
                false,
            ),
            (
                InvitationResponse::Decline,
                l10n::invitation_decline(),
                l10n::a11y_invitation_decline(),
                false,
            ),
        ] {
            let button = gtk::Button::with_label(label);
            if suggested {
                button.add_css_class("suggested-action");
            }
            // Three bare verbs read out of context tell a screen-reader user nothing about which
            // invitation they belong to, so each one carries what it acts on; as a
            // `Description`, **not** a `Label`. A `GtkButton` with a label is `labelled-by` its own
            // `GtkLabel`, and by the ARIA rules GTK follows a relation beats an explicit label: a
            // `Label` here is silently ignored and the button goes on announcing "Accept" alone.
            // A description has no competing relation and is announced after the name
            // (`AGENTS.md`, the same trap `AdwActionRow` sets).
            button.update_property(&[AccessibleProperty::Description(spoken)]);
            let input = sender.clone();
            let comment = comment.clone();
            let notify = notify.clone();
            let subject = reply_subject(response, summary);
            button.connect_clicked(move |_| {
                input.emit(AppInput::RespondToInvitation(Box::new(InvitationAnswer {
                    response,
                    // `comment` exists only where the transport carries one, so this is `None`
                    // exactly when `can_comment` is false: sending a note a transport cannot carry
                    // fails the whole answer rather than quietly losing the text.
                    comment: comment.as_ref().map(|entry| entry.text().to_string()),
                    notify_organizer: notify.as_ref().is_none_or(CheckButtonExt::is_active),
                    reply_subject: subject.clone(),
                })));
            });
            self.answers.borrow_mut().push(button.clone().upcast());
            buttons.append(&button);
        }
        row.append(&buttons);
        self.body.append(&row);
    }
}
