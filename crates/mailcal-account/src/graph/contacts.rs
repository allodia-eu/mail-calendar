//! Building the Microsoft Graph contact adapters a Microsoft account syncs its address books
//! through, with automatic OAuth token refresh: the contacts parallel of the calendar module
//! beside this one, and the Graph counterpart of [`crate::connect_google_contact_providers`].
//!
//! **Source-bound.** One engine adapter speaks for exactly one source, so an account fans out
//! into one provider per source:
//!
//! - **Contacts**, the mailbox's default contacts folder (`/me/contacts`), and one provider per
//!   folder beneath it. These are the account's own cards and the only ones that accept a write.
//!   The folders are listed once, at connect; a folder created later is read from the next connect
//!   on.
//! - **Directory**, the users of a work or school tenant (`/users`), read through
//!   `User.ReadBasic.All`. A personal Microsoft account has no directory and Graph refuses the
//!   read; the engine turns that refusal into `ContactSourceSync::Unavailable`, so the account
//!   binds the same providers however it is hosted and the one with nothing to read says so once
//!   per pass.
//!
//! **Organisational contacts are not bound.** The engine can read them, but only through
//! `OrgContact.Read.All`, which the app does not request (`mailcal_oauth::MICROSOFT_GRAPH_SCOPES`
//! says why), so every pass would spend a request on a refusal.
//!
//! **The delegate is replaced, never emptied**, for the reason the Google module gives: it also
//! answers the trait's synchronous questions (both sync scopes and the write destination), and
//! those must not fall through to [`ContactsProvider`]'s JMAP-shaped defaults. A read that fails
//! on a dead keep-alive socket rebuilds the delegate on the same token and is tried once more,
//! as the Graph mail and calendar providers do; a write is tried once.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine_core::{
    contact::{AddressBook, ContactCard, ContactDraft, ContactPatch, ContactResource},
    ids::{AccountId, AddressBookId, ContactId},
    sync::{SyncScope, SyncState, SyncUpdate},
};
use engine_provider::{
    CalendarWrites, ConnectionInfo, ContactDestination, ContactPhoto, ContactSourceSync,
    ContactWriteReceipt, ContactsProvider, Provider, ProviderError, ProviderResult,
};
use engine_tls::TlsClientConfig;
use provider_graph::{GraphClient, GraphContactProvider, GraphContactSource, MailboxPrincipal};

use super::{GraphTokenSource, should_reconnect};
use crate::{AccountError, tls::tls_with};

/// A [`ContactsProvider`] bound to one Graph contact source that refreshes its access token
/// before every network call and delegates to a [`GraphContactProvider`] built with it.
/// Internal; the account layer hands callers `Box<dyn ContactsProvider>` from
/// [`connect_graph_contact_providers`].
#[derive(Debug)]
pub(crate) struct RefreshingGraphContactProvider {
    /// The source this provider speaks for; what a rebuilt delegate is rebuilt as.
    source: GraphContactSource,
    /// The shared, self-refreshing token source, shared with the account's mail and calendar
    /// providers so one refresh serves all of them.
    tokens: Arc<GraphTokenSource>,
    /// The account's shared TLS policy, cloned into each rebuilt Graph client.
    tls: TlsClientConfig,
    /// The delegate and the access token it was built with. **Never empty**; see the module doc.
    cached: Mutex<(String, Arc<GraphContactProvider>)>,
}

impl RefreshingGraphContactProvider {
    fn new(
        source: GraphContactSource,
        tokens: Arc<GraphTokenSource>,
        tls: TlsClientConfig,
        delegate: (String, Arc<GraphContactProvider>),
    ) -> Self {
        Self {
            source,
            tokens,
            tls,
            cached: Mutex::new(delegate),
        }
    }

    async fn token(&self) -> ProviderResult<String> {
        self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })
    }

    /// Returns the delegate, reusing the cached one while the access token is unchanged and
    /// rebuilding it (and its warm connection pool) only on a refresh.
    async fn delegate(&self) -> ProviderResult<Arc<GraphContactProvider>> {
        let token = self.token().await?;
        {
            let cache = self.cached.lock().expect("graph contacts mutex poisoned");
            if cache.0 == token {
                return Ok(Arc::clone(&cache.1));
            }
        }
        self.rebuild(token)
    }

    /// Builds a delegate on `token` and caches it in place of the previous one.
    fn rebuild(&self, token: String) -> ProviderResult<Arc<GraphContactProvider>> {
        let provider = Arc::new(build(
            &self.source,
            &token,
            &self.tls,
            &self.tokens.retry(),
        )?);
        *self.cached.lock().expect("graph contacts mutex poisoned") =
            (token, Arc::clone(&provider));
        Ok(provider)
    }

    /// The cached delegate, for the questions answered without a network call.
    fn bound(&self) -> Arc<GraphContactProvider> {
        Arc::clone(&self.cached.lock().expect("graph contacts mutex poisoned").1)
    }

    /// Runs an idempotent read, rebuilding the delegate once after a retryable transport
    /// failure (a keep-alive socket the OS closed while the machine slept).
    async fn read<T, F, Fut>(&self, call: F) -> ProviderResult<T>
    where
        F: Fn(Arc<GraphContactProvider>) -> Fut,
        Fut: Future<Output = ProviderResult<T>>,
    {
        match call(self.delegate().await?).await {
            Err(err) if should_reconnect(&err) => {
                let token = self.token().await?;
                call(self.rebuild(token)?).await
            }
            result => result,
        }
    }
}

