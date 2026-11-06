//! Which notices are in force, in what order, and what the person has done about them
//! (`docs/notices.md`).
//!
//! [`raise`] maps the facts the core already holds onto notices, one per account and fact.
//! [`NoticeBook`] is the only state: when each notice was first raised, which were dismissed, and
//! which remedies were reported as failed. It forgets a notice the moment its fact goes, so one
//! raised again later is a new notice, newer, not dismissed and not failed.

use std::collections::{HashMap, HashSet};

use crate::{
    AccountProvider, ConnectivitySnapshot, FolderNotice, Notice, NoticeAction, NoticeKind,
    NoticeScreens, NoticeSeverity, NoticesSnapshot, UseOutage, UseProblem,
};

/// What the notices are built from: everything is read, nothing is kept.
pub(crate) struct NoticeFacts<'a> {
    /// The connectivity snapshot as a host reads it, outages and the Allodia refusal included.
    pub(crate) connectivity: &'a ConnectivitySnapshot,
    /// How many stored accounts could not be read at launch.
    pub(crate) skipped_at_launch: usize,
    /// The folder change the server refused, if one stands.
    pub(crate) folder_notice: Option<FolderNotice>,
    /// Which sign-in an account was connected with, for the remedy a dead sign-in needs.
    pub(crate) provider: &'a dyn Fn(&str) -> Option<AccountProvider>,
}

/// A notice's identity: its kind and the account it is about. Never parsed; only compared.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NoticeId(String);

impl NoticeId {
    fn new(kind: &str, account: Option<&str>) -> Self {
        Self(match account {
            Some(account) => format!("{kind}:{account}"),
            None => kind.to_owned(),
        })
    }
}

/// How a dismissible notice is closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Dismissal {
    /// The book remembers it, for as long as its fact holds.
    Remembered,
    /// The folder pane holds this refusal, and dismissing clears it there, only while it is
    /// still the one the pane holds.
    FolderNotice(FolderNotice),
}

/// A notice as the facts raise it, before the book ranks it.
pub(crate) struct Raised {
    id: NoticeId,
    kind: NoticeKind,
    severity: NoticeSeverity,
    screens: NoticeScreens,
    account: Option<String>,
    action: Option<NoticeAction>,
    dismissal: Option<Dismissal>,
}

impl Raised {
    fn new(
        slug: &str,
        account: Option<&str>,
        kind: NoticeKind,
        severity: NoticeSeverity,
        screens: NoticeScreens,
        action: Option<NoticeAction>,
    ) -> Self {
        Self {
            id: NoticeId::new(slug, account),
            kind,
            severity,
            screens,
            account: account.map(str::to_owned),
            action,
            dismissal: None,
        }
    }

    fn dismissed_by(mut self, dismissal: Dismissal) -> Self {
        self.dismissal = Some(dismissal);
        self
    }
}

