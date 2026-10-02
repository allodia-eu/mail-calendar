//! Which of an account's folders its account pass syncs.

use engine_core::mail::{Mailbox, MailboxRole};

/// Whether the account pass syncs `mailbox`: every folder the account lists except the two
/// kinds that are only ever views of mail filed in another folder.
///
/// `\Flagged` (RFC 6154) and `\Important` (RFC 8457) are defined as views: Gmail's Starred and
/// Important over IMAP, Dovecot's virtual folders. Syncing one stores every message in it a
/// second time under keys of its own, which the list and search then show twice, and adds nothing
/// the pass does not already have. Opening one still downloads it. `\All` is not excluded:
/// servers put it on a real folder too (an Archive), and that folder is where archiving files
/// mail.
#[must_use]
pub fn pass_syncs(mailbox: &Mailbox) -> bool {
    !matches!(
        mailbox.role,
        Some(MailboxRole::Flagged | MailboxRole::Important)
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
        for role in [
            None,
            Some(MailboxRole::Inbox),
            Some(MailboxRole::Archive),
            Some(MailboxRole::All),
            Some(MailboxRole::Junk),
            Some(MailboxRole::Other("custom".to_owned())),
        ] {
            assert!(pass_syncs(&folder(role.clone())), "{role:?}");
        }
    }
}
