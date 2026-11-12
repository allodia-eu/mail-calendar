//! Asking a connected account which stores its sign-in opens besides its own
//! (`docs/shared-mailboxes.md`). Nothing here is kept: the answer is the server's, read when
//! Settings asks.
//!
//! Each use is asked of the provider that serves it, its mailbox for mail and its calendar for
//! calendars, because each provider reports the stores it can open: an IMAP sign-in's CalDAV half
//! finds calendar homes its IMAP half knows nothing of, and a Microsoft calendar is checked by a
//! calendar probe, not an Inbox one. What they answer is one list: a store both report under one
//! handle, a JMAP account or a Microsoft address, is one store holding both. Each store's
//! [`SharedAccount::domains`] says what it holds of what was asked, which is what switching it on
//! opens.

use engine_api::{
    AccountId, ApiError, FailureClass, Provider, SharedAccount, SharedAccounts, StoreDomains,
    SyncError,
};
use mailcal_account::{SharedUse, SharedUses};

use crate::App;

/// What an account's sign-in can say about the stores it opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreOffer {
    /// None of the providers asked is connected, so there is nothing to ask yet.
    NotConnected,
    /// The servers list them. A server that lists them for one of the uses asked but did not
    /// answer leaves that use in `unanswered`: what it would have listed is not among `stores`,
    /// and is not known to be gone.
    Listed {
        /// What the servers that answered listed.
        stores: Vec<SharedAccount>,
        /// The uses whose server lists them and did not answer.
        unanswered: SharedUses,
    },
    /// The servers list none, but answer for an address the person types.
    ByAddress,
    /// The servers have no way to open one.
    Unsupported,
}

/// What the servers said of an address the person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreLookup {
    /// A store the sign-in opens.
    Found(SharedAccount),
    /// Nothing the sign-in can open answers to it, or it is not an address. The server will not
    /// say which of "no such store" and "not shared with you" it is.
    NotOpenable,
    /// None of the providers asked is connected, so there is nothing to ask yet.
    NotConnected,
}

impl<P: Provider> App<P> {
    /// Which of `uses` `account` has a live provider for, to ask which stores it opens: what
    /// [`App::shared_store_offer`] answers [`SharedStoreOffer::NotConnected`] without.
    pub async fn live_shared_uses(&self, account: &AccountId, uses: SharedUses) -> SharedUses {
        match self.account_handle(account).await {
            Some(handle) => uses
                .iter()
                .filter(|kind| asked_of(&handle, *kind).is_some())
                .collect(),
            None => SharedUses::default(),
        }
    }

    /// Which stores `account`'s sign-in opens besides its own that hold any of `uses`, asked of
    /// its live provider for each.
    ///
    /// A server that lists them outranks one that answers by address, which outranks one that
    /// has no way: the list is what can be drawn, and an address field beside it would offer
    /// what the list already holds.
    ///
    /// One server not answering does not hide what another listed: the list is drawn with its
    /// use marked unanswered.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when a server was asked and did not answer, and no other
    /// listed any.
    pub async fn shared_store_offer(
        &self,
        account: &AccountId,
        uses: SharedUses,
    ) -> Result<SharedStoreOffer, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreOffer::NotConnected);
        };
        let (mut listed, mut by_address, mut connected) = (None, false, false);
        let (mut unanswered, mut failed) = (SharedUses::default(), None);
        for kind in uses.iter() {
            let Some(provider) = asked_of(&handle, kind) else {
                continue;
            };
            connected = true;
            match provider.connection_info().capabilities.shared_accounts() {
                SharedAccounts::Unsupported => {}
                SharedAccounts::ByAddress => by_address = true,
                SharedAccounts::Enumerable => {
                    match self.engine.list_shared_accounts(provider).await {
                        Ok(found) => {
                            let all = listed.get_or_insert_with(Vec::new);
                            for store in found {
                                merge(all, store, kind);
                            }
                        }
                        Err(err) => {
                            unanswered = unanswered.with(kind);
                            failed.get_or_insert(err);
                        }
                    }
                }
            }
        }
        Ok(match (listed, failed) {
            (Some(stores), _) => SharedStoreOffer::Listed { stores, unanswered },
            (None, Some(err)) => return Err(err),
            (None, None) if by_address => SharedStoreOffer::ByAddress,
            (None, None) if connected => SharedStoreOffer::Unsupported,
            (None, None) => SharedStoreOffer::NotConnected,
        })
    }

    /// The store at `address`, asked of `account`'s live provider for each of `uses`; found when
    /// any of them opens it, holding what each that did found. One provider's failure does not
    /// hide what another found: a grant from before shared calendars opens a mailbox whose
    /// calendar its calendar probe is refused.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when no provider found the store and one did not answer, or
    /// refused the sign-in.
    pub async fn resolve_shared_store(
        &self,
        account: &AccountId,
        uses: SharedUses,
        address: &str,
    ) -> Result<SharedStoreLookup, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreLookup::NotConnected);
        };
        let (mut found, mut connected, mut failed) = (Vec::new(), false, None);
        for kind in uses.iter() {
            let Some(provider) = asked_of(&handle, kind) else {
                continue;
            };
            connected = true;
            match self.engine.resolve_shared_account(provider, address).await {
                Ok(store) => merge(&mut found, store, kind),
                Err(ApiError::InvalidInput(_)) => {}
                Err(ApiError::Sync(SyncError::Provider(err)))
                    if err.class() == FailureClass::Permanent => {}
                Err(err) => {
                    failed.get_or_insert(err);
                }
            }
        }
        // Each provider that opens it answers under the address it was asked, so one handle.
        Ok(match (found.into_iter().next(), failed) {
            (Some(store), _) => SharedStoreLookup::Found(store),
            (None, Some(err)) => return Err(err),
            (None, None) if connected => SharedStoreLookup::NotOpenable,
            (None, None) => SharedStoreLookup::NotConnected,
        })
    }
}

