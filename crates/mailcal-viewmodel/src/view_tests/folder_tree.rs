//! The folder **tree**: which row a folder is drawn under, in what order, and how deep.
//!
//! Split from `folders_and_accounts`, which covers the ordering of one flat level and the
//! account switcher beside it. The rules are `docs/folder-pane.md`.

use super::*;
use crate::folders::folder_paths;

/// A folder filed inside another one, the way every adapter reports it: its own name, and the
/// parent's id.
fn nested(key: &str, name: &str, parent: &str) -> Mailbox {
    let mut mailbox = Mailbox::new(MailboxId::try_from(key).unwrap(), name);
    mailbox.parent = Some(MailboxId::try_from(parent).unwrap());
    mailbox
}

/// A roleless top-level folder.
fn plain(key: &str, name: &str) -> Mailbox {
    Mailbox::new(MailboxId::try_from(key).unwrap(), name)
}

/// `(key, depth, parent)` per row, which is the shape every assertion here is about.
fn shape(rows: &[FolderRow]) -> Vec<(&str, u32, Option<&str>)> {
    rows.iter()
        .map(|row| (row.key.as_str(), row.depth, row.parent.as_deref()))
        .collect()
}

#[test]
fn a_folder_is_drawn_under_the_one_it_is_filed_in_depth_first() {
    // Listed in an order no pane would draw: a grandchild before its parent, a parent after
    // its own sibling. The projection is what puts each folder immediately beneath the folder
    // that holds it.
    let rows = sorted_folder_rows(&[
        nested("Clients/Acme/2024", "2024", "Clients/Acme"),
        plain("Zebra", "Zebra"),
        nested("Clients/Acme", "Acme", "Clients"),
        plain("Clients", "Clients"),
    ]);

    assert_eq!(
        shape(&rows),
        vec![
            ("Clients", 0, None),
            ("Clients/Acme", 1, Some("Clients")),
            ("Clients/Acme/2024", 2, Some("Clients/Acme")),
            ("Zebra", 0, None),
        ]
    );
    // A disclosure control belongs on the two that hold folders, and on neither of the others.
    let holds: Vec<bool> = rows.iter().map(|row| row.has_children).collect();
    assert_eq!(holds, vec![true, true, false, false]);
}

#[test]
fn siblings_keep_the_canonical_order_within_their_own_branch() {
    // The order one level of the pane has always had, applied to each level rather than to
    // the list as a whole: two branches, each sorted among itself and nowhere else.
    let rows = sorted_folder_rows(&[
        plain("B", "B"),
        plain("A", "A"),
        nested("B/zebra", "zebra", "B"),
        nested("A/zebra", "zebra", "A"),
        nested("B/apricot", "apricot", "B"),
        nested("A/apricot", "apricot", "A"),
    ]);

    let keys: Vec<&str> = rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["A", "A/apricot", "A/zebra", "B", "B/apricot", "B/zebra"]
    );
}

#[test]
fn a_role_bearing_folder_is_drawn_at_the_top_however_the_server_files_it() {
    // Gmail files Sent, Drafts and Trash inside a `[Gmail]` container over IMAP. Burying the
    // folder a person reaches for most inside something their provider invented is what no
    // mail client does, and we already give these folders our own name, so we give them their
    // own place too.
    let mut sent = roled("[Gmail]/Sent Mail", "Sent Mail", MailboxRole::Sent);
    sent.parent = Some(MailboxId::try_from("[Gmail]").unwrap());
    let mut all = roled("[Gmail]/All Mail", "All Mail", MailboxRole::All);
    all.parent = Some(MailboxId::try_from("[Gmail]").unwrap());
    let rows = sorted_folder_rows(&[
        plain("[Gmail]", "[Gmail]"),
        sent,
        all,
        roled("INBOX", "Inbox", MailboxRole::Inbox),
        nested("[Gmail]/Spam-ish", "Spam-ish", "[Gmail]"),
    ]);

    assert_eq!(
        shape(&rows),
        vec![
            ("INBOX", 0, None),
            ("[Gmail]/Sent Mail", 0, None),
            // `All` ranks after the six named roles but is still a role, so it is hoisted too.
            ("[Gmail]/All Mail", 0, None),
            // The container stays, holding whatever has no role of its own.
            ("[Gmail]", 0, None),
            ("[Gmail]/Spam-ish", 1, Some("[Gmail]")),
        ]
    );
}

#[test]
fn a_folder_whose_parent_the_account_does_not_list_is_drawn_at_the_top() {
    // An unsubscribed or hidden intermediate: the row has to go somewhere, and it cannot go
    // under a row that is not there. Its own name is all it has, which is the trade for not
    // dropping it.
    let rows = sorted_folder_rows(&[nested("Work/Clients", "Clients", "Work")]);

    assert_eq!(shape(&rows), vec![("Work/Clients", 0, None)]);
}

#[test]
fn every_folder_gets_a_row_even_when_the_parents_form_a_cycle() {
    // No server should be able to report this. If one does, a folder missing from the pane is
    // a folder whose mail the user has no way to open, so each still gets a row.
    let rows = sorted_folder_rows(&[nested("A", "A", "B"), nested("B", "B", "A")]);

    let keys: Vec<&str> = rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"A") && keys.contains(&"B"));
    assert!(rows.iter().all(|row| row.depth == 0));
}

#[test]
fn a_flat_list_names_a_folder_by_the_folders_it_sits_inside() {
    // The sync-settings push list has no indent to say where a folder is, and every adapter now
    // names a folder by its own name alone: two rows reading `2024` would be one word each.
    let rows = sorted_folder_rows(&[
        plain("Clients", "Clients"),
        nested("Clients/Acme", "2024", "Clients"),
        nested("Clients/Beta", "2024", "Clients"),
        plain("Zebra", "Zebra"),
    ]);

    assert_eq!(
        folder_paths(&rows),
        vec![
            "Clients".to_owned(),
            "Clients / 2024".to_owned(),
            "Clients / 2024".to_owned(),
            "Zebra".to_owned(),
        ]
    );
}

#[test]
fn a_path_is_built_from_the_parents_not_from_the_name() {
    // A folder the pane hoisted to the top has no prefix, and neither has an orphan: the walk
    // follows the rows own parents, so it says exactly what the pane draws.
    let mut sent = roled("[Gmail]/Sent Mail", "Sent Mail", MailboxRole::Sent);
    sent.parent = Some(MailboxId::try_from("[Gmail]").unwrap());
    let rows = sorted_folder_rows(&[
        plain("[Gmail]", "[Gmail]"),
        sent,
        nested("Work/Clients", "Clients", "Work"),
    ]);

    let paths = folder_paths(&rows);
    assert!(paths.contains(&"Sent Mail".to_owned()), "{paths:?}");
    assert!(paths.contains(&"Clients".to_owned()), "{paths:?}");
}

#[test]
fn the_projection_leaves_expansion_open_for_the_core_to_stamp() {
    // `App::restamp_expansion` is what fills these in, at publish time, so a rebuild that
    // began before the user's chevron cannot publish the state as it was then. Until then a
    // folder that holds folders reads as open, which is what an untouched one is.
    let rows = sorted_folder_rows(&[
        plain("Clients", "Clients"),
        nested("Clients/Acme", "Acme", "Clients"),
    ]);

    assert!(rows[0].expanded);
    assert!(rows.iter().all(|row| row.visible));
    // Never "expanded" with nothing inside: there would be no tree to open.
    assert!(!rows[1].expanded);
}
