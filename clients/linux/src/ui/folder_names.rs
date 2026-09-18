//! What a folder, and the scope above the message list, are **called** on screen.
//!
//! Split from [`super::folder_pane`], which draws the pane: these two are the naming half, and
//! three other places name a folder too (the list header, the search scope filter, the account
//! settings dialog). One function every site calls is what stops them disagreeing
//! (`docs/folder-pane.md`, rule 13).

use mailcal_bindings::{FolderRole, MailboxListSnapshot};

use super::folder_pane::folders_of;
use crate::l10n;

/// What a folder is **called** on screen: the app's own word for a role-bearing folder, the
/// server's name for everything else.
///
/// The server's name for a special folder is not a name the user chose; it is whatever their
/// provider stores, in whatever language and casing it likes: `INBOX` shouting in capitals (the
/// one name IMAP mandates), `Deleted Items` from Exchange, `[Gmail]/Sent Mail`. Naming them
/// ourselves is also what makes the folder list follow the **app's** language.
///
/// [`FolderRole::Other`] keeps the server's name deliberately: the core collapses flagged,
/// important and all-mail into that one value, so any single word for it would rename three
/// different folders to the same thing.
///
/// Public to the UI module because the pane is not the only place a folder is named; the list
/// header and the sync-settings folder list show one too, and a folder called two things in one
/// app is worse than one called something odd in both.
pub(crate) fn folder_label(role: Option<&FolderRole>, name: &str) -> String {
    match role {
        Some(FolderRole::Inbox) => l10n::folder_inbox().to_owned(),
        Some(FolderRole::Drafts) => l10n::folder_drafts().to_owned(),
        Some(FolderRole::Sent) => l10n::folder_sent().to_owned(),
        Some(FolderRole::Archive) => l10n::folder_archive().to_owned(),
        Some(FolderRole::Junk) => l10n::folder_junk().to_owned(),
        Some(FolderRole::Trash) => l10n::folder_trash().to_owned(),
        Some(FolderRole::Other) | None => name.to_owned(),
    }
}

/// What the mail list's header calls the scope on screen: the unified inbox, the selected folder
/// by the app's own name for it, or the account's whole mailbox.
///
/// Here rather than in the shell because the pane is not the only place a folder is named, and one
/// function every site calls is what stops the header and the tree disagreeing (rule 13).
pub(crate) fn header_title(snapshot: &MailboxListSnapshot) -> String {
    if snapshot.showing_outbox {
        return l10n::folder_outbox().to_owned();
    }
    let Some(account) = snapshot.selected_account.as_deref() else {
        return l10n::folder_inbox().to_owned();
    };
    let Some(key) = snapshot.selected.as_deref() else {
        return l10n::sidebar_all_mail().to_owned();
    };
    folders_of(snapshot, account)
        .iter()
        .find(|folder| folder.key == key)
        .map_or_else(
            // A key with no row behind it: the folder list has moved on (a rename, a sync) and
            // the header would otherwise name a folder that is no longer there.
            || l10n::folder_fallback().to_owned(),
            |folder| folder_label(folder.role.as_ref(), &folder.name),
        )
}

#[cfg(test)]
#[path = "folder_names_tests.rs"]
mod tests;
