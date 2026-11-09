//! What happens to an invitation answer the account's calendar server cannot pass on: asked each
//! time with a notice, emailed, or not (`docs/invitations.md`). On the page of an account used for
//! its calendar, where the core sends the picker.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountPageIntent, AccountReplyFallback, ReplyFallbackChoice,
};

use super::PageContext;
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, controls},
};

/// The choices in the order the picker lists them.
const CHOICES: [ReplyFallbackChoice; 3] = [
    ReplyFallbackChoice::Ask,
    ReplyFallbackChoice::Always,
    ReplyFallbackChoice::Never,
];

pub(super) fn group(
    ctx: &PageContext,
    entry: &AccountEntry,
    picker: AccountReplyFallback,
) -> adw::PreferencesGroup {
    let section = super::group(
        l10n::settings_reply_fallback_heading(),
        l10n::settings_reply_fallback_description(),
    );
    let labels = [
        l10n::settings_reply_fallback_ask(),
        l10n::settings_reply_fallback_always(),
        l10n::settings_reply_fallback_never(),
    ];
    let (sender, account) = (ctx.sender.clone(), entry.id.clone());
    let (row, _) = controls::picker_row(
        l10n::settings_reply_fallback_label(),
        &labels,
        position(picker.choice),
        picker.control,
        move |at| {
            let Some(&choice) = CHOICES.get(at as usize) else {
                return;
            };
            sender.emit(AppInput::Accounts(AccountsInput::Change(
                AccountPageIntent::SetReplyFallback {
                    account: account.clone(),
                    choice,
                },
            )));
        },
    );
    section.add(&row);
    section
}

/// Where `choice` sits in the picker.
fn position(choice: ReplyFallbackChoice) -> u32 {
    CHOICES
        .iter()
        .position(|listed| *listed == choice)
        .and_then(|at| u32::try_from(at).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::ReplyFallbackChoice;

    use super::{CHOICES, position};

    #[test]
    fn each_choice_is_drawn_where_its_words_are() {
        assert_eq!(position(ReplyFallbackChoice::Ask), 0);
        assert_eq!(position(ReplyFallbackChoice::Always), 1);
        assert_eq!(position(ReplyFallbackChoice::Never), 2);
        for (at, choice) in CHOICES.iter().enumerate() {
            assert_eq!(position(*choice) as usize, at);
        }
    }
}
