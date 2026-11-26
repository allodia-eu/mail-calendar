use engine_core::sync::SyncScope;
use engine_provider::{Provider, WriteGuard};

use super::{
    super::{
        client_connecting,
        token_source::test_support::{mock_token_endpoint, source_at},
    },
    *,
};
use crate::http_fake;

fn account() -> AccountId {
    AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap()
}

fn book(id: &str) -> AddressBookId {
    AddressBookId::try_from(id).unwrap()
}

/// The account's client, over a token source that never touches the network.
fn client() -> Arc<SharedClient<GraphClient>> {
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    account_client(
        source_at(endpoint, None),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
    )
}

/// The id the engine gives the default contacts folder, read from the adapter rather than
/// restated, so a rename there cannot leave these tests asserting on a stale literal.
fn root_book() -> AddressBookId {
    let (_, current) = client().current().unwrap();
    GraphContactProvider::personal(current)
        .contact_destination()
        .unwrap()
        .address_book
}

fn provider(source: GraphContactSource) -> ReconnectingContacts<GraphClient, GraphContactProvider> {
    bind(source, client()).unwrap()
}

/// The default folder comes first because the first destination is the one a client
/// preselects, and the listing it is discovered in is sorted by id, not by importance.
#[test]
fn binds_the_default_folder_first_then_the_others_then_the_directory() {
    let root = root_book();
    let sources = bound_sources(
        &root,
        [book("folder-b"), root.clone(), book("folder-a")],
        true,
    );
    assert_eq!(
        sources,
        vec![
            GraphContactSource::Personal(root),
            GraphContactSource::Personal(book("folder-b")),
            GraphContactSource::Personal(book("folder-a")),
            GraphContactSource::Directory,
        ],
    );
}

/// Organisational contacts need `OrgContact.Read.All`, which the app does not request, so a
/// bound provider would spend a request on a refusal every pass.
#[test]
fn never_binds_organisational_contacts() {
    let root = root_book();
    assert!(
        !bound_sources(&root, [book("folder-a")], true)
            .contains(&GraphContactSource::Organizational)
    );
}

/// Colleagues are a choice of their own: without it the account reads its own folders and never
/// the tenant directory, which is the permission a strict tenant is most likely to refuse.
#[test]
fn binds_the_directory_only_when_colleagues_were_chosen() {
    let root = root_book();
    assert_eq!(
        bound_sources(&root, [book("folder-a")], false),
        vec![
            GraphContactSource::Personal(root),
            GraphContactSource::Personal(book("folder-a")),
        ],
    );
}

/// The wrapper must forward both scopes rather than let them fall through to the trait's
/// JMAP-shaped defaults, which would key every source's cursor to one scope.
#[test]
fn each_source_keys_its_cursors_under_its_own_graph_scope() {
    let account = account();
    assert_eq!(
        provider(GraphContactSource::Personal(book("folder-a"))).contact_scope(&account),
        SyncScope::GraphContacts {
            account: account.clone(),
            address_book: book("folder-a"),
        },
    );
    assert_eq!(
        provider(GraphContactSource::Directory).contact_scope(&account),
        SyncScope::GraphDirectoryUsers {
            account: account.clone(),
        },
    );
    assert_eq!(
        provider(GraphContactSource::Directory).address_book_scope(&account),
        SyncScope::GraphContactFolderList { account },
    );
}

/// The create affordance is drawn from the destination, so a wrapper that answered `None`
/// until the first sync would hide it on a freshly added account.
#[test]
fn only_personal_folders_offer_a_write_destination() {
    let destination = provider(GraphContactSource::Personal(book("folder-a")))
        .contact_destination()
        .expect("a personal folder accepts a create");
    assert!(destination.writable);
    assert_eq!(destination.address_book, book("folder-a"));
    assert_eq!(destination.write_guard, Some(WriteGuard::Absent));
    assert!(
        provider(GraphContactSource::Directory)
            .contact_destination()
            .is_none()
    );
}

#[test]
fn every_source_reports_contacts_and_photos_but_only_personal_folders_report_writes() {
    for source in [
        GraphContactSource::Personal(book("folder-a")),
        GraphContactSource::Directory,
    ] {
        let capabilities = provider(source.clone()).connection_info().capabilities;
        assert!(capabilities.contacts(), "{source:?} reads contacts");
        assert!(capabilities.contact_photos(), "{source:?} reads photos");
        assert_eq!(
            capabilities.contact_writes(),
            matches!(source, GraphContactSource::Personal(_)),
            "{source:?} writes",
        );
    }
}

/// Each request asks the account's source for the token, and one Graph refused is replaced once
/// and the request sent again, on the same client.
#[tokio::test]
async fn each_request_presents_the_current_token_and_a_refused_one_is_replaced() {
    let (endpoint, minted) = mock_token_endpoint(vec![http_fake::minted(1), http_fake::minted(2)]);
    let (base, received) = http_fake::serve(http_fake::refused_then(r#"{"value":[]}"#));
    let client = client_connecting(
        source_at(endpoint, None),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GraphClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );
    let provider = bind(GraphContactSource::Personal(root_book()), client).unwrap();

    provider.sync_address_books(&account(), None).await.unwrap();
    provider.sync_address_books(&account(), None).await.unwrap();

    http_fake::assert_replaced_once(&received, &minted);
}
