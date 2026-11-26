//! Building the Google People contact adapters a Google account syncs its address books
//! through: the contacts parallel of the Gmail provider in the parent module and of
//! [`crate::connect_carddav_contact_providers`].
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
//! Every source is bound as a `ReconnectingContacts` (`crate::contacts_reconnect`) on one client
//! of the account's, which asks the account's token source for the token on each request, as the
//! Graph sources are.

use std::sync::Arc;

use engine_provider::{ContactsProvider, ProviderResult};
use provider_google::{GoogleClient, GoogleContactProvider, GoogleContactSource};

use super::account_client;
use crate::{
    AccountError, GraphTokenSource,
    contacts_reconnect::ReconnectingContacts,
    shared_client::{Delegate, SharedClient},
    tls::tls_with,
};

/// The People sources an account binds, in the order they are bound. Groups is the one the
/// engine also offers and this list leaves out; the module doc says why.
const BOUND_SOURCES: &[GoogleContactSource] = &[
    GoogleContactSource::Connections,
    GoogleContactSource::OtherContacts,
    GoogleContactSource::Directory,
];

/// The sources an account binds: all of [`BOUND_SOURCES`], less the Workspace directory unless
/// the person chose colleagues from their organisation.
fn bound_sources(directory: bool) -> impl Iterator<Item = GoogleContactSource> {
    BOUND_SOURCES
        .iter()
        .copied()
        .filter(move |source| directory || *source != GoogleContactSource::Directory)
}

/// The provider speaking for `source` on `client`.
fn bind(
    source: GoogleContactSource,
    client: Arc<SharedClient<GoogleClient>>,
) -> ProviderResult<ReconnectingContacts<GoogleClient, GoogleContactProvider>> {
    Ok(ReconnectingContacts {
        delegate: Delegate::new(client, move |client| build(source, client))?,
    })
}

/// The People adapter for one source.
fn build(source: GoogleContactSource, client: GoogleClient) -> GoogleContactProvider {
    match source {
        GoogleContactSource::Connections => GoogleContactProvider::connections(client),
        GoogleContactSource::OtherContacts => GoogleContactProvider::other_contacts(client),
        GoogleContactSource::Directory => GoogleContactProvider::directory(client),
        GoogleContactSource::Groups => GoogleContactProvider::groups(client),
    }
}

/// Connects the People contact adapters a Google account syncs its address books through: one
/// per source the app reads, all on one client asking the account's token source. The Workspace
/// directory is bound only when `directory` says the person chose colleagues from their
/// organisation.
///
/// # Errors
///
/// Returns [`AccountError`] if the TLS policy cannot be built, the token refresh fails (a dead
/// refresh token → [`AccountError::SigninRejected`], the re-auth signal), or a People client
/// cannot be constructed.
pub async fn connect_google_contact_providers(
    tokens: Arc<GraphTokenSource>,
    directory: bool,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let tls = tls_with(&[])?;
    // A dead grant fails here as a sign-in to repeat, rather than on the first pass.
    tokens.access_token().await?;
    let client = Arc::new(account_client(tokens, tls));
    bound_sources(directory)
        .map(|source| {
            bind(source, Arc::clone(&client))
                .map(|provider| Box::new(provider) as Box<dyn ContactsProvider>)
                .map_err(|err| AccountError::Google(err.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use engine_core::{ids::AccountId, sync::SyncScope};
    use engine_provider::{Provider, WriteGuard};
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
            "alice@gmail.com",
            "refresh".to_owned(),
            None,
            "google",
            crate::CredentialOrigin::FreshSignIn,
        )
    }

    fn provider(
        source: GoogleContactSource,
    ) -> ReconnectingContacts<GoogleClient, GoogleContactProvider> {
        bind(
            source,
            Arc::new(account_client(google_source(), tls_with(&[]).unwrap())),
        )
        .unwrap()
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

    /// Colleagues are a choice of their own: without it the account reads its own contacts and
    /// the ones Google collected, and never the organisation's directory.
    #[test]
    fn the_directory_is_bound_only_when_colleagues_were_chosen() {
        assert_eq!(
            bound_sources(false).collect::<Vec<_>>(),
            [
                GoogleContactSource::Connections,
                GoogleContactSource::OtherContacts
            ],
        );
        assert_eq!(bound_sources(true).collect::<Vec<_>>(), BOUND_SOURCES);
    }

    /// Each source keys its cursors under a Google scope of its own: [`ContactsProvider`]'s
    /// defaults are JMAP-shaped, and three sources under one scope would overwrite each other's
    /// cursors.
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

    /// The create affordance is drawn from the destination, which a freshly added account offers
    /// before its first sync.
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

    /// Every source reads contacts and photos, and only the account's own connections take a
    /// write.
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
