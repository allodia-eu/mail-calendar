//! Which addresses an account's calendar server treats as its user (RFC 6638 §2.4.1): what a
//! link between a mail account and a calendar is suggested from.

use engine_api::{AccountId, CalendarUserAddresses};
use engine_provider::Provider;

use crate::App;

impl<P: Provider> App<P> {
    /// What `id`'s calendar server treats as its user, asked of its calendar provider once per
    /// connection. `None` while it has none to ask: not dialled yet, or no calendar. A server
    /// that could not be asked answers [`CalendarUserAddresses::Unknown`], which recognises
    /// nothing, and is asked again next time.
    pub async fn calendar_user_addresses(&self, id: &AccountId) -> Option<CalendarUserAddresses> {
        let handle = self.account_handle(id).await?;
        let provider = handle.calendar_providers.first()?;
        let known = self
            .links
            .lock()
            .expect("links mutex poisoned")
            .calendar_users
            .get(id.as_str())
            .cloned();
        if let Some(known) = known {
            return Some(known);
        }
        Some(
            match self.engine.calendar_user_addresses(provider, id).await {
                Ok(CalendarUserAddresses::Unknown) => CalendarUserAddresses::Unknown,
                Ok(addresses) => {
                    self.links
                        .lock()
                        .expect("links mutex poisoned")
                        .calendar_users
                        .insert(id.as_str().to_owned(), addresses.clone());
                    addresses
                }
                Err(err) => {
                    log::info!(
                        "calendar: a{} did not say which addresses it schedules as ({err})",
                        self.account_ordinal(id).await
                    );
                    CalendarUserAddresses::Unknown
                }
            },
        )
    }
}

#[cfg(test)]
#[path = "calendar_users_tests.rs"]
mod tests;
