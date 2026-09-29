use engine_provider::WriteGuard;

use super::{
    super::token_source::test_support::{mock_token_endpoint, source_at},
    *,
};

fn account() -> AccountId {
    AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap()
}

fn book(id: &str) -> AddressBookId {
    AddressBookId::try_from(id).unwrap()
}

/// The id the engine gives the default contacts folder, read from the adapter rather than
/// restated, so a rename there cannot leave these tests asserting on a stale literal.
fn root_book() -> AddressBookId {
    let tls = tls_with(&[]).unwrap();
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let retry = source_at(endpoint, None).retry();
    let client = GraphClient::for_mailbox(
        "access-token".to_owned(),
        MailboxPrincipal::Me,
        &tls,
        &retry,
    )
    .unwrap();
    GraphContactProvider::personal(client)
        .contact_destination()
        .unwrap()
        .address_book
}

fn provider(source: GraphContactSource) -> RefreshingGraphContactProvider {
    let tls = tls_with(&[]).unwrap();
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let tokens = source_at(endpoint, None);
    let delegate = Arc::new(build(&source, "access-token", &tls, &tokens.retry()).unwrap());
    RefreshingGraphContactProvider::new(source, tokens, tls, ("access-token".to_owned(), delegate))
}

/// The default folder comes first because the first destination is the one a client
/// preselects, and the listing it is discovered in is sorted by id, not by importance.
#[test]
fn binds_the_default_folder_first_then_the_others_then_the_directory() {
    let root = root_book();
    let sources = bound_sources(&root, [book("folder-b"), root.clone(), book("folder-a")]);
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
        !bound_sources(&root, [book("folder-a")]).contains(&GraphContactSource::Organizational)
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
