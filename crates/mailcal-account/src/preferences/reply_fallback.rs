//! The per-account answer to "your calendar server could not tell the organiser; shall we
//! email them instead?", and the accessors that read and record it.
//!
//! Its own module because it is one small, self-contained decision with a sharper consequence
//! than the settings it sits beside: [`ReplyFallback::Always`] is **standing permission to send
//! mail as the user**. Everything that grants, reads, or revokes it is therefore in one file,
//! rather than three methods scattered among the display and sync preferences.
//!
//! The behaviour it drives lives in the product core (`invitations_fallback`); see
//! `docs/invitations.md` for the contract.

use serde::{Deserialize, Serialize};

use super::Preferences;

/// What to do when a calendar server that promised to tell the organiser reports that it could
/// not; whether this app may send the reply as an email itself.
///
/// **Per account**, because it is a fact about one provider's server rather than a preference
/// about invitations: the same user may have one mailbox whose server schedules perfectly and
/// another that has never delivered a reply.
///
/// [`Ask`](Self::Ask) is the default and the only state that raises a prompt. The other two are
/// what "remember my choice" writes, and they exist so a user on a server that fails **every**
/// reply (the case this setting was added for) is asked once rather than at every meeting.
///
/// Deliberately not a `bool`: *unasked* and *asked, said no* must stay distinguishable, or
/// declining once would read as never having been offered, and the prompt would come back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyFallback {
    /// Ask each time. The default, and what an account absent from the map means.
    #[default]
    Ask,
    /// Send the reply ourselves whenever the server reports it could not.
    Always,
    /// Never send it ourselves. The card still reports that the organiser was not told; the
    /// choice is about sending mail, not about hiding a failure.
    Never,
}

impl Preferences {
    /// What `account` has decided about sending invitation replies itself;
    /// [`ReplyFallback::Ask`] for an account nobody has answered the prompt for.
    #[must_use]
    pub fn reply_fallback(&self, account: &str) -> ReplyFallback {
        self.invitation_reply_fallback
            .get(account)
            .copied()
            .unwrap_or_default()
    }

    /// Records `account`'s standing answer. Setting it back to [`ReplyFallback::Ask`] drops the
    /// entry rather than writing the default out, so the file does not accumulate a row per
    /// account that has merely been asked once.
    pub fn set_reply_fallback(&mut self, account: &str, choice: ReplyFallback) {
        if choice == ReplyFallback::Ask {
            self.invitation_reply_fallback.remove(account);
        } else {
            self.invitation_reply_fallback
                .insert(account.to_owned(), choice);
        }
    }

    /// Drops the reply-fallback choice for an account; used when the account is removed, so a
    /// later re-add asks again rather than silently inheriting a standing permission to send
    /// mail on the user's behalf. Returns whether anything was stored for it.
    pub fn remove_reply_fallback(&mut self, account: &str) -> bool {
        self.invitation_reply_fallback.remove(account).is_some()
    }
}
