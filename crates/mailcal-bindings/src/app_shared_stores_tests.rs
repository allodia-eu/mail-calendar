//! What Settings draws of an account's shared stores, from the server's answer and the account's
//! choice.

use engine_api::{SearchDomain, SharedAccount, StoreDomains};

use super::*;

fn mail() -> SharedUses {
    SharedUses::of(SharedUse::Mail)
}

fn listed(handle: &str, address: &str) -> SharedAccount {
    listed_holding(handle, address, StoreDomains::of(SearchDomain::Mail))
}

fn listed_holding(handle: &str, address: &str, holds: StoreDomains) -> SharedAccount {
    SharedAccount::new(SharedAccountId::try_from(handle).unwrap(), address, holds)
        .with_address(address)
}

fn chosen(handle: &str, address: &str) -> SharedStoreChoice {
    SharedStoreChoice {
        handle: SharedAccountId::try_from(handle).unwrap(),
        name: address.to_owned(),
        address: Some(address.to_owned()),
        holds: mail(),
    }
}

fn handles(view: &SharedStoresView) -> Vec<(&str, bool, bool)> {
    view.stores
        .iter()
        .map(|entry| (entry.handle.as_str(), entry.selected, entry.listed))
        .collect()
}

#[test]
fn the_chosen_mailboxes_come_first_and_the_rest_the_server_listed_after() {
    let view = view::<()>(
        Ok(SharedStoreOffer::Listed {
            stores: vec![listed("a", "a@example.org"), listed("b", "b@example.org")],
            unanswered: mailcal_account::SharedUses::default(),
        }),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(view.support, SharedStoreSupport::Listed);
    assert_eq!(handles(&view), [("b", true, true), ("a", false, true)]);
}

#[test]
fn a_chosen_mailbox_the_server_stopped_listing_is_kept_and_says_so() {
    let view = view::<()>(
        Ok(SharedStoreOffer::Listed {
            stores: Vec::new(),
            unanswered: mailcal_account::SharedUses::default(),
        }),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(handles(&view), [("b", true, false)]);
}

#[test]
fn an_account_not_yet_connected_keeps_its_choice_without_calling_it_withdrawn() {
    let view = view::<()>(
        Ok(SharedStoreOffer::NotConnected),
        vec![chosen("b", "b@example.org")],
    );
    assert_eq!(view.support, SharedStoreSupport::NotConnected);
    assert_eq!(handles(&view), [("b", true, true)]);
}

#[test]
fn a_server_that_did_not_answer_still_shows_what_the_account_is_used_for() {
    let view = view(Err("timed out"), vec![chosen("b", "b@example.org")]);
    assert_eq!(view.support, SharedStoreSupport::Unreachable);
    assert_eq!(handles(&view), [("b", true, true)]);
}

#[test]
fn each_store_says_what_it_holds() {
    let group = mail().with(SharedUse::Calendar);
    let calendar = StoreDomains::of(SearchDomain::Calendar);
    let view = view::<()>(
        Ok(SharedStoreOffer::Listed {
            stores: vec![
                listed_holding("a", "a@example.org", calendar.with(SearchDomain::Mail)),
                listed_holding("c", "c@example.org", calendar),
            ],
            unanswered: mailcal_account::SharedUses::default(),
        }),
        vec![chosen("b", "b@example.org")],
    );
    let holds: Vec<_> = view.stores.iter().map(|store| store.holds).collect();
    assert_eq!(holds, [mail(), group, SharedUses::of(SharedUse::Calendar)]);
}

#[test]
fn a_chosen_store_the_unanswered_server_holds_is_not_called_withdrawn() {
    // The mailbox listed nothing chosen; the calendar server did not answer.
    let calendars = SharedStoreChoice {
        holds: SharedUses::of(SharedUse::Calendar),
        ..chosen("c", "c@example.org")
    };
    let view = view::<()>(
        Ok(SharedStoreOffer::Listed {
            stores: vec![listed("a", "a@example.org")],
            unanswered: SharedUses::of(SharedUse::Calendar),
        }),
        vec![chosen("b", "b@example.org"), calendars],
    );
    assert_eq!(view.support, SharedStoreSupport::Listed);
    assert_eq!(
        handles(&view),
        [("b", true, false), ("c", true, true), ("a", false, true)]
    );
}

#[test]
fn a_chosen_store_shows_what_the_account_opens_of_it_now() {
    let group = SharedStoreChoice {
        holds: mail().with(SharedUse::Calendar),
        ..chosen("g", "g@example.org")
    };
    let calendars = SharedStoreChoice {
        holds: SharedUses::of(SharedUse::Calendar),
        ..chosen("c", "c@example.org")
    };
    let holds: Vec<_> = shown(vec![group, calendars], mail())
        .into_iter()
        .map(|choice| choice.holds)
        .collect();
    // The calendar alone, which the account no longer opens, keeps its row to be switched off.
    assert_eq!(holds, [mail(), SharedUses::of(SharedUse::Calendar)]);
}

#[test]
fn an_account_used_for_calendars_alone_opens_shared_calendars_alone() {
    let server = crate::nextcloud_mock::mock_nextcloud(Vec::new());
    let calendar = format!(
        "[caldav]\nbase_url = \"{}/dav/cal/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = crate::nextcloud_mock::boot("shared-calendar-only", vec![calendar]);
    let entry = app.accounts_snapshot().accounts.remove(0);
    assert_eq!(
        app.shared_uses(&entry.id),
        SharedUses::of(SharedUse::Calendar)
    );
}
