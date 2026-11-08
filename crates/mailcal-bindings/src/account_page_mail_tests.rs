use super::mail_half;
use crate::{
    AccountSyncRow, ControlState, FolderRole, SyncFolderRow, SyncStrategyKind,
    account_work::{Hold, PageWork},
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};
const RUNNING: ControlState = ControlState {
    enabled: false,
    running: true,
};

fn folder(key: &str, subscribed: bool) -> SyncFolderRow {
    SyncFolderRow {
        key: key.to_owned(),
        name: key.to_owned(),
        role: (key == "INBOX").then_some(FolderRole::Inbox),
        subscribed,
    }
}

fn row() -> AccountSyncRow {
    AccountSyncRow {
        account_id: "a".to_owned(),
        email: "alice@example.com".to_owned(),
        sender_name: String::new(),
        sender_name_editable: true,
        idle_supported: true,
        strategy: SyncStrategyKind::Push,
        poll_interval_mins: 30,
        sync_depth_months: 6,
        message_size_limit_mb: 5,
        at_push_limit: false,
        folders: vec![folder("INBOX", true), folder("Lists", false)],
    }
}

fn own(work: PageWork) -> Hold {
    Hold {
        busy: true,
        own: vec![work],
    }
}

#[test]
fn a_name_the_provider_holds_is_shown_and_falls_back_to_the_address() {
    let (name, _) = mail_half(row(), &Hold::default());
    assert_eq!(
        name.name, "",
        "an editable field shows what is set, nothing included"
    );
    assert!(name.editable);
    assert_eq!(name.control, FREE);

    let held = AccountSyncRow {
        sender_name_editable: false,
        ..row()
    };
    let (name, _) = mail_half(held, &Hold::default());
    assert_eq!(
        name.name, "alice@example.com",
        "a line with nothing in it says nothing"
    );
    assert!(!name.editable);
    assert!(
        !name.control.enabled,
        "nothing to press where the provider holds the name"
    );
}

#[test]
fn mail_that_arrives_as_it_comes_lists_its_folders_and_no_interval() {
    let (_, mail) = mail_half(row(), &Hold::default());
    assert_eq!(mail.push.map(|push| push.on), Some(true));
    assert!(mail.interval.is_none());
    assert_eq!(mail.folders.len(), 2);
    assert!(mail.folders[0].on && !mail.folders[1].on);
    assert_eq!(mail.depth.options[mail.depth.selected as usize], 6);
    assert_eq!(mail.size.options[mail.size.selected as usize], 5);
}

#[test]
fn a_server_that_cannot_push_offers_only_the_interval() {
    let polling = AccountSyncRow {
        idle_supported: false,
        strategy: SyncStrategyKind::Poll,
        ..row()
    };
    let (_, mail) = mail_half(polling, &Hold::default());
    assert!(
        mail.push.is_none(),
        "no choice where there is nothing to choose"
    );
    let interval = mail.interval.expect("it checks on a timer");
    assert_eq!(interval.options[interval.selected as usize], 30);
    assert!(mail.folders.is_empty());
}

#[test]
fn at_the_limit_only_a_watched_folder_can_be_pressed() {
    let full = AccountSyncRow {
        at_push_limit: true,
        ..row()
    };
    let (_, mail) = mail_half(full, &Hold::default());
    assert_eq!(
        mail.folders[0].control, FREE,
        "a watched one can always stop"
    );
    assert_eq!(
        mail.folders[1].control, HELD,
        "another would pass the limit"
    );
}

#[test]
fn a_running_change_is_drawn_where_it_goes_and_holds_the_rest() {
    let (_, mail) = mail_half(row(), &own(PageWork::Depth(12)));
    assert_eq!(mail.depth.options[mail.depth.selected as usize], 12);
    assert_eq!(mail.depth.control, RUNNING);
    assert_eq!(mail.size.control, HELD);
    assert!(mail.folders.iter().all(|folder| folder.control == HELD));

    // Switching to a timer shows the interval at once, and the folders go with the push.
    let (_, mail) = mail_half(row(), &own(PageWork::Push(false)));
    assert_eq!(
        mail.push.map(|push| (push.on, push.control)),
        Some((false, RUNNING))
    );
    assert!(mail.interval.is_some());
    assert!(mail.folders.is_empty());

    let (name, mail) = mail_half(
        row(),
        &own(PageWork::Folder {
            key: "Lists".to_owned(),
            on: true,
        }),
    );
    assert!(mail.folders[1].on);
    assert_eq!(mail.folders[1].control, RUNNING);
    assert_eq!(name.control, HELD);

    let (name, _) = mail_half(row(), &own(PageWork::SenderName("Alice".to_owned())));
    assert_eq!((name.name.as_str(), name.control), ("Alice", RUNNING));

    // A change waiting behind the running one is drawn running too, where it goes.
    let queued = Hold {
        busy: true,
        own: vec![
            PageWork::Depth(12),
            PageWork::SenderName("Alice".to_owned()),
        ],
    };
    let (name, mail) = mail_half(row(), &queued);
    assert_eq!((name.name.as_str(), name.control), ("Alice", RUNNING));
    assert_eq!(mail.depth.control, RUNNING);
    assert_eq!(mail.size.control, HELD);
}

#[test]
fn another_pages_change_holds_every_control_without_a_spinner() {
    let elsewhere = Hold {
        busy: true,
        own: Vec::new(),
    };
    let (name, mail) = mail_half(row(), &elsewhere);
    assert_eq!(name.control, HELD);
    assert_eq!(mail.depth.control, HELD);
    assert_eq!(mail.push.map(|push| push.control), Some(HELD));
}

#[test]
fn a_count_no_longer_offered_is_still_the_one_shown() {
    let odd = AccountSyncRow {
        sync_depth_months: 18,
        ..row()
    };
    let (_, mail) = mail_half(odd, &Hold::default());
    assert_eq!(mail.depth.options[mail.depth.selected as usize], 18);
}
