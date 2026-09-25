//! Building the Google People contact adapters a Google account syncs its address books
//! through, with automatic OAuth token refresh: the contacts parallel of the Gmail provider in
//! the parent module and of [`crate::connect_carddav_contact_providers`].
//!
//! **Source-bound, like CardDAV rather than like JMAP.** One People adapter speaks for exactly
//! one source, so an account fans out into one provider apiece over the three sources the app
//! reads:
//!
//! - **Connections**, the user's own address book (`people/me/connections`), the only one that
//!   accepts a write, and so the only one that offers a destination to create a card in.
//! - **Other Contacts** (`otherContacts.list`), the addresses Google saves from the user's mail on
//!   their behalf. Read-only at the source, not merely in what we offer.
//! - **Directory** (`people:listDirectoryPeople`), colleagues on a Workspace domain.
//!
//! **A personal account is not a special case here.** Neither read-only source exists for an
//! account with no Workspace domain behind it, and People answers `403` for both. The engine
//! turns that into `ContactSourceSync::Unavailable` rather than an error, so the account binds
//! the same three providers however it is hosted, and the two that have nothing to read say so
//! once per pass instead of failing the sync. Deciding here which sources an account "should"
//! have would mean guessing at Workspace membership from an address, which a `gmail.com`
//! address does not tell us either way.
//!
//! **Contact groups are the fourth source and are deliberately not bound.** The engine can read
//! them, but the view-model filters group cards out of the people list
//! ([`docs/contacts.md`](../../../../docs/contacts.md)), so binding them would cost a sync pass
//! per account to produce rows nothing draws.
//!
//! **No reconnect loop, unlike the mail and calendar siblings.** Their delegates are dropped and
//! rebuilt on a retryable error, which leaves a window where nothing is cached; here the cached
//! delegate is also what answers the three *synchronous* questions the trait asks (both sync
//! scopes and the write destination), and those must not fall through to
//! [`ContactsProvider`]'s JMAP-shaped defaults, which would key every Google source's cursor to
//! one JMAP scope. So the delegate is replaced, never emptied, and a dead keep-alive socket is
//! left to the retry policy inside [`GoogleClient`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine_core::{
    contact::{AddressBook, ContactCard, ContactDraft, ContactPatch, ContactResource},
    ids::{AccountId, ContactId},
    sync::{SyncScope, SyncState},
};
use engine_provider::{
    CalendarWrites, ConnectionInfo, ContactDestination, ContactPhoto, ContactSourceSync,
    ContactWriteReceipt, ContactsProvider, Provider, ProviderError, ProviderResult,
};
use engine_tls::TlsClientConfig;
use provider_google::{GoogleClient, GoogleContactProvider, GoogleContactSource};

use crate::{AccountError, GraphTokenSource, tls::tls_with};

/// The People sources an account binds, in the order they are bound. Groups is the one the
/// engine also offers and this list leaves out; the module doc says why.
const BOUND_SOURCES: &[GoogleContactSource] = &[
    GoogleContactSource::Connections,
    GoogleContactSource::OtherContacts,
    GoogleContactSource::Directory,
];

/// A [`ContactsProvider`] bound to one Google People source that refreshes its access token
/// before every network call and delegates to a [`GoogleContactProvider`] built with it.
/// Internal; the account layer hands callers `Box<dyn ContactsProvider>` from
/// [`connect_google_contact_providers`].
#[derive(Debug)]
pub(crate) struct RefreshingGoogleContactProvider {
    /// The People source this provider speaks for; what a rebuilt delegate is rebuilt as.
    source: GoogleContactSource,
    /// The shared, self-refreshing token source, shared with the account's Gmail and calendar
    /// providers so one refresh serves all of them.
    tokens: Arc<GraphTokenSource>,
    /// The account's shared TLS policy, cloned into each rebuilt Google client.
    tls: TlsClientConfig,
    /// The delegate and the access token it was built with. **Never empty**: one is built at
    /// connect and thereafter replaced rather than cleared, because the synchronous trait
    /// methods read it (see the module doc).
    cached: Mutex<(String, Arc<GoogleContactProvider>)>,
}

impl RefreshingGoogleContactProvider {
    /// Binds a refreshing provider to the source `delegate` speaks for, sharing `tokens`.
    fn new(
        source: GoogleContactSource,
        tokens: Arc<GraphTokenSource>,
        tls: TlsClientConfig,
        delegate: (String, Arc<GoogleContactProvider>),
    ) -> Self {
        Self {
            source,
            tokens,
            tls,
            cached: Mutex::new(delegate),
        }
    }

