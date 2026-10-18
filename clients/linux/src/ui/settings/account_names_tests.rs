//! Naming an account where another has the same address.

use mailcal_bindings::{
    AccountCapability, AccountDistinction, AccountEntry, AccountKind, AccountLinksView, AccountUse,
    CapabilityState, LinkCandidates, LinkedAccount,
};

use super::{kind_line, linked_name};
use crate::{
    l10n,
    ui::settings::{account_rows::summary, link_pickers},
};

fn linked(distinction: Option<AccountDistinction>) -> LinkedAccount {
    LinkedAccount {
        id: "alice@example.org@dav:dav.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        distinction,
    }
}

fn mailbox(distinction: Option<AccountDistinction>) -> AccountEntry {
    AccountEntry {
        id: "alice@example.org@imap.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        kind: AccountKind::Imap,
        sign_in: mailcal_bindings::SignInMethod::Password,
        uses: vec![AccountUse {
            capability: AccountCapability::Mail,
            state: CapabilityState::On,
            problem: None,
        }],
        links: AccountLinksView::default(),
        linked_from: Vec::new(),
        link_candidates: LinkCandidates::default(),
        endpoints: None,
        separate_server: None,
        distinction,
    }
}

#[test]
fn an_account_no_other_shares_the_address_of_is_named_by_its_address() {
    assert_eq!(linked_name(&linked(None)), "alice@example.org");
}

#[test]
fn a_kind_names_the_account_by_the_label_its_own_row_uses() {
    let kind = Some(AccountDistinction::Kind {
        kind: AccountKind::Dav,
    });
    assert_eq!(
        linked_name(&linked(kind)),
        l10n::settings_account_distinguished("alice@example.org", l10n::account_kind_dav())
    );
}

#[test]
fn a_server_names_the_account_by_its_host() {
    let server = Some(AccountDistinction::Server {
        host: "dav.example.org:8443".to_owned(),
    });
    assert_eq!(
        linked_name(&linked(server)),
        l10n::settings_account_distinguished("alice@example.org", "dav.example.org:8443")
    );
}

#[test]
fn the_kind_line_gains_the_host_for_a_server_only() {
    let imap = l10n::setup_account_type_password();
    assert_eq!(kind_line(&mailbox(None)), imap);
    let kind = Some(AccountDistinction::Kind {
        kind: AccountKind::Imap,
    });
    assert_eq!(kind_line(&mailbox(kind)), imap);

    let server = mailbox(Some(AccountDistinction::Server {
        host: "mail.example.org".to_owned(),
    }));
    assert_eq!(kind_line(&server), format!("{imap} · mail.example.org"));
    assert_eq!(
        summary(&server).lines().next(),
        Some(format!("{imap} · mail.example.org · {}", l10n::nav_mail()).as_str())
    );
}

#[test]
fn a_link_line_names_the_linked_account_apart_from_this_one() {
    let mut entry = mailbox(Some(AccountDistinction::Kind {
        kind: AccountKind::Imap,
    }));
    entry.links.calendar = Some(linked(Some(AccountDistinction::Kind {
        kind: AccountKind::Dav,
    })));
    let named = l10n::settings_account_distinguished("alice@example.org", l10n::account_kind_dav());
    assert_eq!(
        summary(&entry).lines().nth(1),
        Some(l10n::settings_account_linked_line(l10n::nav_calendar(), &named).as_str())
    );
}

#[test]
fn a_picker_names_every_option_and_the_suggestion_apart() {
    let mut entry = mailbox(None);
    let dav = linked(Some(AccountDistinction::Kind {
        kind: AccountKind::Dav,
    }));
    entry.link_candidates.calendar = vec![dav.clone()];
    entry.link_candidates.suggested = vec![dav.id];
    let pickers = link_pickers(&entry);
    let named = l10n::settings_account_distinguished("alice@example.org", l10n::account_kind_dav());
    assert_eq!(
        pickers[0].labels(),
        vec![l10n::settings_account_link_none().to_owned(), named.clone()]
    );
    assert_eq!(pickers[0].suggested_name(), Some(named));
}
