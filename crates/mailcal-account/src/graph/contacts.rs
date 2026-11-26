//! Building the Microsoft Graph contact adapters a Microsoft account syncs its address books
//! through: the contacts parallel of the calendar module beside this one, and the Graph
//! counterpart of [`crate::connect_google_contact_providers`].
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
//! Every source is bound as a `ReconnectingContacts` (`crate::contacts_reconnect`) on one client
//! of the account's, which asks the account's token source for the token on each request.

use std::sync::Arc;

use engine_core::{
    ids::{AccountId, AddressBookId},
    sync::SyncUpdate,
};
use engine_provider::{ContactSourceSync, ContactsProvider, ProviderError, ProviderResult};
use provider_graph::{GraphClient, GraphContactProvider, GraphContactSource, MailboxPrincipal};

use super::{GraphTokenSource, account_client};
use crate::{
    AccountError,
    contacts_reconnect::ReconnectingContacts,
    shared_client::{Delegate, SharedClient},
    tls::tls_with,
};

/// The provider speaking for `source` on `client`.
fn bind(
    source: GraphContactSource,
    client: Arc<SharedClient<GraphClient>>,
) -> ProviderResult<ReconnectingContacts<GraphClient, GraphContactProvider>> {
    Ok(ReconnectingContacts {
        delegate: Delegate::new(client, move |client| build(&source, client))?,
    })
}

/// The engine adapter for one source.
fn build(source: &GraphContactSource, client: GraphClient) -> GraphContactProvider {
    match source {
        GraphContactSource::Personal(book) => {
            GraphContactProvider::personal_folder(client, book.clone())
        }
        GraphContactSource::Organizational => GraphContactProvider::organizational(client),
        GraphContactSource::Directory => GraphContactProvider::directory(client),
    }
}

/// Connects the Graph contact adapters a Microsoft account syncs its address books through:
/// one per personal contacts folder, then the tenant directory when `directory` says the person
/// chose colleagues from their organisation, all on one client.
///
/// # Errors
///
/// Returns [`AccountError`] if the TLS policy cannot be built, the token refresh fails (a dead
/// refresh token → [`AccountError::SigninRejected`], the re-auth signal), or the folder listing
/// fails, which is also what an account whose grant lacks `Contacts.ReadWrite` answers.
pub async fn connect_graph_contact_providers(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    directory: bool,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let tls = tls_with(&[])?;
    // A dead grant fails here as a sign-in to repeat, rather than as a folder list that failed.
    tokens.access_token().await?;
    let graph = |err: ProviderError| AccountError::Graph(err.to_string());
    let client = account_client(tokens, MailboxPrincipal::Me, tls);
    let (_, current) = client.current().map_err(graph)?;
    let root = GraphContactProvider::personal(current);
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
    bound_sources(&root_book, books.into_iter().map(|book| book.id), directory)
        .into_iter()
        .map(|source| {
            bind(source, Arc::clone(&client))
                .map(|provider| Box::new(provider) as Box<dyn ContactsProvider>)
                .map_err(graph)
        })
        .collect()
}

/// The sources an account binds, in binding order: the default contacts folder first, so it is
/// the create target a client preselects; then the other folders; then the directory, when the
/// person chose colleagues from their organisation.
fn bound_sources(
    root: &AddressBookId,
    discovered: impl IntoIterator<Item = AddressBookId>,
    directory: bool,
) -> Vec<GraphContactSource> {
    let mut sources = vec![GraphContactSource::Personal(root.clone())];
    sources.extend(
        discovered
            .into_iter()
            .filter(|book| book != root)
            .map(GraphContactSource::Personal),
    );
    if directory {
        sources.push(GraphContactSource::Directory);
    }
    sources
}

#[cfg(test)]
#[path = "contacts_tests.rs"]
mod tests;
