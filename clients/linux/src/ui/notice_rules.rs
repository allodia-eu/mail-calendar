//! Which of the core's notices this client draws where (`docs/notices.md`): the one in the banner
//! on the screen on show, how many more stand behind the Issues button, the mark each account's
//! row carries, which notices are new, and which flow a notice's action runs. The core has already
//! ranked them, so every rule here keeps its order and decides nothing about rank. GTK-free, so
//! each rule is a plain unit test.

use std::collections::{HashMap, HashSet};

use mailcal_bindings::{
    Intent, Notice, NoticeAction, NoticeIntent, NoticeKind, NoticeScreens, NoticeSeverity,
    NoticeText,
};

use super::{PrimaryView, icons, invitation::reply_subject};
use crate::l10n;

/// A notice's words, through the lookup generated from the catalog (rule 11).
pub(super) fn words(text: &NoticeText) -> String {
    l10n::notice(&text.key, &text.args)
}

/// The screens a notice can name, for the destination on show.
pub(super) const fn screen_of(primary: PrimaryView) -> NoticeScreens {
    match primary {
        PrimaryView::Mail => NoticeScreens::Mail,
        PrimaryView::Calendar => NoticeScreens::Calendar,
        PrimaryView::Contacts => NoticeScreens::Contacts,
    }
}

/// Whether `notice` is drawn on `screen`.
pub(super) fn belongs(notice: &Notice, screen: NoticeScreens) -> bool {
    notice.screens == NoticeScreens::Everywhere || notice.screens == screen
}

/// The notice the banner on `screen` holds: the first, in the core's order, that belongs there
/// (rule 2). None when nothing does.
pub(super) fn prominent(notices: &[Notice], screen: NoticeScreens) -> Option<&Notice> {
    notices.iter().find(|notice| belongs(notice, screen))
}

/// How many notices besides the banner's are in force, which the Issues button counts. None while
/// the screen has no banner: an account's notices are on its row then.
pub(super) fn more(notices: &[Notice], screen: NoticeScreens) -> usize {
    if prominent(notices, screen).is_some() {
        notices.len() - 1
    } else {
        0
    }
}

/// What is said beneath a notice's sentence: that its remedy did not work, which stands until the
/// notice goes (rule 10), or else its detail.
pub(super) fn beneath(notice: &Notice) -> Option<String> {
    if notice.remedy_failed {
        Some(l10n::notices_remedy_failed().to_owned())
    } else {
        notice.detail.as_ref().map(words)
    }
}

/// The notices about `account`, in the core's order: the top of its page in Settings (rule 6).
pub(super) fn about<'a>(notices: &'a [Notice], account: &str) -> Vec<&'a Notice> {
    notices
        .iter()
        .filter(|notice| notice.account.as_deref() == Some(account))
        .collect()
}

/// The mark an account's row carries while a notice is about it (rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountMark {
    /// Its first notice's, which the core ranked most severe: the glyph's.
    pub(crate) severity: NoticeSeverity,
    /// One short line beneath the address.
    pub(crate) label: String,
    /// Every one of the account's sentences, in order, for a screen reader.
    pub(crate) detail: String,
}

/// The mark each account with a notice carries in the folder pane: the glyph of its first notice
/// and that notice's mark, or how many there are when there are several.
pub(super) fn marks(notices: &[Notice]) -> HashMap<String, AccountMark> {
    let mut grouped: Vec<(&str, Vec<&Notice>)> = Vec::new();
    for notice in notices {
        let Some(account) = notice.account.as_deref() else {
            continue;
        };
        match grouped.iter_mut().find(|(id, _)| *id == account) {
            Some((_, group)) => group.push(notice),
            None => grouped.push((account, vec![notice])),
        }
    }
    grouped
        .into_iter()
        .map(|(account, group)| {
            let first = group[0];
            let label = if group.len() > 1 {
                l10n::notices_account_count(i64::try_from(group.len()).unwrap_or(i64::MAX))
            } else {
                words(first.mark.as_ref().unwrap_or(&first.title))
            };
            let detail = group
                .iter()
                .map(|notice| words(&notice.title))
                .collect::<Vec<_>>()
                .join("\n");
            let mark = AccountMark {
                severity: first.severity,
                label,
                detail,
            };
            (account.to_owned(), mark)
        })
        .collect()
}

