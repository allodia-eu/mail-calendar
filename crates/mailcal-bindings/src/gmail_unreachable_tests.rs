use mailcal_account::{AccountLinks, Capability, GmailMailRoute as Route};

use super::with_unreachable_gmail;
use crate::{
    AccountKind, ConnectivitySnapshot,
    accounts_view::{
        AccountFacts,
        tests::{mailbox, set},
    },
};

fn snapshot(offline: bool, expired: &[&str]) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        offline,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: expired.iter().map(|id| (*id).to_owned()).collect(),
        calendar_outages: Vec::new(),
        contacts_outages: Vec::new(),
        allodia_sign_in_ended: None,
        allodia_lapses_soon: false,
    }
}

fn gmail(id: &str, uses: &[Capability]) -> AccountFacts {
    AccountFacts {
        id: id.to_owned(),
        address: format!("{id}@gmail.com"),
        kind: AccountKind::Google,
        chosen: set(uses),
        server: None,
        ..mailbox(AccountLinks::default())
    }
}

#[test]
fn a_gmail_account_whose_mail_cannot_be_reached_is_badged_like_any_unreachable_account() {
    let facts = [
        gmail("blocked", &[Capability::Mail]),
        gmail("fine", &[Capability::Mail]),
        gmail("calendar-only", &[Capability::Calendar]),
    ];
    let route = |address: &str| {
        if address.starts_with("fine") {
            Route::Imap
        } else {
            Route::Unreachable
        }
    };
    let badged = with_unreachable_gmail(snapshot(false, &[]), &facts, route);
    assert_eq!(badged.unreachable_accounts, ["blocked"]);
}

#[test]
fn offline_or_an_expired_sign_in_says_it_instead() {
    let facts = [gmail("blocked", &[Capability::Mail])];
    let unreachable = |_: &str| Route::Unreachable;
    let offline = with_unreachable_gmail(snapshot(true, &[]), &facts, unreachable);
    assert!(offline.unreachable_accounts.is_empty());
    let expired = with_unreachable_gmail(snapshot(false, &["blocked"]), &facts, unreachable);
    assert!(expired.unreachable_accounts.is_empty());

    // An account the pass already found unreachable is listed once.
    let mut already = snapshot(false, &[]);
    already.unreachable_accounts.push("blocked".to_owned());
    let once = with_unreachable_gmail(already, &facts, unreachable);
    assert_eq!(once.unreachable_accounts, ["blocked"]);
}