    /// Returns the delegate, reusing the cached one while the access token is unchanged and
    /// rebuilding it (and its warm connection pool) only on a refresh.
    async fn delegate(&self) -> ProviderResult<Arc<GoogleContactProvider>> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        {
            let cache = self.cached.lock().expect("google contacts mutex poisoned");
            if cache.0 == token {
                return Ok(Arc::clone(&cache.1));
            }
        }
        let provider = Arc::new(build(self.source, &token, &self.tls, &self.tokens.retry())?);
        *self.cached.lock().expect("google contacts mutex poisoned") =
            (token, Arc::clone(&provider));
        Ok(provider)
    }

    /// The cached delegate, for the questions that are answered without a network call: which
    /// sync scopes this source keys its cursors under, and whether it accepts a create.
    fn bound(&self) -> Arc<GoogleContactProvider> {
        Arc::clone(
            &self
                .cached
                .lock()
                .expect("google contacts mutex poisoned")
                .1,
        )
    }
}

/// Builds the People adapter for one source on a fresh access token.
///
/// `retry` comes from the account's own [`GraphTokenSource`], not from a global: these
/// providers belong to the same account as its mail and calendar, and a server that limits
/// concurrency counts all of them together (`crate::throttle`).
fn build(
    source: GoogleContactSource,
    token: &str,
    tls: &TlsClientConfig,
    retry: &engine_api::RetryConfig,
) -> Result<GoogleContactProvider, ProviderError> {
    let client =
        GoogleClient::connect(token.to_owned(), tls, retry).map_err(ProviderError::from)?;
    Ok(match source {
        GoogleContactSource::Connections => GoogleContactProvider::connections(client),
        GoogleContactSource::OtherContacts => GoogleContactProvider::other_contacts(client),
        GoogleContactSource::Directory => GoogleContactProvider::directory(client),
        GoogleContactSource::Groups => GoogleContactProvider::groups(client),
    })
}

#[async_trait]
impl Provider for RefreshingGoogleContactProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.bound().connection_info()
    }
}

impl engine_api::MailboxWrites for RefreshingGoogleContactProvider {}
impl CalendarWrites for RefreshingGoogleContactProvider {}

#[async_trait]
impl ContactsProvider for RefreshingGoogleContactProvider {
    fn address_book_scope(&self, account: &AccountId) -> SyncScope {
        self.bound().address_book_scope(account)
    }

    fn contact_scope(&self, account: &AccountId) -> SyncScope {
        self.bound().contact_scope(account)
    }

    fn contact_destination(&self) -> Option<ContactDestination> {
        // Taken from the delegate rather than rebuilt: the field set a People create accepts
        // is the adapter's to state, and a destination synthesised here would be a second
        // answer able to disagree with the one the write is actually validated against.
        self.bound().contact_destination()
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.delegate()
            .await?
            .sync_address_books(account, cursor)
            .await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.delegate().await?.sync_contacts(account, cursor).await
    }

    async fn fetch_contact(
        &self,
        account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.delegate().await?.fetch_contact(account, contact).await
    }

    async fn create_contact(
        &self,
        account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.delegate().await?.create_contact(account, draft).await
    }

    async fn patch_contact(
        &self,
        account: &AccountId,
        base: &ContactCard,
        patch: &ContactPatch,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.delegate()
            .await?
            .patch_contact(account, base, patch)
            .await
    }

    async fn delete_contact(&self, account: &AccountId, base: &ContactCard) -> ProviderResult<()> {
        self.delegate().await?.delete_contact(account, base).await
    }

    async fn fetch_contact_photo(
        &self,
        account: &AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        self.delegate()
            .await?
            .fetch_contact_photo(account, card, media)
            .await
    }
}

