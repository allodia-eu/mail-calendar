//! What Settings draws of an account's shared mailboxes, from the server's answer and the
//! account's choice.

use engine_api::{SearchDomain, SharedAccount, StoreDomains};

use super::*;

fn listed(handle: &str, address: &str) -> SharedAccount {
    SharedAccount::new(
        SharedAccountId::try_from(handle).unwrap(),
        address,
        StoreDomains::of(SearchDomain::Mail),
    )
    .with_address(address)
}

fn chosen(handle: &str, address: &str) -> SharedMailboxChoice {
    SharedMailboxChoice {
        handle: SharedAccountId::try_from(handle).unwrap(),
        name: address.to_owned(),
        address: Some(address.to_owned()),
    }
}

fn handles(view: &SharedMailboxesView) -> Vec<(&str, bool, bool)> {
    view.mailboxes
        .iter()
        .map(|entry| (entry.handle.as_str(), entry.selected, entry.listed))
        .collect()
}

#[test]
fn the_chosen_mailboxes_come_first_and_the_rest_the_server_listed_after() {
    let view = view::<()>(
        Ok(SharedMailboxOffer::Listed(vec![
            listed("a", "a@example.org"),
            listed("b", "b@example.org"),
        ])),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(view.support, SharedMailboxSupport::Listed);
    assert_eq!(handles(&view), [("b", true, true), ("a", false, true)]);
}

#[test]
fn a_chosen_mailbox_the_server_stopped_listing_is_kept_and_says_so() {
    let view = view::<()>(
        Ok(SharedMailboxOffer::Listed(Vec::new())),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(handles(&view), [("b", true, false)]);
}

#[test]
fn an_account_not_yet_connected_keeps_its_choice_without_calling_it_withdrawn() {
    let view = view::<()>(
        Ok(SharedMailboxOffer::NotConnected),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(view.support, SharedMailboxSupport::NotConnected);
    assert_eq!(handles(&view), [("b", true, true)]);
}

#[test]
fn a_server_that_did_not_answer_still_shows_what_the_account_is_used_for() {
    let view = view(Err("timed out"), vec![chosen("b", "b@example.org")]);
    assert_eq!(view.support, SharedMailboxSupport::Unreachable);
    assert_eq!(handles(&view), [("b", true, true)]);
}

#[test]
fn an_account_not_used_for_mail_opens_no_shared_mailbox() {
    let server = crate::nextcloud_mock::mock_nextcloud(Vec::new());
    let calendar = format!(
        "[caldav]\nbase_url = \"{}/dav/cal/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = crate::nextcloud_mock::boot("shared-calendar-only", vec![calendar]);
    let entry = app.accounts_snapshot().accounts.remove(0);
    let view = app.shared_mailboxes(entry.id).unwrap();
    assert_eq!(view.support, SharedMailboxSupport::Unsupported);
    assert!(view.mailboxes.is_empty());
}