/// The provider `account` is asked of for `kind`: its mailbox for mail, its calendar for
/// calendars.
fn asked_of<P>(account: &crate::Account<P>, kind: SharedUse) -> Option<&P> {
    match kind {
        SharedUse::Mail => account.providers.first(),
        SharedUse::Calendar => account.calendar_provider.as_ref(),
    }
}

/// Adds `store`, reported by the provider asked for `kind`, to `into`: `kind` joined to what the
/// store already there under its handle holds, or a store of its own holding `kind` alone. A
/// provider reports whatever it serves of a store, and only `kind` was asked of this one. A store
/// that does not hold `kind` is not one this provider opens for it: a server lists every store the
/// sign-in opens, a colleague's calendar among them, and one without mail is no mailbox.
fn merge(into: &mut Vec<SharedAccount>, mut store: SharedAccount, kind: SharedUse) {
    let asked = StoreDomains::of(kind.domain());
    if !store.domains.contains(kind.domain()) {
        return;
    }
    if let Some(kept) = into.iter_mut().find(|kept| kept.handle == store.handle) {
        kept.domains = kept.domains.union(asked);
    } else {
        store.domains = asked;
        into.push(store);
    }
}

#[cfg(test)]
mod tests {
    use engine_api::{SearchDomain, SharedAccountId, StoreDomains};

    use super::*;

    fn store(handle: &str, domains: StoreDomains) -> SharedAccount {
        SharedAccount::new(SharedAccountId::try_from(handle).unwrap(), handle, domains)
    }

    fn both() -> StoreDomains {
        StoreDomains::of(SearchDomain::Mail).with(SearchDomain::Calendar)
    }