/// Connects the People contact adapters a Google account syncs its address books through: one
/// per source the app reads, each sharing the account's token source.
///
/// One token is minted here and seeded into every provider, so the fan-out costs a single
/// refresh rather than one per source, and each provider can answer its synchronous questions
/// (sync scopes, write destination) from the moment it is bound.
///
/// # Errors
///
/// Returns [`AccountError`] if the TLS policy cannot be built, the token refresh fails (a dead
/// refresh token → [`AccountError::SigninRejected`], the re-auth signal), or a People client
/// cannot be constructed.
pub async fn connect_google_contact_providers(
    tokens: Arc<GraphTokenSource>,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let tls = tls_with(&[])?;
    let token = tokens.access_token().await?;
    let mut providers: Vec<Box<dyn ContactsProvider>> = Vec::with_capacity(BOUND_SOURCES.len());
    for source in BOUND_SOURCES {
        let delegate = Arc::new(
            build(*source, &token, &tls, &tokens.retry())
                .map_err(|err| AccountError::Google(err.to_string()))?,
        );
        providers.push(Box::new(RefreshingGoogleContactProvider::new(
            *source,
            Arc::clone(&tokens),
            tls.clone(),
            (token.clone(), delegate),
        )));
    }
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use engine_provider::WriteGuard;
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    use super::*;

    fn google_source() -> Arc<GraphTokenSource> {
        let provider = OAuthProviderConfig::google(
            "google-client",
            None,
            "com.googleusercontent.apps.google-client:/oauth2redirect",
            GOOGLE_SCOPES,
        );
        GraphTokenSource::from_parts(
            OAuthClient::new(provider).unwrap(),
            AccountId::try_from("alice@gmail.com@mail.google.com").unwrap(),
            "refresh".to_owned(),
            None,
            "google",
            crate::CredentialOrigin::FreshSignIn,
        )
    }

    fn provider(source: GoogleContactSource) -> RefreshingGoogleContactProvider {
        let tls = tls_with(&[]).unwrap();
        let delegate =
            Arc::new(build(source, "access-token", &tls, &google_source().retry()).unwrap());
        RefreshingGoogleContactProvider::new(
            source,
            google_source(),
            tls,
            ("access-token".to_owned(), delegate),
        )
    }

    #[test]
    fn binds_the_three_sources_the_app_reads_and_not_groups() {
        assert_eq!(
            BOUND_SOURCES,
            &[
                GoogleContactSource::Connections,
                GoogleContactSource::OtherContacts,
                GoogleContactSource::Directory,
            ],
        );
    }

    /// The wrapper must forward both scopes rather than let them fall through:
    /// [`ContactsProvider`]'s defaults are JMAP-shaped, so an omission would key all three
    /// Google sources' cursors to one JMAP scope and have them overwrite each other.
    #[test]
    fn each_source_keys_its_cursors_under_its_own_google_scope() {
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        assert_eq!(
            provider(GoogleContactSource::Connections).contact_scope(&account),
            SyncScope::GoogleContacts {
                account: account.clone(),
            },
        );
        assert_eq!(
            provider(GoogleContactSource::OtherContacts).contact_scope(&account),
            SyncScope::GoogleOtherContacts {
                account: account.clone(),
            },
        );
        assert_eq!(
            provider(GoogleContactSource::Directory).contact_scope(&account),
            SyncScope::GoogleDirectoryPeople {
                account: account.clone(),
            },
        );
        assert_eq!(
            provider(GoogleContactSource::Connections).address_book_scope(&account),
            SyncScope::GoogleContactSourceList { account },
        );
    }

    /// The create affordance is drawn from the destination, so a wrapper that answered `None`
    /// until the first sync would hide it on a freshly added account.
    #[test]
    fn only_owned_connections_offer_a_write_destination() {
        let connections = provider(GoogleContactSource::Connections);
        let destination = connections
            .contact_destination()
            .expect("owned connections accept a create");
        assert!(destination.writable);
        assert_eq!(destination.write_guard, Some(WriteGuard::Enforced));
        assert!(
            provider(GoogleContactSource::OtherContacts)
                .contact_destination()
                .is_none(),
        );
        assert!(
            provider(GoogleContactSource::Directory)
                .contact_destination()
                .is_none(),
        );
    }

    /// The wrapper advertises exactly what it forwards, and it forwards the delegate's own
    /// answer rather than restating it: a capability claimed here but absent there falls
    /// through to the trait's rejecting default, which is how a provider comes to look able
    /// to do something that fails on the first attempt.
    #[test]
    fn every_source_reports_contacts_and_photos_but_only_connections_reports_writes() {
        for source in BOUND_SOURCES {
            let capabilities = provider(*source).connection_info().capabilities;
            assert!(capabilities.contacts(), "{source:?} reads contacts");
            assert!(capabilities.contact_photos(), "{source:?} reads photos");
        }
        let writes = |source| {
            provider(source)
                .connection_info()
                .capabilities
                .contact_writes()
        };
        assert!(writes(GoogleContactSource::Connections));
        assert!(!writes(GoogleContactSource::OtherContacts));
        assert!(!writes(GoogleContactSource::Directory));
    }
}