/// Every notice the facts raise, unranked.
pub(crate) fn raise(facts: &NoticeFacts<'_>) -> Vec<Raised> {
    use NoticeScreens::{Calendar, Contacts, Everywhere, Mail};
    use NoticeSeverity::{Blocked, Degraded, Informational};

    let connectivity = facts.connectivity;
    let mut raised = Vec::new();
    if connectivity.offline {
        raised.push(Raised::new(
            "offline",
            None,
            NoticeKind::Offline,
            Informational,
            Everywhere,
            None,
        ));
    }
    if let Some(address) = &connectivity.allodia_sign_in_ended {
        raised.push(Raised::new(
            "allodia_sign_in_ended",
            None,
            NoticeKind::AllodiaSignInEnded {
                address: address.clone(),
            },
            Degraded,
            Everywhere,
            Some(NoticeAction::SignInToAllodia),
        ));
    }
    if facts.skipped_at_launch > 0 {
        let count = u32::try_from(facts.skipped_at_launch).unwrap_or(u32::MAX);
        raised.push(
            Raised::new(
                "accounts_skipped",
                None,
                NoticeKind::AccountsSkipped { count },
                Degraded,
                Mail,
                None,
            )
            .dismissed_by(Dismissal::Remembered),
        );
    }
    for account in sorted(&connectivity.signin_expired_accounts) {
        let action = sign_in_remedy((facts.provider)(account));
        let kind = NoticeKind::SignInExpired;
        raised.push(Raised::new(
            "signin_expired",
            Some(account),
            kind,
            Blocked,
            Everywhere,
            Some(action),
        ));
    }
    let reauth = Some(NoticeAction::SignInAgain);
    for account in sorted(&connectivity.mail_reauth_accounts) {
        let kind = NoticeKind::MailReauth;
        raised.push(Raised::new(
            "mail_reauth",
            Some(account),
            kind,
            Blocked,
            Mail,
            reauth,
        ));
    }
    for account in sorted(&connectivity.calendar_reauth_accounts) {
        let kind = NoticeKind::CalendarReauth;
        let slug = "calendar_reauth";
        raised.push(Raised::new(
            slug,
            Some(account),
            kind,
            Degraded,
            Calendar,
            reauth,
        ));
    }
    for outage in sorted_outages(&connectivity.calendar_outages) {
        let kind = NoticeKind::CalendarOutage {
            problem: outage.problem,
        };
        let (account, action) = (outage.account_id.as_str(), outage_remedy(outage.problem));
        raised.push(Raised::new(
            "calendar_outage",
            Some(account),
            kind,
            Degraded,
            Calendar,
            action,
        ));
    }
    for outage in sorted_outages(&connectivity.contacts_outages) {
        let kind = NoticeKind::ContactsOutage {
            problem: outage.problem,
        };
        let (account, action) = (outage.account_id.as_str(), outage_remedy(outage.problem));
        raised.push(Raised::new(
            "contacts_outage",
            Some(account),
            kind,
            Degraded,
            Contacts,
            action,
        ));
    }
    for account in sorted(&connectivity.unreachable_accounts) {
        let retry = Some(NoticeAction::Retry);
        let kind = NoticeKind::Unreachable;
        raised.push(Raised::new(
            "unreachable",
            Some(account),
            kind,
            Degraded,
            Mail,
            retry,
        ));
    }
    if let Some(notice) = &facts.folder_notice {
        let kind = NoticeKind::FolderChangeRefused {
            folder: notice.folder.clone(),
            action: notice.action,
            problem: notice.problem,
        };
        let account = Some(notice.account.as_str());
        let mut refused = Raised::new("folder_change_refused", account, kind, Degraded, Mail, None)
            .dismissed_by(Dismissal::FolderNotice(notice.clone()));
        // A later refusal on the same account replaces this one in the pane; its own id keeps a
        // dismiss aimed at this one from closing that one.
        let scope = format!("{}:{:?}:{}", notice.account, notice.action, notice.folder);
        refused.id = NoticeId::new("folder_change_refused", Some(&scope));
        raised.push(refused);
    }
    raised
}

/// A dead sign-in's remedy: the provider's browser sign-in where there is one, the account's
/// page in Settings where a password or token has to be entered again.
fn sign_in_remedy(provider: Option<AccountProvider>) -> NoticeAction {
    match provider {
        Some(
            AccountProvider::Microsoft
            | AccountProvider::Google
            | AccountProvider::JmapOauth
            | AccountProvider::ImapOauth,
        ) => NoticeAction::SignInAgain,
        Some(AccountProvider::Password | AccountProvider::Jmap) | None => {
            NoticeAction::OpenAccountSettings
        }
    }
}

/// A use that did not connect: dialled again by a refresh when it could not be reached, and
/// explained on the account's page otherwise.
fn outage_remedy(problem: UseProblem) -> Option<NoticeAction> {
    Some(match problem {
        UseProblem::Unreachable => NoticeAction::Retry,
        UseProblem::NeedsOwnSignIn
        | UseProblem::SignInRefused
        | UseProblem::ServerChanged
        | UseProblem::NoneOnServer => NoticeAction::OpenAccountSettings,
    })
}

