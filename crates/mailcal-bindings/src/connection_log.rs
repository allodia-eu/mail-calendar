//! Privacy-safe connection diagnostics emitted after a provider has connected.

use engine_api::ContactsProvider;
use engine_provider::ConnectionInfo;

use crate::BoxedAccount;

/// Logs every live provider's [`engine_provider::ConnectionInfo`] for one account.
///
/// `account_label` must be a non-identifying label such as `account[0]` or
/// `new-account`; account ids embed addresses and must never be logged. `account_type`
/// is the provider family (`imap`, `graph`, `jmap`, ...), not an endpoint or account id.
pub(crate) fn log_account_connection_info(
    account_label: &str,
    account_type: &str,
    account: &BoxedAccount,
) {
    let logged = log_provider_infos(
        account_label,
        account_type,
        "mail",
        account
            .providers
            .iter()
            .map(|provider| provider.connection_info()),
    ) + log_provider_infos(
        account_label,
        account_type,
        "calendar",
        account
            .calendar_providers
            .iter()
            .map(|provider| provider.connection_info()),
    ) + log_provider_infos(
        account_label,
        account_type,
        "contacts",
        account
            .contact_providers
            .iter()
            .map(|provider| provider.connection_info()),
    );
    log_contact_destinations(account_label, account_type, account);
    if logged == 0 {
        log::info!(
            "connection_info: {account_label} account_type={account_type} no live providers",
        );
    }
}

/// Logs which address book each bound contact source is, and whether it takes writes.
///
/// Separate from the `ConnectionInfo` lines above because it answers a different question.
/// `ConnectionInfo` says *how* the transport came up; this says **what got bound**, which is
/// the one thing a live contacts session actually needs: an account with three address books
/// where only two appear is an account where the third was never bound, and no transport line
/// can distinguish that from a book the server returned empty. `writable` rides along because
/// a source that advertises no destination at all is read-only, and this is where that becomes
/// a fact in the log rather than an inference from the absence of a save affordance.
///
/// An **address-book id is a container id, not content**: the rule `docs/logging.md` states
/// explicitly: so it may be logged where a card's name or address never may.
fn log_contact_destinations(account_label: &str, account_type: &str, account: &BoxedAccount) {
    if account.contact_providers.is_empty() {
        // Emitted for **every** account family, not just the two that can bind sources, because
        // this is the only line a Graph or Google account produces about contacts at all; those
        // need OAuth scopes this build does not request, and without a line here their empty
        // Contacts list is indistinguishable from a broken one (`docs/contacts.md`, Known gaps).
        log::info!(
            "connection_info: {account_label} account_type={account_type} \
             contacts_sources=0; this account contributes no contact cards",
        );
        return;
    }
    for (index, provider) in account.contact_providers.iter().enumerate() {
        match provider.contact_destination() {
            Some(destination) => log::info!(
                "connection_info: {account_label} account_type={account_type} \
                 contacts_source[{index}] book={} class={:?} writable={}",
                destination.address_book.as_str(),
                destination.source_class,
                destination.writable,
            ),
            None => log::info!(
                "connection_info: {account_label} account_type={account_type} \
                 contacts_source[{index}] read-only, no destination advertised",
            ),
        }
    }
}

fn log_provider_infos(
    account_label: &str,
    account_type: &str,
    provider_kind: &str,
    infos: impl IntoIterator<Item = ConnectionInfo>,
) -> usize {
    let groups = connection_info_groups(infos);
    for (index, (info, count)) in groups.iter().enumerate() {
        log::info!(
            "connection_info: {account_label} account_type={account_type} \
             {provider_kind}_providers[{index}] count={count} {info:?}",
        );
    }
    groups.into_iter().map(|(_, count)| count).sum()
}

fn connection_info_groups(
    infos: impl IntoIterator<Item = ConnectionInfo>,
) -> Vec<(ConnectionInfo, usize)> {
    let mut groups = Vec::new();
    for info in infos {
        if let Some((_, count)) = groups.iter_mut().find(|(existing, _)| *existing == info) {
            *count += 1;
        } else {
            groups.push((info, 1));
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use engine_provider::{Capabilities, HttpVersion, TlsVersion};

    use super::*;

    #[test]
    fn groups_identical_connection_info_values() {
        let imap = ConnectionInfo {
            tls_version: Some(TlsVersion::Tls1_3),
            ..ConnectionInfo::new(Capabilities::none().with_mail())
        };
        let caldav = ConnectionInfo {
            http_version: Some(HttpVersion::Http2),
            ..ConnectionInfo::new(Capabilities::none().with_calendars())
        };

        assert_eq!(
            connection_info_groups([imap, imap, caldav]),
            vec![(imap, 2), (caldav, 1)],
        );
    }
}
