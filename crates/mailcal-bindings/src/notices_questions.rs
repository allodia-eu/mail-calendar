//! The questions the core holds, as notices with two answers (`docs/notices.md` rule 8): a sent
//! message whose copy is not in Sent, an invitation reply the server could not pass on, and a
//! changed device time zone. A question is never closed, only answered.

use super::{NoticeFacts, NoticeId, Raised};
use crate::{InvitationResponse, NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity};

/// What the core is asking, read once per pull.
#[derive(Debug, Default)]
pub(crate) struct Questions {
    /// A sent message whose copy could not be saved in Sent.
    pub(crate) unfiled: Option<UnfiledQuestion>,
    /// An invitation answer the calendar server could not pass on.
    pub(crate) reply: Option<ReplyQuestion>,
    /// The device's zone, when it differs from the one the calendar is shown in.
    pub(crate) time_zone: Option<String>,
}

/// The sent message whose copy is missing.
#[derive(Debug, Clone)]
pub(crate) struct UnfiledQuestion {
    /// The account whose Sent folder lacks it.
    pub(crate) account: String,
    pub(crate) subject: String,
    /// A save to Sent is running.
    pub(crate) retrying: bool,
}

/// The invitation reply that was not passed on.
#[derive(Debug, Clone)]
pub(crate) struct ReplyQuestion {
    /// The account whose calendar server failed.
    pub(crate) account: String,
    pub(crate) summary: String,
    pub(crate) organizer: String,
    pub(crate) response: InvitationResponse,
}

/// Raises a notice for each question the core holds. Each question's id names what it asks
/// about, so a second question replacing the first is a new notice, announced as one.
pub(super) fn raise(facts: &NoticeFacts<'_>, raised: &mut Vec<Raised>) {
    use NoticeScreens::{Everywhere, Mail};
    use NoticeSeverity::{Degraded, Informational};

    let questions = facts.questions;
    if let Some(unfiled) = &questions.unfiled {
        let mut notice = Raised::new(
            facts.address,
            "sent_not_filed",
            Some(&unfiled.account),
            NoticeKind::SentNotFiled {
                subject: unfiled.subject.clone(),
            },
            Degraded,
            Mail,
            Some(NoticeAction::FileSentCopy),
        )
        .or(NoticeAction::LeaveSentCopy)
        .running(unfiled.retrying);
        let scope = format!("{}:{}", unfiled.account, unfiled.subject);
        notice.id = NoticeId::new("sent_not_filed", Some(&scope));
        raised.push(notice);
    }
    if let Some(reply) = &questions.reply {
        let mut notice = Raised::new(
            facts.address,
            "reply_not_delivered",
            Some(&reply.account),
            NoticeKind::ReplyNotDelivered {
                summary: reply.summary.clone(),
                organizer: reply.organizer.clone(),
                response: reply.response,
            },
            Degraded,
            // An answer is given from the reading pane as often as from the calendar.
            Everywhere,
            Some(NoticeAction::SendReply),
        )
        .or(NoticeAction::DontSendReply);
        let scope = format!("{}:{}:{}", reply.account, reply.summary, reply.organizer);
        notice.id = NoticeId::new("reply_not_delivered", Some(&scope));
        raised.push(notice);
    }
    if let Some(zone) = &questions.time_zone {
        let mut notice = Raised::new(
            facts.address,
            "time_zone_changed",
            None,
            NoticeKind::TimeZoneChanged { zone: zone.clone() },
            Informational,
            // The zone is the one message times are shown in too.
            Everywhere,
            Some(NoticeAction::UseDeviceTimeZone),
        )
        .or(NoticeAction::KeepTimeZone);
        notice.id = NoticeId::new("time_zone_changed", Some(zone));
        raised.push(notice);
    }
}

#[cfg(test)]
#[path = "notices_questions_tests.rs"]
mod tests;
