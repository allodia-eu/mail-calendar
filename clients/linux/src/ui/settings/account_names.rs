//! How Settings names an account where another has the same address (`docs/accounts.md` rule 8).

use mailcal_bindings::{AccountDistinction, AccountEntry, LinkedAccount};

use super::account_rows::kind_label;
use crate::l10n;

/// What a person reads another account by: its address, and what tells it apart from another
/// account with that address when one exists.
pub(in crate::ui) fn linked_name(linked: &LinkedAccount) -> String {
    match &linked.distinction {
        None => linked.address.clone(),
        Some(AccountDistinction::Kind { kind }) => {
            l10n::settings_account_distinguished(&linked.address, kind_label(*kind))
        }
        Some(AccountDistinction::Server { host }) => {
            l10n::settings_account_distinguished(&linked.address, host)
        }
    }
}

/// The account's kind, as its own row and page say it. Both already say the kind, so only a
/// server's host is added, for an account another of its kind shares the address of.
pub(super) fn kind_line(entry: &AccountEntry) -> String {
    let kind = kind_label(entry.kind);
    match &entry.distinction {
        Some(AccountDistinction::Server { host }) => format!("{kind} · {host}"),
        Some(AccountDistinction::Kind { .. }) | None => kind.to_owned(),
    }
}

#[cfg(test)]
#[path = "account_names_tests.rs"]
mod tests;