/// The core keeps these as sets, so the order they arrive in is not the same twice; ties in
/// rank are broken by the order a notice was first raised in, which this makes stable.
fn sorted(accounts: &[String]) -> Vec<&str> {
    let mut accounts: Vec<&str> = accounts.iter().map(String::as_str).collect();
    accounts.sort_unstable();
    accounts
}

fn sorted_outages(outages: &[UseOutage]) -> Vec<&UseOutage> {
    let mut outages: Vec<&UseOutage> = outages.iter().collect();
    outages.sort_by(|a, b| a.account_id.cmp(&b.account_id));
    outages
}

/// What the book knows of a notice in force.
struct InForce {
    dismissal: Option<Dismissal>,
    action: Option<NoticeAction>,
}

/// The notices' state across snapshots: age, dismissal and failed remedies.
#[derive(Default)]
pub(crate) struct NoticeBook {
    /// When each notice in force was first raised, as a sequence number: the age rule 3 ranks by.
    first_raised: HashMap<NoticeId, u64>,
    next: u64,
    /// The notices in force at the last snapshot.
    in_force: HashMap<NoticeId, InForce>,
    dismissed: HashSet<NoticeId>,
    /// The remedy each notice was offering when it was reported as failed. A notice whose action
    /// has changed since offers something nobody has tried, so its entry is dropped.
    remedy_failed: HashMap<NoticeId, NoticeAction>,
}

impl NoticeBook {
    /// Ranks what the facts raise, and forgets everything about a notice no longer raised.
    pub(crate) fn snapshot(&mut self, raised: Vec<Raised>) -> NoticesSnapshot {
        let live: HashSet<NoticeId> = raised.iter().map(|notice| notice.id.clone()).collect();
        self.first_raised.retain(|id, _| live.contains(id));
        self.dismissed.retain(|id| live.contains(id));
        self.remedy_failed.retain(|id, _| live.contains(id));
        self.in_force.clear();
        let mut ranked = Vec::with_capacity(raised.len());
        for notice in raised {
            let age = *self
                .first_raised
                .entry(notice.id.clone())
                .or_insert_with(|| {
                    self.next += 1;
                    self.next
                });
            self.in_force.insert(
                notice.id.clone(),
                InForce {
                    dismissal: notice.dismissal.clone(),
                    action: notice.action,
                },
            );
            if self.remedy_failed.get(&notice.id).copied() != notice.action {
                self.remedy_failed.remove(&notice.id);
            }
            if self.dismissed.contains(&notice.id) {
                continue;
            }
            let remedy_failed = self.remedy_failed.contains_key(&notice.id);
            let key = (notice.severity, notice.account.is_some(), age);
            ranked.push((key, notice, remedy_failed));
        }
        ranked.sort_by_key(|(key, ..)| *key);
        let notices = ranked
            .into_iter()
            .map(|(_, notice, remedy_failed)| Notice {
                id: notice.id.0,
                kind: notice.kind,
                severity: notice.severity,
                screens: notice.screens,
                account: notice.account,
                action: notice.action,
                dismissible: notice.dismissal.is_some(),
                remedy_failed,
            })
            .collect();
        NoticesSnapshot { notices }
    }

    /// Closes `id` if it is in force, dismissible and not already closed, answering how. A notice
    /// the book remembers is hidden from the next snapshot on; one whose fact lives elsewhere is
    /// left for the caller to clear there.
    pub(crate) fn dismiss(&mut self, id: &str) -> Option<Dismissal> {
        let id = NoticeId(id.to_owned());
        let dismissal = self.in_force.get(&id)?.dismissal.clone()?;
        if dismissal == Dismissal::Remembered && !self.dismissed.insert(id) {
            return None;
        }
        Some(dismissal)
    }

    /// Marks `id`'s remedy as failed if it is in force and has one, answering whether it did.
    pub(crate) fn note_remedy_failed(&mut self, id: &str) -> bool {
        let id = NoticeId(id.to_owned());
        let Some(action) = self.in_force.get(&id).and_then(|notice| notice.action) else {
            return false;
        };
        self.remedy_failed.insert(id, action) != Some(action)
    }
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
