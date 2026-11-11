//! How an account is shared with the person's other devices, as its page's `sharing` says it.
//!
//! Split from [`super::account_page`] to keep that file inside the 500-line limit.

use adw::prelude::*;
use mailcal_bindings::{AccountPageIntent, AccountSharing, AllodiaAccountSyncMode, AllodiaSyncFailure};

use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, controls, mailbox::plain_text_row},
};

const MODES: [AllodiaAccountSyncMode; 3] = [
    AllodiaAccountSyncMode::On,
    AllodiaAccountSyncMode::Paused,
    AllodiaAccountSyncMode::Off,
];

/// The control, one choice of three rather than a switch and a button: the two questions
/// underneath (is this account on my other devices, does this device exchange changes about it)
/// are not independent in any way somebody can act on. The description carries the position in
/// force only: three at once is a paragraph nobody reads.
pub(super) fn synced_group(
    sender: &relm4::Sender<AppInput>,
    account_id: &str,
    sharing: &AccountSharing,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(l10n::settings_account_sync_heading()).as_str())
        .description(gtk::glib::markup_escape_text(sync_mode_hint(sharing.mode)).as_str())
        .build();
    let labels = [
        l10n::settings_account_sync_on(),
        l10n::settings_account_sync_paused(),
        l10n::settings_account_sync_off(),
    ];
    let selected = MODES.iter().position(|mode| *mode == sharing.mode).unwrap_or(0);
    let sender = sender.clone();
    let account = account_id.to_owned();
    group.add(&controls::choice_row(
        l10n::settings_account_sync_heading(),
        &labels,
        selected,
        sharing.control,
        move |position| {
            sender.emit(AppInput::Accounts(AccountsInput::Change(
                AccountPageIntent::SetSharing {
                    account: account.clone(),
                    mode: MODES[position],
                },
            )));
        },
    ));
    if let Some(failed) = sharing.failed {
        group.add(&failed_row(failed));
    }
    group
}

/// Why a change to how an account is shared, or the pass over the other devices, did not take.
pub(super) fn sync_failed(failure: AllodiaSyncFailure) -> &'static str {
    match failure {
        AllodiaSyncFailure::NeedsReauth => l10n::settings_allodia_reauth(),
        AllodiaSyncFailure::SignedOut => l10n::settings_allodia_signed_out_elsewhere(),
        AllodiaSyncFailure::Unavailable => l10n::settings_account_sync_failed(),
    }
}

/// A line saying a change did not take.
pub(super) fn failed_row(failure: AllodiaSyncFailure) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(sync_failed(failure));
    row.add_css_class("error");
    row
}

/// What the selected position means, in one line.
fn sync_mode_hint(mode: AllodiaAccountSyncMode) -> &'static str {
    match mode {
        AllodiaAccountSyncMode::On => l10n::settings_account_sync_on_hint(),
        AllodiaAccountSyncMode::Paused => l10n::settings_account_sync_paused_hint(),
        AllodiaAccountSyncMode::Off => l10n::settings_account_sync_off_hint(),
    }
}

#[cfg(test)]
#[path = "account_sync_mode_tests.rs"]
pub(crate) mod tests;
