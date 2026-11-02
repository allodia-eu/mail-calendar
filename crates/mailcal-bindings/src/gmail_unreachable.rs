//! The Gmail accounts whose mail cannot be reached, though the rest of Google answers them.
//!
//! A Gmail account reads its mail over IMAP and everything else over the API, so a network that
//! lets HTTPS through and blocks IMAP leaves its folder list syncing and its mail stopped. The
//! pass counts that account as reachable, rightly for the server it reached, so the account's
//! IMAP side says it instead ([`mailcal_account::GmailMailRoute::Unreachable`]) and the account
//! is badged like any other that cannot reach its server.

use crate::{AccountKind, ConnectivitySnapshot, accounts_view::AccountFacts};

/// `snapshot` with every Gmail account whose mail is unreachable among its unreachable accounts.
///
/// Left out while the device is offline, which the offline banner already says, and for an
/// account whose sign-in expired, whose own prompt covers it.
pub(crate) fn with_unreachable_gmail(
    mut snapshot: ConnectivitySnapshot,
    facts: &[AccountFacts],
    route: impl Fn(&str) -> mailcal_account::GmailMailRoute,
) -> ConnectivitySnapshot {
    if snapshot.offline {
        return snapshot;
    }
    for account in facts {
        let unreachable = account.kind == AccountKind::Google
            && account.chosen.contains(mailcal_account::Capability::Mail)
            && route(&account.address) == mailcal_account::GmailMailRoute::Unreachable
            && !snapshot.signin_expired_accounts.contains(&account.id)
            && !snapshot.unreachable_accounts.contains(&account.id);
        if unreachable {
            snapshot.unreachable_accounts.push(account.id.clone());
        }
    }
    snapshot
}

#[cfg(test)]
#[path = "gmail_unreachable_tests.rs"]
mod tests;
