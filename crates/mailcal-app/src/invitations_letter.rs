//! The iTIP `REPLY` this core emails itself, whatever the answer was given from.
//!
//! The invitation card names a message and the calendar names an event; both come down to one
//! letter, so the reply, and the question asked when a server could not deliver one, cannot
//! differ by where the user pressed the button.

use engine_api::{
    AccountId, Draft, DraftCalendar, EmailAddress, MessageIdHeader, Provider, ScheduleMethod,
    UtcDateTime,
};

use crate::{
    App,
    helpers::{generated_message_id, now_utc},
    invitations_rsvp::InvitationResponse,
    reference::{EventRef, MessageRef},
};

/// What an answer was given from, so a later "email it instead" can find it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplySource {
    /// The invitation card, over the message the invitation arrived in.
    Message(MessageRef),
    /// The event in the calendar.
    Event(EventRef),
}

/// Everything one reply says.
#[derive(Debug, Clone)]
pub(crate) struct ReplyLetter {
    /// The account answering, whose mail transport sends the reply.
    pub(crate) account: AccountId,
    /// Where the answer was given.
    pub(crate) source: ReplySource,
    /// The meeting's title, for the sentence a delivery question names it in.
    pub(crate) summary: String,
    /// The meeting's `UID`, which is all the organiser's scheduler matches on.
    pub(crate) uid: String,
    /// The `SEQUENCE` being answered.
    pub(crate) sequence: u32,
    /// The organiser's calendar address, as the meeting wrote it.
    pub(crate) organizer: String,
    /// The address this account answers as: the one the meeting matched.
    pub(crate) attendee: String,
    /// The attendee's display name, if the meeting carried one.
    pub(crate) attendee_name: Option<String>,
    /// The answer.
    pub(crate) response: InvitationResponse,
    /// The note for the organiser, if any.
    pub(crate) comment: Option<String>,
    /// The meeting's start.
    pub(crate) starts_at: UtcDateTime,
    /// The occurrence answered, when the invitation named one.
    pub(crate) recurrence_id: Option<UtcDateTime>,
    /// The subject line.
    pub(crate) subject: String,
    /// The message the reply threads onto, when the answer came from one.
    pub(crate) thread: Option<MessageIdHeader>,
}

impl<P: Provider> App<P> {
    /// Builds `letter` into an iTIP `REPLY` and sends it from the account.
    ///
    /// # Errors
    ///
    /// Returns why the reply was not sent; the caller must not report the answer as delivered.
    pub(crate) async fn send_reply_letter(&self, letter: &ReplyLetter) -> Result<(), String> {
        let document = crate::itip::reply(&crate::itip::Reply {
            uid: &letter.uid,
            sequence: letter.sequence,
            organizer: &letter.organizer,
            attendee: &letter.attendee,
            attendee_name: letter.attendee_name.as_deref(),
            status: letter.response.partstat(),
            starts_at: letter.starts_at,
            recurrence_id: letter.recurrence_id,
            stamp: now_utc().map_err(|err| format!("the reply cannot be timestamped: {err}"))?,
            comment: letter.comment.as_deref(),
        });
        let draft = reply_draft(letter, document)
            .ok_or_else(|| "the reply could not be addressed".to_owned())?;
        if self.send_draft_result(&letter.account, &draft, None).await {
            log::info!("respond_to_invitation: the iTIP reply was submitted");
            Ok(())
        } else {
            Err(
                "your answer could not be emailed to the organizer. Check the connection and \
                 try again."
                    .to_owned(),
            )
        }
    }
}

/// Assembles the message the reply travels in.
///
/// The `From` is the **matched attendee**, not the account identity: RFC 6047 §3 has the reply
/// come from the party answering, and an organiser's scheduler that finds no `ATTENDEE` for the
/// sender is entitled to ignore it. On an aliased account those are different addresses, which
/// is the whole reason the alias is resolved before we get here.
///
/// The text body repeats the subject rather than being empty. RFC 6047 §2.4 asks a scheduling
/// message to carry a human-readable alternative for the recipients whose client does not
/// understand `text/calendar`, and one line naming the answer and the meeting is that.
fn reply_draft(letter: &ReplyLetter, document: String) -> Option<Draft> {
    let message_id = MessageIdHeader::new(generated_message_id()).ok()?;
    let organizer = letter
        .organizer
        .strip_prefix("mailto:")
        .or_else(|| letter.organizer.strip_prefix("MAILTO:"))
        .unwrap_or(&letter.organizer);
    let body = match letter
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty())
    {
        Some(note) => format!("{}\r\n\r\n{note}", letter.subject),
        None => letter.subject.clone(),
    };
    let draft = Draft::new(
        message_id,
        EmailAddress::new(letter.attendee.as_str()),
        vec![EmailAddress::new(organizer)],
        letter.subject.clone(),
        body,
    )
    .with_calendar(DraftCalendar::new(ScheduleMethod::Reply, document));
    // Thread the reply onto the invitation, so an organiser reading their mailbox sees the
    // answer beside what they sent rather than as a stray message.
    Some(match &letter.thread {
        Some(parent) => draft.in_reply_to(parent.clone(), vec![parent.clone()]),
        None => draft,
    })
}