    #[test]
    fn one_handle_reported_for_mail_and_for_calendars_is_one_store_holding_both() {
        let mut offered = Vec::new();
        merge(&mut offered, store("group", both()), SharedUse::Mail);
        merge(&mut offered, store("group", both()), SharedUse::Calendar);
        assert_eq!(offered.len(), 1);
        assert_eq!(
            SharedUses::held_by(&offered[0]),
            SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar)
        );
    }

    #[test]
    fn a_store_holds_only_what_the_provider_asked_for_it_found() {
        let mut offered = Vec::new();
        // A JMAP mailbox provider lists a colleague's calendar too; it is no mailbox.
        let colleague = store("colleague", StoreDomains::of(SearchDomain::Calendar));
        merge(&mut offered, colleague.clone(), SharedUse::Mail);
        assert_eq!(offered, []);
        merge(&mut offered, colleague, SharedUse::Calendar);
        assert_eq!(
            SharedUses::held_by(&offered[0]),
            SharedUses::of(SharedUse::Calendar)
        );
        // A group asked of the mailbox alone holds mail, whatever else it holds.
        merge(&mut offered, store("group", both()), SharedUse::Mail);
        assert_eq!(
            SharedUses::held_by(&offered[1]),
            SharedUses::of(SharedUse::Mail)
        );
    }

    #[test]
    fn handles_from_different_servers_stay_different_stores() {
        // An IMAP account's mail server and calendar server name stores their own way.
        let mut offered = Vec::new();
        let folders = store(
            "Shared Folders/support",
            StoreDomains::of(SearchDomain::Mail),
        );
        let principal = store(
            "https://dav.example.org/principals/support/",
            StoreDomains::of(SearchDomain::Calendar),
        );
        merge(&mut offered, folders, SharedUse::Mail);
        merge(&mut offered, principal, SharedUse::Calendar);
        assert_eq!(offered.len(), 2);
    }

    /// A server that opens the stores in `domain`, as `shares` says: listing them as a JMAP or
    /// CalDAV server does, or answering an address as Graph's mailbox and calendar halves do.
    /// It answers with `refusal` in place of the store when given.
    #[derive(Debug)]
    struct Prober {
        domain: SearchDomain,
        shares: SharedAccounts,
        refusal: Option<engine_api::FailureClass>,
    }

    impl Prober {
        fn answer(&self, handle: &str) -> engine_provider::ProviderResult<SharedAccount> {
            match self.refusal {
                Some(class) => Err(engine_provider::ProviderError::new(class, "refused")),
                None => Ok(store(handle, StoreDomains::of(self.domain))),
            }
        }
    }

    #[async_trait::async_trait]
    impl engine_provider::Provider for Prober {
        fn connection_info(&self) -> engine_provider::ConnectionInfo {
            engine_provider::ConnectionInfo::new(
                engine_provider::Capabilities::none()
                    .with_mail()
                    .with_calendars()
                    .with_shared_accounts(self.shares),
            )
        }

        async fn list_shared_accounts(
            &self,
        ) -> engine_provider::ProviderResult<Vec<SharedAccount>> {
            self.answer("support").map(|store| vec![store])
        }

        async fn resolve_shared_account(
            &self,
            address: &str,
        ) -> engine_provider::ProviderResult<SharedAccount> {
            self.answer(address)
        }
    }

    impl engine_api::MailboxWrites for Prober {}
    impl engine_provider::CalendarWrites for Prober {}

    struct Silent;

    impl crate::AppObserver for Silent {
        fn surface_changed(&self, _surface: crate::Surface) {}
    }

    fn account_id() -> AccountId {
        AccountId::try_from("alice@example.org@graph.microsoft.com").unwrap()
    }

    fn app(shares: SharedAccounts, calendar: Option<engine_api::FailureClass>) -> App<Prober> {
        let mail = Prober {
            domain: SearchDomain::Mail,
            shares,
            refusal: None,
        };
        let calendar = Prober {
            domain: SearchDomain::Calendar,
            shares,
            refusal: calendar,
        };
        App::new(
            engine_api::Engine::open_in_memory().unwrap(),
            vec![crate::Account {
                id: account_id(),
                providers: vec![mail].into(),
                calendar_provider: Some(calendar),
                meetings: None,
                contact_providers: Vec::new(),
                identity: engine_api::EmailAddress::new("alice@example.org"),
                dialled: true,
                uses_mail: true,
            }],
            crate::TimeZoneInit {
                device_zone: engine_api::TimeZoneId::utc(),
                prefs_path: None,
            },
            None,
            std::sync::Arc::new(Silent),
            crate::Telemetry::off(None),
        )
    }

    #[tokio::test]
    async fn a_calendar_probe_refused_leaves_the_mailbox_another_provider_found() {
        // A grant from before shared calendars: the calendar half is refused the scope.
        let both = SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar);
        let lookup = app(
            SharedAccounts::ByAddress,
            Some(engine_api::FailureClass::Authentication),
        )
        .resolve_shared_store(&account_id(), both, "support@example.org")
        .await
        .expect("the mailbox was found");
        let SharedStoreLookup::Found(found) = lookup else {
            panic!("the mailbox was found: {lookup:?}");
        };
        assert_eq!(SharedUses::held_by(&found), SharedUses::of(SharedUse::Mail));
    }

    #[tokio::test]
    async fn a_refusal_is_said_when_nothing_else_found_the_store() {
        let calendar = SharedUses::of(SharedUse::Calendar);
        assert!(
            app(
                SharedAccounts::ByAddress,
                Some(engine_api::FailureClass::Authentication)
            )
            .resolve_shared_store(&account_id(), calendar, "support@example.org")
            .await
            .is_err()
        );
        assert_eq!(
            app(
                SharedAccounts::ByAddress,
                Some(engine_api::FailureClass::Permanent)
            )
            .resolve_shared_store(&account_id(), calendar, "support@example.org")
            .await
            .unwrap(),
            SharedStoreLookup::NotOpenable
        );
    }

    #[tokio::test]
    async fn a_listing_refused_leaves_what_another_server_listed() {
        // An IMAP mailbox lists its shared folders; the CalDAV server beside it did not answer.
        let both = SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar);
        let offer = app(
            SharedAccounts::Enumerable,
            Some(engine_api::FailureClass::Retryable),
        )
        .shared_store_offer(&account_id(), both)
        .await
        .expect("the mailbox listed its stores");
        let SharedStoreOffer::Listed { stores, unanswered } = offer else {
            panic!("the mailbox listed its stores: {offer:?}");
        };
        assert_eq!(stores.len(), 1);
        assert_eq!(
            SharedUses::held_by(&stores[0]),
            SharedUses::of(SharedUse::Mail)
        );
        assert_eq!(unanswered, SharedUses::of(SharedUse::Calendar));
        // With nothing else listed, the refusal is the answer.
        assert!(
            app(
                SharedAccounts::Enumerable,
                Some(engine_api::FailureClass::Retryable)
            )
            .shared_store_offer(&account_id(), SharedUses::of(SharedUse::Calendar))
            .await
            .is_err()
        );
    }
}
