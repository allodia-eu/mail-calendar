//! Answering a meeting invitation from a reading view.
//!
//! The intent names the **message**, never the event: the answer must go out as the address the
//! invitation matched, which on an aliased account is not the account's primary identity, and only
//! the core knows the address set (`docs/invitations.md` §4). So the message the **named reader**
//! has open is the whole of what this host contributes, beside the localised reply subject: a card
//! drawn in a window answers for that window's message, never for whatever the pane behind it
//! shows (`docs/reading-window.md`).
//!
//! Nothing is applied optimistically. The write is awaited behind the existing
//! `CalendarWriteStatus` spinner and both surfaces are rebuilt from what the server holds: hiding
//! a declined meeting immediately would buy a few hundred milliseconds and cost a rollback path
//! exercised only when something has already gone wrong.

use mailcal_bindings::Intent;

use super::{AppModel, invitation::InvitationAnswer, reader::ReadingSource};

impl AppModel {
    pub(super) fn respond_to_invitation(&self, source: &ReadingSource, answer: InvitationAnswer) {
        let Some(opened) = self.reader_message(source) else {
            return;
        };
        // The closed-enum answer and two flags only; never the note, the subject, the meeting or
        // the organiser, all of which are message content (`docs/invitations.md` → Logging and
        // privacy).
        log::info!(
            "invitation: answering response={:?} note={} notify={}",
            answer.response,
            answer.comment.is_some(),
            answer.notify_organizer
        );
        self.dispatch(Intent::RespondToInvitation {
            account: opened.account.clone(),
            key: opened.key.clone(),
            response: answer.response,
            // Blank is the same as none, and the core treats it so: but sending `Some("")` on a
            // transport that carries no note would still be a note it must refuse.
            comment: answer.comment.filter(|note| !note.trim().is_empty()),
            notify_organizer: answer.notify_organizer,
            reply_subject: Some(answer.reply_subject),
        });
    }
}
