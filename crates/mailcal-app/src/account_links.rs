//! The accounts an account relies on for what it does not do itself (`docs/accounts.md` rule 12),
//! read where this layer acts on them: an invitation that arrived in a mail account is filed
//! into and answered from the calendar that account links to, and an answer given from that
//! calendar is emailed through the mail account linked to it (`docs/invitations.md`, "Which
//! calendar an invitation uses").
//!
//! The binding layer stores the links and decides which of them hold, so this layer is handed a
//! source to read them from rather than a copy: a link set, cleared or left dangling in Settings
//! counts from the next read.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use engine_api::{AccountId, CalendarUserAddresses, addresses_match};
use mailcal_account::{AccountLinks, Preferences, load_preferences};

use crate::App;

/// Every account's links that hold, keyed by account id; an account with none may be absent.
pub type LinksSource = Arc<dyn Fn() -> BTreeMap<String, AccountLinks> + Send + Sync>;

/// Where the links are read from, and what each calendar server said it schedules as.
#[derive(Default)]
pub(crate) struct LinkState {
    source: Option<LinksSource>,
    /// The answers a calendar server gave about its user (RFC 6638 §2.4.1), kept until the
    /// account is connected again. An answer the server could not give is not kept.
    pub(crate) calendar_users: HashMap<String, CalendarUserAddresses>,
    /// Moved on by every forget, so an answer asked for before one is not kept.
    pub(crate) calendar_users_epoch: u64,
}

impl<P> App<P> {
    /// Reads every account's links from `source` from now on.
    pub fn set_links_source(&self, source: LinksSource) {
        self.links.lock().expect("links mutex poisoned").source = Some(source);
    }

    /// `id`'s links that hold; none where no source was set.
    pub(crate) fn links_of(&self, id: &AccountId) -> AccountLinks {
        self.every_link().remove(id.as_str()).unwrap_or_default()
    }

    /// The calendar an invitation that arrived in `mail` is filed into and answered from: the
    /// account's own, or the calendar it links to when it is not used for one.
    pub fn invitation_calendar(&self, mail: &AccountId) -> AccountId {
        self.links_of(mail).calendar.unwrap_or_else(|| mail.clone())
    }

    /// The account whose mail sends an answer given from `calendar`: the account itself, or the
    /// mail account the calendar sends through when it is not used for mail.
    pub(crate) fn invitation_mail(&self, calendar: &AccountId) -> AccountId {
        self.links_of(calendar)
            .mail
            .unwrap_or_else(|| calendar.clone())
    }

    /// The accounts `id` is linked with for invitations, either way round: the calendar it
    /// files into, the mail account it sends through, and every mail account filing into it.
    pub(crate) fn invitation_partners(&self, id: &AccountId) -> Vec<AccountId> {
        let links = self.every_link();
        let own = links.get(id.as_str()).cloned().unwrap_or_default();
        let mut partners: Vec<AccountId> = own.calendar.into_iter().chain(own.mail).collect();
        for (other, theirs) in &links {
            if theirs.calendar.as_ref() == Some(id)
                && let Ok(other) = AccountId::try_from(other.as_str())
                && !partners.contains(&other)
            {
                partners.push(other);
            }
        }
        partners
    }

    /// Forgets what `id`'s calendar server said it schedules as, for an account connected again.
    pub(crate) fn forget_calendar_users(&self, id: &AccountId) {
        let mut links = self.links.lock().expect("links mutex poisoned");
        links.calendar_users.remove(id.as_str());
        links.calendar_users_epoch = links.calendar_users_epoch.wrapping_add(1);
    }

    fn every_link(&self) -> BTreeMap<String, AccountLinks> {
        // Cloned out first: the source takes the binding layer's own lock.
        let source = self
            .links
            .lock()
            .expect("links mutex poisoned")
            .source
            .clone();
        source.map(|read| read()).unwrap_or_default()
    }
}

impl<P: engine_api::Provider> App<P> {
    /// Every address that is *this account's own*: its primary identity plus its configured
    /// aliases, and the same of each account it is linked with for invitations
    /// ([`Self::invitation_partners`]), since a mail account and the calendar it files into
    /// answer as one person.
    ///
    /// This is the **persisted** set, and therefore the one the calendar grid uses too: a grid
    /// has no message to read delivery headers from. The reading view widens it with the
    /// message's own delivery recipients.
    pub(crate) async fn account_address_set(&self, account: &AccountId) -> Vec<String> {
        // Read from disk rather than caching: an alias list is edited rarely and read on two
        // cold paths (opening a message, rebuilding the calendar cache), both of which already do
        // store or network I/O: so one small TOML read is noise, and it can never be stale after
        // the user edits it.
        let preferences = self.prefs_path.as_ref().map(load_preferences);
        let mut addresses = self.own_addresses(account, preferences.as_ref()).await;
        for partner in self.invitation_partners(account) {
            addresses.extend(self.own_addresses(&partner, preferences.as_ref()).await);
        }
        addresses
    }

    /// The account whose mail sends `attendee`'s answer given from `calendar`: the calendar's
    /// mail account ([`Self::invitation_mail`]) when the address is the calendar's own, else the
    /// account linked with it that the address belongs to, so a calendar two mailboxes file into
    /// answers each invitation from the mailbox it invited.
    pub(crate) async fn answering_mail(&self, calendar: &AccountId, attendee: &str) -> AccountId {
        let preferences = self.prefs_path.as_ref().map(load_preferences);
        let owns = |addresses: Vec<String>| {
            addresses
                .iter()
                .any(|address| addresses_match(attendee, address))
        };
        if !owns(self.own_addresses(calendar, preferences.as_ref()).await) {
            for partner in self.invitation_partners(calendar) {
                if owns(self.own_addresses(&partner, preferences.as_ref()).await) {
                    return partner;
                }
            }
        }
        self.invitation_mail(calendar)
    }

    /// `account`'s primary identity and its configured aliases.
    async fn own_addresses(
        &self,
        account: &AccountId,
        preferences: Option<&Preferences>,
    ) -> Vec<String> {
        let mut addresses = Vec::new();
        if let Some(identity) = self.account_identity(account).await {
            addresses.push(identity.email);
        }
        if let Some(preferences) = preferences {
            addresses.extend(preferences.aliases_of(account.as_str()).iter().cloned());
        }
        addresses
    }
}

#[cfg(test)]
#[path = "account_links_tests.rs"]
mod tests;
