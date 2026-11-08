//! Which of the core's notices this client draws where (`docs/notices.md`): the one in the banner
//! on the screen on show, how many more stand behind the Issues button, the mark each account's
//! row carries, which notices are new, and which flow a notice's action runs. The core has already
//! ranked them, so every rule here keeps its order and decides nothing about rank. GTK-free, so
//! each rule is a plain unit test.

use std::collections::{HashMap, HashSet};

use mailcal_bindings::{
    AccountProvider, Notice, NoticeAction, NoticeIntent, NoticeKind, NoticeScreens, NoticeSeverity,
    NoticeText,
};

use super::{PrimaryView, icons};
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

/// Whether the server refuses `account`'s stored sign-in, which its page in Settings offers a new
/// password for. A grant too narrow for one use is a different notice: the sign-in still works.
pub(super) fn sign_in_expired(notices: &[Notice], account: &str) -> bool {
    notices.iter().any(|notice| {
        notice.kind == NoticeKind::SignInExpired && notice.account.as_deref() == Some(account)
    })
}

/// Whether the account service refused the Allodia sign-in, which the Allodia card draws.
pub(super) fn allodia_sign_in_ended(notices: &[Notice]) -> bool {
    notices
        .iter()
        .any(|notice| matches!(notice.kind, NoticeKind::AllodiaSignInEnded { .. }))
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

/// What the core runs for a notice's action, or `None` for a flow this client runs itself. A
/// Retry is the core's: it refreshes the notice's screen, which dials the account again, and marks
/// the notice when that dial ends with the notice still standing, which only it can tell (rules 7
/// and 10). So the client never reports a Retry as failed.
pub(super) fn core_remedy(notice: &Notice) -> Option<NoticeIntent> {
    (notice.action == Some(NoticeAction::Retry)).then(|| NoticeIntent::Retry {
        id: notice.id.clone(),
    })
}

/// The flow a notice's Sign in again runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignInFlow {
    /// The provider's consent, which keeps the account and asks again for everything it is used
    /// for: a Microsoft or Google sign-in, and a grant that withholds mail or the calendar.
    Consent,
    /// A JMAP account's own stored grant, authorised again.
    Jmap,
    /// The account's own browser sign-in: an IMAP account that signs in at its provider, and one
    /// a Nextcloud sign-in made.
    AccountSignIn,
    /// No browser flow to run: the password is entered again on the account's page.
    Settings,
}

/// Which flow signs `kind`'s account in again. A grant too narrow for mail or the calendar is
/// widened at its provider; a dead sign-in runs whatever flow the account signs in by.
pub(super) fn sign_in_flow(
    kind: &NoticeKind,
    provider: Option<&AccountProvider>,
    nextcloud: bool,
) -> SignInFlow {
    if matches!(kind, NoticeKind::MailReauth | NoticeKind::CalendarReauth) {
        return SignInFlow::Consent;
    }
    if nextcloud {
        return SignInFlow::AccountSignIn;
    }
    match provider {
        Some(AccountProvider::Microsoft | AccountProvider::Google) => SignInFlow::Consent,
        Some(AccountProvider::JmapOauth) => SignInFlow::Jmap,
        Some(AccountProvider::ImapOauth) => SignInFlow::AccountSignIn,
        Some(AccountProvider::Password | AccountProvider::Jmap) | None => SignInFlow::Settings,
    }
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
