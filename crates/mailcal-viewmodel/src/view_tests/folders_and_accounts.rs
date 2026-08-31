//! The sidebar decorations `build` carries alongside the rows: the account switcher and the
//! canonical folder ordering (special roles first, then custom folders by name).

use super::*;

#[test]
fn build_carries_the_account_switcher() {
    let accounts = vec![
        account("work", "me@work.example"),
        account("home", "me@home.example"),
    ];
    let snapshot = build(&[], &[], &accounts, vec![], None, None, ViewMode::Flat, ALL);
    assert_eq!(snapshot.accounts, accounts);
    assert!(snapshot.selected_account.is_none()); // all inboxes
}

#[test]
fn expansion_rides_on_the_account_row_and_is_independent_of_selection() {
    // The whole point of the contract: which trees are open is not "whichever account is
    // selected". A shut account stays shut while it IS the selected one, and an open one
    // stays open while the unified view is showing.
    let accounts = vec![
        AccountRow {
            id: "work".to_owned(),
            email: "me@work.example".to_owned(),
            expanded: false,
        },
        account("home", "me@home.example"),
    ];
    let snapshot = build(
        &[],
        &[],
        &accounts,
        vec![],
        Some("work"),
        None,
        ViewMode::Flat,
        ALL,
    );
    assert!(!snapshot.accounts[0].expanded);
    assert!(snapshot.accounts[1].expanded);
    assert_eq!(snapshot.selected_account.as_deref(), Some("work"));
}

#[test]
fn folders_lead_with_special_roles_in_canonical_order_then_custom_by_name() {
    // The provider lists folders in an arbitrary order, mixing special (role) and custom
    // ones. The sidebar must always lead with the special folders in the fixed canonical
    // order (Inbox, Drafts, Sent, Archive, Junk, Trash), then every other top-level
    // folder by name; identical on every platform.
    let folders = vec![
        Mailbox::new(MailboxId::try_from("Zebra").unwrap(), "Zebra"),
        roled("Trash", "Trash", MailboxRole::Trash),
        Mailbox::new(MailboxId::try_from("apricot").unwrap(), "apricot"),
        roled("Archive", "Archive", MailboxRole::Archive),
        roled("INBOX", "Inbox", MailboxRole::Inbox),
        // An unrecognized SPECIAL-USE role is a custom folder, not a special one.
        roled("Notes", "Notes", MailboxRole::Other("notes".into())),
        roled("Sent", "Sent", MailboxRole::Sent),
    ];
    let snapshot = build(
        &[],
        &folders,
        &[],
        vec![],
        Some("a"),
        None,
        ViewMode::Flat,
        ALL,
    );
    let names: Vec<&str> = snapshot.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            // Special folders, canonical order (Drafts/Junk absent here, so skipped).
            "Inbox", "Sent", "Archive", "Trash",
            // Custom folders, case-insensitive by name (Notes is an `Other` role).
            "apricot", "Notes", "Zebra",
        ]
    );
}

#[test]
fn a_folder_carries_the_servers_count_and_an_uncounted_one_shows_nothing() {
    let folders = vec![
        counted("INBOX", "Inbox", MailboxRole::Inbox, 545),
        counted("Trash", "Trash", MailboxRole::Trash, 4),
        // Counted and empty: a real zero, which renders the same as no badge.
        counted("Sent", "Sent", MailboxRole::Sent, 0),
        // Never counted (Gmail's label list, or an IMAP folder the server refused):
        // collapses to 0, because the client hides the badge either way.
        roled("Archive", "Archive", MailboxRole::Archive),
    ];
    let snapshot = build(
        &[],
        &folders,
        &[],
        vec![],
        Some("a"),
        None,
        ViewMode::Flat,
        ALL,
    );
    let counts: Vec<(&str, u32)> = snapshot
        .folders
        .iter()
        .map(|folder| (folder.name.as_str(), folder.unread))
        .collect();
    assert_eq!(
        counts,
        vec![("Inbox", 545), ("Sent", 0), ("Archive", 0), ("Trash", 4)]
    );
}

#[test]
fn all_inboxes_sums_every_accounts_inbox_and_nothing_else() {
    // Junk and Archive are deliberately loud here: summing every folder would put a number
    // above the unified row that counts mail the unified list will never show.
    let account_folders = vec![
        AccountFolderRow {
            account_id: "work".to_owned(),
            folders: sorted_folder_rows(&[
                counted("INBOX", "Inbox", MailboxRole::Inbox, 545),
                counted("Junk", "Junk", MailboxRole::Junk, 72),
            ]),
        },
        AccountFolderRow {
            account_id: "home".to_owned(),
            folders: sorted_folder_rows(&[
                counted("INBOX", "Inbox", MailboxRole::Inbox, 3),
                counted("Archive", "Archive", MailboxRole::Archive, 900),
            ]),
        },
        // An account whose provider reports no counts at all contributes nothing.
        AccountFolderRow {
            account_id: "gmail".to_owned(),
            folders: sorted_folder_rows(&[roled("INBOX", "Inbox", MailboxRole::Inbox)]),
        },
    ];
    let snapshot = build(
        &[],
        &[],
        &[],
        account_folders,
        None,
        None,
        ViewMode::Flat,
        ALL,
    );
    assert_eq!(snapshot.unified_unread, 548);
}

#[test]
fn the_folder_tree_is_carried_in_every_view_including_one_accounts_own() {
    // The regression this exists for: the pane used to be fed from `folders`, which only ever
    // held the SELECTED account's: so selecting an account emptied every other account's
    // tree, and All Inboxes emptied them all.
    let account_folders = vec![
        AccountFolderRow {
            account_id: "work".to_owned(),
            folders: sorted_folder_rows(&[counted("INBOX", "Inbox", MailboxRole::Inbox, 2)]),
        },
        AccountFolderRow {
            account_id: "home".to_owned(),
            folders: sorted_folder_rows(&[counted("INBOX", "Inbox", MailboxRole::Inbox, 1)]),
        },
    ];
    for selected in [None, Some("work")] {
        let snapshot = build(
            &[],
            &[],
            &[],
            account_folders.clone(),
            selected,
            None,
            ViewMode::Flat,
            ALL,
        );
        assert_eq!(snapshot.account_folders.len(), 2, "selected: {selected:?}");
        assert_eq!(snapshot.unified_unread, 3, "selected: {selected:?}");
    }
}
