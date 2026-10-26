//! How Settings names an account where another has the same address (`docs/accounts.md` rule 8).

use mailcal_bindings::{AccountDistinction, AccountEntry, LinkedAccount};

use super::account_rows::kind_label;
use crate::l10n;

/// What a person reads an account by: its address, and what tells it apart from another account
/// with that address when one exists.
pub(in crate::ui) fn distinguished(
    address: &str,
    distinction: Option<&AccountDistinction>,
) -> String {
    match distinction {
        None => address.to_owned(),
        Some(AccountDistinction::Kind { kind }) => {
            l10n::settings_account_distinguished(address, kind_label(*kind))
        }
        Some(AccountDistinction::Server { host }) => {
            l10n::settings_account_distinguished(address, host)
        }
    }
}

/// Another account, as a link names it.
pub(in crate::ui) fn linked_name(linked: &LinkedAccount) -> String {
    distinguished(&linked.address, linked.distinction.as_ref())
}

/// An account a message or a toast asks about or acts on. Its own heading stays its address.
pub(in crate::ui) fn entry_name(entry: &AccountEntry) -> String {
    distinguished(&entry.address, entry.distinction.as_ref())
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
