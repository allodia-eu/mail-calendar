//! How long a connected calendar or address book is trusted before its connect is repeated.
//!
//! A connect walks from the stored principal to the home its server names now, and the home is
//! only ever kept in memory. While the app stays open the connection keeps using the home it found;
//! after [`DAV_FRESH_FOR`] the next retry connects the calendar and contacts again from the
//! principal, so a home the server has since moved is followed within a day rather than at the
//! next launch.

use std::time::{Duration, Instant};

use super::AccountRegistry;
use crate::ConnectedAccount;

/// How long a calendar or address-book connection is used before it is made again.
pub(crate) const DAV_FRESH_FOR: Duration = Duration::from_secs(24 * 60 * 60);

impl AccountRegistry {
    /// Notes that `id`'s calendar and contacts connected at `at`.
    pub(crate) fn note_dav_dialled(&self, id: &str, at: Instant) {
        self.dav_dialled
            .lock()
            .expect("dav-dialled mutex poisoned")
            .insert(id.to_owned(), at);
    }

    /// The standards accounts with a calendar or address-book server whose connection is older
    /// than [`DAV_FRESH_FOR`] at `now`. An account that has not connected yet is not listed: the
    /// retry that dials it whole is already on its way.
    pub(crate) fn dav_due(&self, now: Instant) -> Vec<String> {
        let dialled = self.dav_dialled.lock().expect("dav-dialled mutex poisoned");
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .filter(|(_, entry)| {
                matches!(entry, ConnectedAccount::Imap { config, .. }
                    if config.caldav.is_some() || config.carddav.is_some())
            })
            .filter(|(id, _)| {
                dialled
                    .get(id.as_str())
                    .is_some_and(|at| now.saturating_duration_since(*at) >= DAV_FRESH_FOR)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::DAV_FRESH_FOR;
    use crate::{ConnectedAccount, account_registry::AccountRegistry};

    fn registered(config: &str) -> std::sync::Arc<AccountRegistry> {
        let registry = AccountRegistry::new();
        let entry =
            ConnectedAccount::imap_account(mailcal_account::load_str(config).unwrap(), None);
        registry.pre_register("alice".to_owned(), entry).commit();
        registry
    }

    const DAV: &str = "[caldav]\nbase_url = \"https://dav.example.net\"\nusername = \"alice\"\npassword = \"pw\"\n";
    const MAIL_ONLY: &str = "[imap]\naddr = \"127.0.0.1:1\"\nserver_name = \"localhost\"\nusername = \"alice\"\npassword = \"pw\"\n";

    #[test]
    fn a_calendar_connection_older_than_a_day_is_made_again() {
        let registry = registered(DAV);
        let start = Instant::now();
        registry.note_dav_dialled("alice", start);

        assert!(registry.dav_due(start + Duration::from_secs(60)).is_empty());
        assert_eq!(registry.dav_due(start + DAV_FRESH_FOR), ["alice"]);
    }

    #[test]
    fn an_account_without_a_dav_server_or_not_yet_connected_is_never_due() {
        let registry = registered(MAIL_ONLY);
        let start = Instant::now();
        registry.note_dav_dialled("alice", start);
        assert!(registry.dav_due(start + DAV_FRESH_FOR).is_empty());

        let never = registered(DAV);
        assert!(never.dav_due(start + DAV_FRESH_FOR).is_empty());
    }
}
