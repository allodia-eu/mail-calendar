//! The provider half of the sender name: reading what a server already holds, and pushing
//! a change back to the ones whose copy the account holder owns.
//!
//! Split from `sender_names`, which is the stored value and the resolver every send reads.
//! Nothing here is on a path anyone waits for: the seed is asked for once, by a setup
//! screen, and the push happens after the local value is already saved.
//!
//! Three answers, not two, and the capability carries which
//! (`engine_api::IdentityControls`): a provider with **no** identity object leaves the name
//! entirely to us (IMAP/SMTP), one with a **read-only** directory holds a name we may show
//! and may not change (Graph), and a **writable** one lets the account holder set it (JMAP,
//! Gmail). A client asks [`App::sender_name_editable`] rather than deciding from the
//! account kind, which is the switch-on-provider this whole seam exists to prevent.

use engine_api::{AccountId, IdentityControls, Provider, SenderIdentity, addresses_match};

use crate::App;

impl<P: Provider> App<P> {
    /// Whether a client may offer to change `account`'s sender name.
    ///
    /// `false` only where a provider holds the name and the account holder cannot change
    /// it: a Graph mailbox's display name is a directory attribute a tenant administrator
    /// owns, so an editor there offers an edit that cannot land. An account whose provider
    /// has no identity object at all is editable; the name is ours in the first place.
    pub async fn sender_name_editable(&self, account: &AccountId) -> bool {
        let Some(handle) = self.account_handle(account).await else {
            return false;
        };
        !handle.providers.iter().any(|provider| {
            provider.connection_info().capabilities.sender_identities()
                == Some(IdentityControls::ReadOnly)
        })
    }

    /// The name to show in a "your name" field for `account`: the one the user has already
    /// set, else the one the provider holds.
    ///
    /// Empty when neither exists, which is the ordinary IMAP case and means "ask". Asked
    /// for once, by the screen that offers the field, so a provider that is slow or
    /// unreachable costs that screen and nothing else.
    pub async fn suggested_sender_name(&self, account: &AccountId) -> String {
        if let Some(stored) = self.sender_name(account.as_str()) {
            return stored;
        }
        self.provider_sender_identity(account)
            .await
            .and_then(|identity| identity.address.name)
            .unwrap_or_default()
    }

    /// Pushes `account`'s stored name to a provider that lets the account holder set it.
    ///
    /// Best-effort by design: the local value is what this app puts in the `From` header,
    /// so a refusal or an outage changes nothing the user can see. It is logged, never
    /// surfaced, because the user asked to be called something and they now are.
    pub(crate) async fn push_sender_name(&self, account: &str) {
        let Ok(id) = AccountId::try_from(account) else {
            return;
        };
        let Some(handle) = self.account_handle(&id).await else {
            return;
        };
        let Some(provider) = handle.providers.iter().find(|provider| {
            provider.connection_info().capabilities.sender_identities()
                == Some(IdentityControls::Writable)
        }) else {
            return;
        };
        let Some(identity) = self.matching_identity(provider, &id).await else {
            return;
        };
        let name = self.sender_name(account).unwrap_or_default();
        // The account's position in the stored list, never its address (`docs/logging.md`).
        let ordinal = self.account_ordinal(&id).await;
        match self
            .engine
            .set_sender_name(provider, &id, &identity.id, &name)
            .await
        {
            Ok(()) => log::info!("sender name: a{ordinal} updated the server's copy"),
            Err(err) => log::warn!(
                "sender name: a{ordinal} kept the server's own copy ({err}); this device sends \
                 under the name that was set"
            ),
        }
    }

    /// The provider's identity for this account's own address, from the first provider that
    /// has identities at all.
    async fn provider_sender_identity(&self, account: &AccountId) -> Option<SenderIdentity> {
        let handle = self.account_handle(account).await?;
        let provider = handle.providers.iter().find(|provider| {
            provider
                .connection_info()
                .capabilities
                .sender_identities()
                .is_some()
        })?;
        self.matching_identity(provider, account).await
    }

    /// The identity whose address is this account's own.
    ///
    /// Matched on the address, never taken as the first entry: the order a provider returns
    /// is its own, and Gmail returns every send-as alias, so "the first" would be whichever
    /// alias the server felt like listing.
    async fn matching_identity(
        &self,
        provider: &P,
        account: &AccountId,
    ) -> Option<SenderIdentity> {
        let own = self.account_identity(account).await?;
        let identities = match self.engine.sender_identities(provider, account).await {
            Ok(identities) => identities,
            Err(err) => {
                log::info!("sender name: the provider did not answer with its identities ({err})");
                return None;
            }
        };
        identities
            .into_iter()
            .find(|identity| addresses_match(&identity.address.email, &own.email))
    }
}
