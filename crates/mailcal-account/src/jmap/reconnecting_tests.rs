//! A JMAP account's provider after a dead connection: a read is made once more on a provider
//! connected again, a write is made once, and the provider connected again keeps the contact book
//! and the shared mailbox the first was bound to.

use engine_core::{
    ids::{AccountId, SharedAccountId},
    vacation::VacationResponse,
};
use engine_provider::{MailboxEdit, VacationPrecondition};

use crate::{
    jmap::{
        JmapAccountConfig,
        fake_server::{BOOK, FakeJmap, SHARED, account_id},
    },
    load_jmap_str,
};

/// An account that stores a password, at `base_url`.
fn account(base_url: &str) -> JmapAccountConfig {
    load_jmap_str(&format!(
        "[jmap]\nemail = \"you@example.net\"\nbase_url = \"{base_url}\"\npassword = \"hunter2\"\n"
    ))
    .unwrap()
}

fn any_account() -> AccountId {
    AccountId::try_from("you@example.net").unwrap()
}

#[tokio::test]
async fn a_read_on_a_dead_connection_is_made_again_on_a_provider_connected_again() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_mail_providers(&account(server.url()), None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };

    server.hang_up_next_call();
    provider.sync_mailboxes(&any_account(), None).await.unwrap();

    assert_eq!(server.discoveries(), 2, "the session fetched anew");
    assert_eq!(server.calls(), 2, "the read made again");
}

/// A folder created before the connection died would be created twice by a repeat, so the write
/// fails and the next request goes out on a provider connected again.
#[tokio::test]
async fn a_write_on_a_dead_connection_is_made_once_and_the_next_request_connects_again() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_mail_providers(&account(server.url()), None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    let folder = MailboxEdit::Create {
        name: "Receipts".to_owned(),
        parent: None,
    };

    server.hang_up_next_call();
    provider
        .edit_mailbox(&any_account(), &folder)
        .await
        .unwrap_err();
    assert_eq!(server.calls(), 1, "made once");
    assert_eq!(server.discoveries(), 1);

    provider.sync_mailboxes(&any_account(), None).await.unwrap();
    assert_eq!(server.discoveries(), 2, "the next request connected again");
}

#[tokio::test]
async fn the_contact_book_stays_bound_on_a_provider_connected_again() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_contact_providers(&account(server.url()), None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one contacts provider");
    };
    let discovered = server.discoveries();

    server.hang_up_next_call();
    provider
        .sync_address_books(&any_account(), None)
        .await
        .unwrap();

    assert_eq!(server.discoveries(), discovered + 1, "connected again");
    let destination = provider.contact_destination().expect("a write destination");
    assert_eq!(destination.address_book.as_str(), BOOK);
}

#[tokio::test]
async fn a_shared_mailbox_stays_bound_on_a_provider_connected_again() {
    let server = FakeJmap::start();
    let shared = SharedAccountId::try_from(SHARED).unwrap();
    let provider =
        crate::connect_jmap_calendar_provider(&account(server.url()), None, Some(&shared))
            .await
            .unwrap();

    server.hang_up_next_call();
    provider.sync_calendars(&any_account(), None).await.unwrap();

    assert_eq!(server.discoveries(), 2, "connected again");
    let calls: Vec<String> = server.seen().iter().filter_map(account_id).collect();
    assert_eq!(
        calls,
        [SHARED, SHARED],
        "the call hung up on and its repeat"
    );
}

#[tokio::test]
async fn a_reply_read_on_a_dead_connection_is_made_again_on_a_provider_connected_again() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_mail_providers(&account(server.url()), None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    let reply = provider.vacation().expect("the session offers a reply");

    server.hang_up_next_call();
    let read = reply.read_vacation(&any_account()).await.unwrap();

    assert!(!read.enabled);
    assert_eq!(server.discoveries(), 2, "connected again");
    assert_eq!(server.calls(), 2, "the read made again");
}

/// A reply written before the connection died would be written again by a repeat over whatever
/// changed since, so the write is made once, and the next request goes out on a provider
/// connected again.
#[tokio::test]
async fn a_reply_written_on_a_dead_connection_is_written_once() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_mail_providers(&account(server.url()), None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    let reply = provider.vacation().expect("the session offers a reply");

    server.hang_up_next_call();
    reply
        .set_vacation(
            &any_account(),
            VacationPrecondition::Unconditional,
            &VacationResponse::default(),
        )
        .await
        .unwrap_err();

    assert_eq!(server.calls(), 1, "made once");
    assert_eq!(server.discoveries(), 1);

    reply.read_vacation(&any_account()).await.unwrap();
    assert_eq!(server.discoveries(), 2, "the next request connected again");
}

#[tokio::test]
async fn the_rules_are_read_again_and_replaced_once_on_a_provider_connected_again() {
    let server = FakeJmap::start();
    let providers = crate::connect_jmap_mail_providers(&account(server.url()), None, None)
        .await
        .unwrap();
    let [provider] = providers.as_slice() else {
        panic!("one mail provider");
    };
    let rules = provider
        .mail_rules()
        .expect("the session offers mail rules");
    rules.rule_vocabulary(&any_account()).await.unwrap();

    server.hang_up_next_call();
    let base = rules.read_rules(&any_account()).await.unwrap();
    assert_eq!(server.discoveries(), 2, "connected again");
    assert_eq!(server.calls(), 2, "the read made again");

    server.hang_up_next_call();
    rules
        .replace_rules(&any_account(), &base, &[])
        .await
        .unwrap_err();
    assert_eq!(server.calls(), 3, "made once");
    assert_eq!(server.discoveries(), 2);
    rules.rule_vocabulary(&any_account()).await.unwrap();
    assert_eq!(server.discoveries(), 3, "the next request connected again");
}