/// Builds the engine adapter for one source on a fresh access token.
fn build(
    source: &GraphContactSource,
    token: &str,
    tls: &TlsClientConfig,
    retry: &engine_api::RetryConfig,
) -> Result<GraphContactProvider, ProviderError> {
    let client = GraphClient::for_mailbox(token.to_owned(), MailboxPrincipal::Me, tls, retry)
        .map_err(ProviderError::from)?;
    Ok(match source {
        GraphContactSource::Personal(book) => {
            GraphContactProvider::personal_folder(client, book.clone())
        }
        GraphContactSource::Organizational => GraphContactProvider::organizational(client),
        GraphContactSource::Directory => GraphContactProvider::directory(client),
    })
}

#[async_trait]
impl Provider for RefreshingGraphContactProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.bound().connection_info()
    }
}

impl engine_api::MailboxWrites for RefreshingGraphContactProvider {}
impl CalendarWrites for RefreshingGraphContactProvider {}

#[async_trait]
impl ContactsProvider for RefreshingGraphContactProvider {
    fn address_book_scope(&self, account: &AccountId) -> SyncScope {
        self.bound().address_book_scope(account)
    }

    fn contact_scope(&self, account: &AccountId) -> SyncScope {
        self.bound().contact_scope(account)
    }

    fn contact_destination(&self) -> Option<ContactDestination> {
        // The adapter's own answer, so the field set offered is the one the write is
        // validated against.
        self.bound().contact_destination()
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.read(|provider| async move { provider.sync_address_books(account, cursor).await })
            .await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.read(|provider| async move { provider.sync_contacts(account, cursor).await })
            .await
    }

    async fn fetch_contact(
        &self,
        account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.read(|provider| async move { provider.fetch_contact(account, contact).await })
            .await
    }

    async fn create_contact(
        &self,
        account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        // A `POST`: a blind retry could create the card twice.
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
        self.read(
            |provider| async move { provider.fetch_contact_photo(account, card, media).await },
        )
        .await
    }
}

/// Connects the Graph contact adapters a Microsoft account syncs its address books through:
/// one per personal contacts folder, then the tenant directory.
///
/// One token is minted here and seeded into every provider, so the fan-out costs a single
/// refresh, and each provider answers its synchronous questions from the moment it is bound.
///
/// # Errors
///
/// Returns [`AccountError`] if the TLS policy cannot be built, the token refresh fails (a dead
/// refresh token → [`AccountError::SigninRejected`], the re-auth signal), or the folder listing
/// fails, which is also what an account whose grant lacks `Contacts.ReadWrite` answers.
pub async fn connect_graph_contact_providers(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let tls = tls_with(&[])?;
    let token = tokens.access_token().await?;
    let graph = |err: ProviderError| AccountError::Graph(err.to_string());
    let client =
        GraphClient::for_mailbox(token.clone(), MailboxPrincipal::Me, &tls, &tokens.retry())
            .map_err(|err| AccountError::Graph(err.to_string()))?;
    let root = GraphContactProvider::personal(client);
    let root_book = root
        .contact_destination()
        .map(|destination| destination.address_book)
        .ok_or_else(|| AccountError::Graph("personal contacts offer no destination".into()))?;
    let books = match root
        .sync_address_books(account_id, None)
        .await
        .map_err(graph)?
    {
        ContactSourceSync::Available { sync, .. } => match sync.update {
            SyncUpdate::Snapshot { objects, .. } => objects,
            SyncUpdate::Delta { changed, .. } => changed,
        },
        ContactSourceSync::Unavailable(unavailable) => {
            return Err(AccountError::Graph(unavailable.reason));
        }
    };
    let sources = bound_sources(&root_book, books.into_iter().map(|book| book.id));
    let mut providers: Vec<Box<dyn ContactsProvider>> = Vec::with_capacity(sources.len());
    for source in sources {
        let delegate = Arc::new(build(&source, &token, &tls, &tokens.retry()).map_err(graph)?);
        providers.push(Box::new(RefreshingGraphContactProvider::new(
            source,
            Arc::clone(&tokens),
            tls.clone(),
            (token.clone(), delegate),
        )));
    }
    Ok(providers)
}

/// The sources an account binds, in binding order: the default contacts folder first, so it is
/// the create target a client preselects; then the other folders; then the directory.
fn bound_sources(
    root: &AddressBookId,
    discovered: impl IntoIterator<Item = AddressBookId>,
) -> Vec<GraphContactSource> {
    let mut sources = vec![GraphContactSource::Personal(root.clone())];
    sources.extend(
        discovered
            .into_iter()
            .filter(|book| book != root)
            .map(GraphContactSource::Personal),
    );
    sources.push(GraphContactSource::Directory);
    sources
}

#[cfg(test)]
#[path = "contacts_tests.rs"]
mod tests;
