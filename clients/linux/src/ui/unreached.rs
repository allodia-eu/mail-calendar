//! The calendar and contacts a sign-in does not reach (`docs/accounts.md` rule 18): which of
//! them a set names, and the question a switch to the provider asks before the browser opens.
//! The sign-in of their own that setup and the account's page offer is
//! [`super::separate_sign_in`].

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, UnreachedUses};

use crate::l10n;

const KEEP: &str = "keep";
const STOP: &str = "stop";
const CANCEL: &str = "cancel";

/// Whether `capability` is one a sign-in of its own can take over.
const fn moves(capability: AccountCapability) -> bool {
    matches!(
        capability,
        AccountCapability::Calendar | AccountCapability::Contacts
    )
}

/// Whether a sign-in that leaves `unreached` behind asks first.
pub(super) fn asks(unreached: &[AccountCapability]) -> bool {
    unreached.iter().any(|use_| moves(*use_))
}

/// Whether `uses` names the calendar and whether it names contacts.
fn names(uses: &[AccountCapability]) -> (bool, bool) {
    (
        uses.contains(&AccountCapability::Calendar),
        uses.contains(&AccountCapability::Contacts),
    )
}

/// The switch's question, for what it leaves behind.
pub(super) fn switch_title(unreached: &[AccountCapability]) -> &'static str {
    match names(unreached) {
        (true, false) => l10n::signin_unreached_title_calendar(),
        (false, true) => l10n::signin_unreached_title_contacts(),
        _ => l10n::signin_unreached_title_both(),
    }
}

/// The title of the form that signs `uses` in on their own server.
pub(super) fn separate_title(uses: &[AccountCapability]) -> &'static str {
    match names(uses) {
        (true, false) => l10n::separate_server_title_calendar(),
        (false, true) => l10n::separate_server_title_contacts(),
        _ => l10n::separate_server_title_both(),
    }
}

/// What the person answered the switch's question with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    Keep,
    Stop,
    Cancel,
}

impl Answer {
    fn from_response(response: &str) -> Self {
        match response {
            KEEP => Self::Keep,
            STOP => Self::Stop,
            _ => Self::Cancel,
        }
    }

    /// What the core is told, or `None` when the switch is abandoned.
    pub(super) const fn choice(self) -> Option<UnreachedUses> {
        match self {
            Self::Keep => Some(UnreachedUses::KeepOnPassword),
            Self::Stop => Some(UnreachedUses::StopUsing),
            Self::Cancel => None,
        }
    }
}

/// The question a switch to the provider asks when it would leave `unreached` behind. Keeping
/// them is the default; stopping deletes what the device holds of them, so it reads as
/// destructive; Escape cancels.
pub(super) fn question(unreached: &[AccountCapability]) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(
        Some(switch_title(unreached)),
        Some(l10n::signin_unreached_body()),
    );
    dialog.add_response(CANCEL, l10n::action_cancel());
    dialog.add_response(STOP, l10n::signin_unreached_stop());
    dialog.add_response(KEEP, l10n::signin_unreached_keep());
    dialog.set_response_appearance(STOP, adw::ResponseAppearance::Destructive);
    dialog.set_response_appearance(KEEP, adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some(KEEP));
    dialog.set_close_response(CANCEL);
    dialog
}

/// Asks the question over `parent`, and hands the answer to `answered`.
pub(super) fn ask(
    parent: &impl IsA<gtk::Widget>,
    unreached: &[AccountCapability],
    answered: impl FnOnce(Answer) + 'static,
) {
    let asked = question(unreached).choose_future(Some(parent));
    gtk::glib::MainContext::default().spawn_local(async move {
        answered(Answer::from_response(asked.await.as_str()));
    });
}

#[cfg(test)]
#[path = "unreached_tests.rs"]
pub(crate) mod tests;
