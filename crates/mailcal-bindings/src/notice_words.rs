//! Which catalog message says each notice, and with what (`docs/notices.md` rule 11).
//!
//! The core is locale-free, so it names words rather than writing them; each client's generated
//! `notice` lookup resolves the key in its own locale. A test holds every key here to the catalog.

use std::collections::HashMap;

use crate::{FolderProblem, NoticeAction, NoticeKind, NoticeText, UseProblem};

fn text(key: &str, args: &[(&str, &str)]) -> NoticeText {
    NoticeText {
        key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>(),
    }
}

/// The sentence for `kind`; `address` is the address of the account it is about, if any.
pub(crate) fn title(kind: &NoticeKind, address: &str) -> NoticeText {
    let about = [("address", address)];
    match kind {
        NoticeKind::SignInExpired => text("notice_signin_expired", &about),
        NoticeKind::MailReauth => text("notice_mail_reauth", &about),
        NoticeKind::CalendarReauth => text("notice_calendar_reauth", &about),
        NoticeKind::CalendarOutage { .. } => text("notice_calendar_outage", &about),
        NoticeKind::ContactsOutage { .. } => text("notice_contacts_outage", &about),
        NoticeKind::Unreachable => text("notice_unreachable", &about),
        NoticeKind::Offline => text("notice_offline", &[]),
        NoticeKind::AllodiaSignInEnded { address } => {
            text("notice_allodia_sign_in_ended", &[("address", address)])
        }
        NoticeKind::AccountsSkipped { .. } => text("notice_accounts_skipped", &[]),
        NoticeKind::SentNotFiled { subject } => {
            text("notice_sent_not_filed", &[("subject", subject)])
        }
        NoticeKind::ReplyNotDelivered {
            summary, organizer, ..
        } => text(
            "notice_reply_not_delivered",
            &[("summary", summary), ("organizer", organizer)],
        ),
        NoticeKind::TimeZoneChanged { zone } => text("notice_time_zone_changed", &[("zone", zone)]),
        NoticeKind::FolderChangeRefused {
            folder, problem, ..
        } => {
            let key = match problem {
                FolderProblem::ChangedElsewhere => "notice_folder_changed",
                FolderProblem::Refused => "notice_folder_refused",
            };
            text(key, &[("folder", folder)])
        }
    }
}

/// What a host says under the title: for an outage, why a calendar or an address book did not
/// connect and that what is shown is what was last synced, the reason being the one the
/// account's page gives under that use's switch; for a question, what each answer does.
pub(crate) fn detail(kind: &NoticeKind) -> Option<NoticeText> {
    let (problem, used) = match kind {
        NoticeKind::CalendarOutage { problem } => (problem, "calendar"),
        NoticeKind::ContactsOutage { problem } => (problem, "contacts"),
        NoticeKind::ReplyNotDelivered { .. } => {
            return Some(text("notice_detail_reply_not_delivered", &[]));
        }
        _ => return None,
    };
    let why = match problem {
        UseProblem::Unreachable => "unreachable",
        UseProblem::SignInRefused => "refused",
        UseProblem::NeedsOwnSignIn => "own_sign_in",
        UseProblem::ServerChanged => "server_changed",
        UseProblem::NoneOnServer => "none_on_server",
    };
    Some(text(&format!("notice_detail_{why}_{used}"), &[]))
}

/// The few words an account's row says for `kind`, or `None` for a notice about the whole app.
pub(crate) fn mark(kind: &NoticeKind) -> Option<NoticeText> {
    let key = match kind {
        NoticeKind::SignInExpired => "notice_mark_signin_expired",
        NoticeKind::MailReauth => "notice_mark_mail_reauth",
        NoticeKind::CalendarReauth => "notice_mark_calendar_reauth",
        NoticeKind::CalendarOutage {
            problem: UseProblem::ServerChanged,
        } => "notice_mark_calendar_paused",
        NoticeKind::CalendarOutage { .. } => "notice_mark_calendar_outage",
        NoticeKind::ContactsOutage {
            problem: UseProblem::ServerChanged,
        } => "notice_mark_contacts_paused",
        NoticeKind::ContactsOutage { .. } => "notice_mark_contacts_outage",
        NoticeKind::Unreachable => "notice_mark_unreachable",
        NoticeKind::FolderChangeRefused { .. } => "notice_mark_folder_change_refused",
        NoticeKind::SentNotFiled { .. } => "notice_mark_sent_not_filed",
        NoticeKind::ReplyNotDelivered { .. } => "notice_mark_reply_not_delivered",
        NoticeKind::Offline
        | NoticeKind::AllodiaSignInEnded { .. }
        | NoticeKind::AccountsSkipped { .. }
        | NoticeKind::TimeZoneChanged { .. } => return None,
    };
    Some(text(key, &[]))
}

/// The label of `action`'s button.
pub(crate) fn action_label(action: NoticeAction) -> NoticeText {
    let key = match action {
        NoticeAction::SignInAgain | NoticeAction::SignInToAllodia => "notice_action_sign_in_again",
        NoticeAction::OpenAccountSettings => "notice_action_open_account_settings",
        NoticeAction::Retry => "notice_action_retry",
        NoticeAction::Review => "notice_action_review",
        NoticeAction::FileSentCopy => "notice_action_file_sent_copy",
        NoticeAction::LeaveSentCopy => "notice_action_leave_sent_copy",
        NoticeAction::SendReply => "notice_action_send_reply",
        NoticeAction::DontSendReply => "notice_action_dont_send_reply",
        NoticeAction::UseDeviceTimeZone => "notice_action_use_time_zone",
        NoticeAction::KeepTimeZone => "notice_action_keep_time_zone",
    };
    text(key, &[])
}

#[cfg(test)]
#[path = "notice_words_tests.rs"]
mod tests;
