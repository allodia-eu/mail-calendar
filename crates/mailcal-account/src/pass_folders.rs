//! Which of an account's folders its account pass syncs.

use engine_core::mail::{Mailbox, MailboxRole};

/// Whether the account pass syncs `mailbox`: every folder the account lists except one that holds
/// no mail at all, the two kinds that are only ever views of mail filed in another folder, and the
/// folders a service keeps for its own records.
///
/// A folder that is only a level of the tree (`Mailbox::selectable` false: an IMAP `\Noselect`
/// row such as Gmail's `[Gmail]`) refuses to be opened, so binding it fails on every pass.
///
/// `\Flagged` (RFC 6154) and `\Important` (RFC 8457) are defined as views: Gmail's Starred and
/// Important over IMAP, Dovecot's virtual folders. Syncing one stores every message in it a
/// second time under keys of its own, which the list and search then show twice, and adds nothing
/// the pass does not already have. Opening one still downloads it. `\All` is not excluded:
/// servers put it on a real folder too (an Archive), and that folder is where archiving files
/// mail.
///
/// A folder of the service's own records (`ServerRecords`: Microsoft 365's Sync Issues and the
/// folders under it) holds no mail anyone filed, and the pane draws no row for it
/// (`docs/folder-pane.md`, rule 35). Synced, its messages would still reach the account's All Mail
/// and search, which list everything stored.
#[must_use]
pub fn pass_syncs(mailbox: &Mailbox) -> bool {
    mailbox.selectable
        && !matches!(
            mailbox.role,
            Some(MailboxRole::Flagged | MailboxRole::Important | MailboxRole::ServerRecords)
        )
}

#[cfg(test)]
mod tests {
    use engine_core::ids::MailboxId;

    use super::*;

    fn folder(role: Option<MailboxRole>) -> Mailbox {
        let mut mailbox = Mailbox::new(MailboxId::try_from("folder").unwrap(), "folder");
        mailbox.role = role;
        mailbox
    }

    #[test]
    fn a_view_of_mail_filed_elsewhere_is_left_out_and_every_folder_else_is_synced() {
        assert!(!pass_syncs(&folder(Some(MailboxRole::Flagged))));
        assert!(!pass_syncs(&folder(Some(MailboxRole::Important))));
        assert!(!pass_syncs(&folder(Some(MailboxRole::ServerRecords))));
        for role in [
            None,
            Some(MailboxRole::Inbox),
            Some(MailboxRole::Archive),
            Some(MailboxRole::All),
            Some(MailboxRole::Junk),
            Some(MailboxRole::ChatHistory),
            Some(MailboxRole::Other("custom".to_owned())),
        ] {
            assert!(pass_syncs(&folder(role.clone())), "{role:?}");
        }
    }

    #[test]
    fn a_folder_that_only_holds_folders_is_left_out() {
        let mut container = folder(None);
        container.selectable = false;
        assert!(!pass_syncs(&container));
    }
}
