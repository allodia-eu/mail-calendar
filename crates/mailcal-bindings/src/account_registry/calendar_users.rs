//! What each account's calendar server said about which addresses it schedules as, asked once per
//! registration, so a link suggestion costs no request on any later snapshot.

use super::AccountRegistry;

impl AccountRegistry {
    /// The addresses `id`'s calendar server schedules as; empty until it has answered, and when it
    /// named none.
    pub(crate) fn calendar_addresses(&self, id: &str) -> Vec<String> {
        self.calendar_users
            .lock()
            .expect("calendar users mutex poisoned")
            .get(id)
            .cloned()
            .flatten()
            .unwrap_or_default()
    }

    /// Claims the one ask for `id`: `true` when nobody has asked since it was registered.
    pub(crate) fn begin_calendar_ask(&self, id: &str) -> bool {
        let mut asked = self
            .calendar_users
            .lock()
            .expect("calendar users mutex poisoned");
        if asked.contains_key(id) {
            return false;
        }
        asked.insert(id.to_owned(), None);
        true
    }

    /// Records the answer to that ask. `None` means there was no calendar to ask yet, so the claim
    /// is released and a later snapshot asks again.
    pub(crate) fn calendar_answered(&self, id: &str, addresses: Option<Vec<String>>) {
        let mut asked = self
            .calendar_users
            .lock()
            .expect("calendar users mutex poisoned");
        match addresses {
            Some(addresses) => {
                asked.insert(id.to_owned(), Some(addresses));
            }
            None => {
                asked.remove(id);
            }
        }
    }

    /// Forgets what `id`'s calendar server said: it is being registered afresh or removed.
    pub(super) fn forget_calendar_users(&self, id: &str) {
        self.calendar_users
            .lock()
            .expect("calendar users mutex poisoned")
            .remove(id);
    }
}

#[cfg(test)]
mod tests {
    use crate::account_registry::AccountRegistry;

    #[test]
    fn a_calendar_is_asked_once_and_again_only_when_there_was_nobody_to_ask() {
        let registry = AccountRegistry::default();
        assert!(registry.begin_calendar_ask("cloud"));
        assert!(!registry.begin_calendar_ask("cloud"), "already being asked");

        registry.calendar_answered("cloud", None);
        assert!(
            registry.begin_calendar_ask("cloud"),
            "nobody was there to ask"
        );

        registry.calendar_answered("cloud", Some(vec!["alice@example.org".to_owned()]));
        assert!(!registry.begin_calendar_ask("cloud"));
        assert_eq!(registry.calendar_addresses("cloud"), ["alice@example.org"]);
    }

    #[test]
    fn removing_an_account_forgets_what_its_calendar_said() {
        let registry = AccountRegistry::default();
        assert!(registry.begin_calendar_ask("cloud"));
        registry.calendar_answered("cloud", Some(vec!["alice@example.org".to_owned()]));

        registry.remove("cloud");
        assert!(registry.calendar_addresses("cloud").is_empty());
        assert!(registry.begin_calendar_ask("cloud"));
    }
}