/// The glyph a severity is drawn with: a different shape each, so colour is never the only signal
/// (rule 4).
pub(super) const fn glyph(severity: NoticeSeverity) -> &'static str {
    match severity {
        NoticeSeverity::Blocked => icons::NOTICE_BLOCKED,
        NoticeSeverity::Degraded => icons::NOTICE_DEGRADED,
        NoticeSeverity::Informational => icons::NOTICE_INFORMATIONAL,
    }
}

/// Whether a notice has something to run, which only an action label can say: the core sends
/// both or neither.
pub(super) fn offers(notice: &Notice) -> bool {
    notice.action.is_some() && notice.action_label.is_some()
}

/// Whether a notice is a question (rule 8): beside its action it carries the other answer, drawn as
/// the lesser button. The core sends the second answer and its label together.
pub(super) fn asks(notice: &Notice) -> bool {
    offers(notice) && notice.second_action.is_some() && notice.second_action_label.is_some()
}

/// What the core runs for `action`, one of `notice`'s answers, or `None` for a flow this client
/// runs itself. A Retry is the core's: it refreshes the notice's screen, which dials the account
/// again, and marks the notice when that dial ends with the notice still standing, which only it
/// can tell (rules 7 and 10). So the client never reports a Retry as failed.
pub(super) fn core_remedy(notice: &Notice, action: NoticeAction) -> Option<NoticeIntent> {
    (action == NoticeAction::Retry).then(|| NoticeIntent::Retry {
        id: notice.id.clone(),
    })
}

/// The intent a question's answer sends (rule 8), or `None` for an action that answers no
/// question. A reply emailed in the person's place carries the subject composed exactly as the
/// invitation's own answer composes it; it is never remembered, because the account's page in
/// Settings is where that is chosen (`docs/invitations.md`).
pub(super) fn answer(notice: &Notice, action: NoticeAction) -> Option<Intent> {
    let intent = match (action, &notice.kind) {
        (NoticeAction::FileSentCopy, _) => Intent::RetryUnfiledCopy,
        (NoticeAction::LeaveSentCopy, _) => Intent::DismissUnfiledCopy,
        (
            NoticeAction::SendReply,
            NoticeKind::ReplyNotDelivered {
                summary, response, ..
            },
        ) => Intent::AnswerReplyPrompt {
            send: true,
            remember: false,
            reply_subject: Some(reply_subject(*response, summary)),
        },
        (NoticeAction::DontSendReply, _) => Intent::AnswerReplyPrompt {
            send: false,
            remember: false,
            reply_subject: None,
        },
        (NoticeAction::UseDeviceTimeZone, _) => Intent::AcceptTimeZoneChange,
        (NoticeAction::KeepTimeZone, _) => Intent::DismissTimeZoneChange,
        _ => return None,
    };
    Some(intent)
}

/// Which notices are new since the last snapshot, so each is announced to assistive technology
/// once, when its id first appears (rule 12), and never again on a redraw.
#[derive(Debug, Default)]
pub(super) struct Announcements {
    in_force: HashSet<String>,
}

impl Announcements {
    /// The notices in `notices` that were not in force last time. A notice that goes and comes
    /// back is new again.
    pub(super) fn arrived<'a>(&mut self, notices: &'a [Notice]) -> Vec<&'a Notice> {
        let arrived = notices
            .iter()
            .filter(|notice| !self.in_force.contains(&notice.id))
            .collect();
        self.in_force = notices.iter().map(|notice| notice.id.clone()).collect();
        arrived
    }
}

#[cfg(test)]
#[path = "notice_rules_tests.rs"]
mod tests;
