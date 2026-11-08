use super::{AccountWork, PageWork, Start};
use crate::{AccountChangeNotice, AccountPageIntent};

fn name(account: &str, name: &str) -> AccountPageIntent {
    AccountPageIntent::SetSenderName {
        account: account.to_owned(),
        name: name.to_owned(),
    }
}

fn depth(account: &str, months: u16) -> AccountPageIntent {
    AccountPageIntent::SetSyncDepth {
        account: account.to_owned(),
        months,
    }
}

#[test]
fn a_change_asked_for_while_another_runs_waits_and_runs_next() {
    let work = AccountWork::default();
    assert!(matches!(work.start(depth("a", 6)), Start::Run(_)));
    assert!(matches!(work.start(name("a", "Alice")), Start::Waits));

    let (hold, _) = work.of("a");
    assert_eq!(
        hold.own,
        vec![PageWork::Depth(6), PageWork::SenderName("Alice".to_owned())],
        "both are drawn running, in the order they run"
    );
    assert!(work.of("b").0.busy && work.of("b").0.own.is_empty());

    let next = work
        .finish("a", None)
        .expect("the waiting change runs next");
    assert_eq!(next, name("a", "Alice"));
    assert_eq!(
        work.of("a").0.own,
        vec![PageWork::SenderName("Alice".to_owned())]
    );
    assert_eq!(work.finish("a", None), None);
    assert!(
        !work.of("a").0.busy,
        "the page is free once the last one ends"
    );
}

#[test]
fn the_same_work_asked_for_again_is_dropped() {
    let work = AccountWork::default();
    assert!(matches!(work.start(name("a", "Alice")), Start::Run(_)));
    assert!(
        matches!(work.start(name("a", "Alice")), Start::Dropped),
        "Return and then the focus leaving send the same name twice"
    );
    assert!(matches!(work.start(depth("a", 6)), Start::Waits));
    assert!(matches!(work.start(depth("a", 6)), Start::Dropped));
    assert!(
        matches!(work.start(name("b", "Alice")), Start::Waits),
        "the same name on another account is other work"
    );
}

#[test]
fn a_notice_raised_before_the_next_change_starts_stays_to_be_said() {
    let work = AccountWork::default();
    let _ = work.start(depth("a", 6));
    let _ = work.start(name("a", "Alice"));
    let notice = AccountChangeNotice::RemoveFailed {
        detail: "keychain".to_owned(),
    };
    assert!(work.finish("a", Some(notice)).is_some());
    assert!(work.of("a").1.is_some());
}

#[test]
fn a_removal_drops_what_waited_for_the_account_it_removed() {
    let work = AccountWork::default();
    let _ = work.start(AccountPageIntent::Remove {
        account: "a".to_owned(),
    });
    let _ = work.start(name("a", "Alice"));
    let _ = work.start(name("b", "Bob"));
    assert_eq!(work.finish("a", None), Some(name("b", "Bob")));
}

#[test]
fn a_change_starting_keeps_the_edit_the_last_save_refused() {
    let work = AccountWork::default();
    let refused = crate::AccountEndpoints {
        imap_host: Some("imap.example.com".to_owned()),
        imap_security: crate::ConnectionSecurity::ImplicitTls,
        smtp_host: Some("smtp.example.com".to_owned()),
        smtp_security: crate::ConnectionSecurity::StartTls,
        caldav_url: None,
        carddav_url: None,
        username: "alice".to_owned(),
        password: Some("typed".to_owned()),
    };
    work.servers_saved("a", Some(refused.clone()));
    let _ = work.start(depth("a", 6));
    assert!(
        work.refused_servers("a") == Some(refused),
        "the form goes on being drawn as it was typed, password included"
    );
}

#[test]
fn a_change_starting_keeps_what_the_server_said_of_its_shared_mailboxes() {
    let work = AccountWork::default();
    let asked = crate::account_page::shared::Asked {
        uses: mailcal_account::Capabilities::default(),
        live: true,
    };
    let ticket = work.shared.ask("a", &asked, false).expect("never asked");
    work.shared
        .answered("a", ticket, Ok(mailcal_app::SharedMailboxOffer::ByAddress));
    let _ = work.start(AccountPageIntent::SetSharedMailbox {
        account: "a".to_owned(),
        handle: "Shared Folders/support".to_owned(),
        on: true,
    });
    assert!(
        work.shared.offer("a").is_some(),
        "the switch's change finds the mailbox it was drawn from, and the section stays drawn"
    );
}
