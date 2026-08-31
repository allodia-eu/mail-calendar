//! Which mailbox a move lands in: the RFC 6154 SPECIAL-USE role lookup and the
//! conventional-folder-name fallback behind it.
//!
//! Split out of `mail_ops` (its parent) to stay under the 500-line limit. The fallback is here
//! rather than inline because it is a *table*: the conventional Archive/Trash/Sent/Junk names
//! across the locales the product ships in, and a table grows; keeping it beside the action
//! logic would push that file over the cap every time a locale is added.

use engine_api::{Mailbox, MailboxRole};

/// Resolves the mailbox for a move to `role` (Trash / Archive) or the Sent folder to protect:
/// the RFC 6154 SPECIAL-USE role, else a conventional folder name, else; **for Archive only**
/// : the account's RFC 6154 `\All` mailbox. `None` when none resolves. Shared by the
/// single-message move-to-role and the thread archive so both agree on which folder is
/// Archive/Sent.
///
/// The `\All` fallback exists because **Gmail has no Archive folder in any form**: archiving
/// there is the *absence* of the Inbox label, and the archived message's home is "All Mail"
/// (which the engine's Gmail adapter surfaces as a synthetic `\All` mailbox). Both earlier
/// lookups miss, so without this the move is never even built and archive is a **silent
/// no-op** on every Gmail account. It is deliberately last: a server that tags a real
/// `\Archive` folder, or names one conventionally, still wins: so this can only turn a
/// no-op into a move, never redirect an existing one.
pub(super) fn resolve_move_target<'a>(
    mailboxes: &'a [Mailbox],
    role: &MailboxRole,
) -> Option<&'a Mailbox> {
    mailboxes
        .iter()
        .find(|mailbox| mailbox.role.as_ref() == Some(role))
        .or_else(|| {
            mailboxes
                .iter()
                .find(|mailbox| folder_name_matches_role(&mailbox.name, role))
        })
        .or_else(|| {
            (role == &MailboxRole::Archive)
                .then(|| {
                    mailboxes
                        .iter()
                        .find(|mailbox| mailbox.role.as_ref() == Some(&MailboxRole::All))
                })
                .flatten()
        })
}

/// Whether `name` is a conventional folder name for `role`: the fallback used when a server
/// doesn't advertise the RFC 6154 SPECIAL-USE role (common for Archive). Matches the leaf of a
/// hierarchical name (e.g. `INBOX.Archive`) case-insensitively, across the locales the product
/// ships in. Only the move targets (Trash, Archive, Junk) and Sent (protected by the thread
/// archive) are matched; it's consulted only after the role lookup misses, so it can never
/// override a server-tagged folder.
pub(super) fn folder_name_matches_role(name: &str, role: &MailboxRole) -> bool {
    let leaf = name.rsplit(['/', '.', '\\']).next().unwrap_or(name).trim();
    let candidates: &[&str] = match role {
        MailboxRole::Archive => &[
            "archive",
            "archives",
            "archief",
            "archieven",
            "archiv",
            "archivio",
            "archivo",
            "arkiv",
        ],
        MailboxRole::Trash => &[
            "trash",
            "deleted",
            "deleted items",
            "deleted messages",
            "bin",
            "prullenbak",
            "papierkorb",
            "corbeille",
            "cestino",
            "papelera",
        ],
        MailboxRole::Sent => &[
            "sent",
            "sent items",
            "sent messages",
            "sent mail",
            "verzonden",
            "verzonden items",
            "gesendet",
            "gesendete",
            "envoyés",
            "inviata",
            "enviados",
        ],
        MailboxRole::Junk => &[
            "junk",
            "junk email",
            "spam",
            "ongewenste mail",
            "ongewenste e-mail",
            "spamordner",
            "courrier indésirable",
            "posta indesiderata",
            "correo no deseado",
        ],
        _ => return false,
    };
    candidates
        .iter()
        .any(|candidate| leaf.eq_ignore_ascii_case(candidate))
}
