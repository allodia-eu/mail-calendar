//! What the app can and cannot currently reach, read once per pull of the notices. A host never
//! sees it: every fact here reaches it as a notice (`docs/notices.md`, "What each fact becomes").

use crate::{MailcalApp, UseProblem};

/// The connectivity facts the notices are raised from.
#[derive(Debug, Default)]
pub(crate) struct ConnectivitySnapshot {
    /// The device has no network connectivity, per the OS reachability the host reports.
    pub(crate) offline: bool,
    /// Accounts whose last sync could not reach their server while online. Empty while
    /// `offline`. A Gmail account whose mail cannot be reached is listed too, though its API
    /// calls succeed.
    pub(crate) unreachable_accounts: Vec<String>,
    /// Accounts whose calendar is withheld because their grant lacks the calendar scope.
    pub(crate) calendar_reauth_accounts: Vec<String>,
    /// Accounts whose mail writes and sends are withheld because their grant lacks those scopes.
    pub(crate) mail_reauth_accounts: Vec<String>,
    /// Accounts whose stored sign-in the server refuses. Such an account is absent from
    /// `unreachable_accounts`.
    pub(crate) signin_expired_accounts: Vec<String>,
    /// Calendars that did not connect on an account that did, with why (`docs/accounts.md`
    /// rule 16). An unreachable one is left out while `offline`, and an account in
    /// `signin_expired_accounts` is left out.
    pub(crate) calendar_outages: Vec<UseOutage>,
    /// The same as `calendar_outages`, for the address book.
    pub(crate) contacts_outages: Vec<UseOutage>,
    /// The address of the Allodia account whose sign-in the account service refused.
    pub(crate) allodia_sign_in_ended: Option<String>,
}

/// A calendar or address book that did not connect, on an account that did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UseOutage {
    /// The account's id.
    pub(crate) account_id: String,
    /// Why it did not connect.
    pub(crate) problem: UseProblem,
}

impl MailcalApp {
    /// The connectivity facts as they stand.
    pub(crate) fn connectivity(&self) -> ConnectivitySnapshot {
        let snapshot = crate::use_outages::with_outages(
            self.app.connectivity().into(),
            &self.app.every_use_problem(),
        );
        let mut snapshot = crate::gmail_unreachable::with_unreachable_gmail(
            snapshot,
            &self.registry.facts(),
            mailcal_account::gmail_mail_route,
        );
        snapshot.allodia_sign_in_ended = self.allodia_sign_in_ended();
        snapshot
    }
}
