//! Settings → Composing → Default send account, as the core offers it (`send_account_setting`):
//! no account, one address, or a choice over every account used for mail on the one a new
//! message is written from.

use adw::prelude::*;
use mailcal_bindings::{PreferenceIntent, SendAccountOption, SendAccountSetting};

use super::super::{PageContext, choice};
use crate::{l10n, ui::preference};

pub(super) fn row(ctx: &PageContext) -> adw::ActionRow {
    match ctx.app.send_account_setting() {
        SendAccountSetting::NoAccount => plain_row(l10n::settings_accounts_empty(), None),
        SendAccountSetting::Only { email } => {
            plain_row(l10n::settings_send_account_heading(), Some(&email))
        }
        SendAccountSetting::Choose { accounts, selected } => {
            let labels = accounts
                .iter()
                .map(|account| account.email.as_str())
                .collect::<Vec<_>>();
            let (row, picker) = choice(
                l10n::settings_send_account_heading(),
                &labels,
                position(&accounts, &selected),
            );
            let app = ctx.app.clone();
            picker.connect_selected_notify(move |picker| {
                let account = accounts
                    .get(picker.selected() as usize)
                    .map(|account| account.id.clone());
                preference::set(&app, PreferenceIntent::SetDefaultSendAccount { account });
            });
            row
        }
    }
}

fn plain_row(title: &str, subtitle: Option<&str>) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .build();
    if let Some(subtitle) = subtitle {
        row.set_subtitle(subtitle);
    }
    row
}

/// Where `selected` is among `accounts`. The core names one of them, so the first is only what a
/// malformed answer would fall to.
fn position(accounts: &[SendAccountOption], selected: &str) -> u32 {
    accounts
        .iter()
        .position(|account| account.id == selected)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::SendAccountOption;

    use super::position;

    #[test]
    fn the_picker_opens_on_the_account_the_core_names() {
        let accounts = ["work", "home"].map(|id| SendAccountOption {
            id: id.to_owned(),
            email: format!("{id}@test.local"),
        });
        assert_eq!(position(&accounts, "home"), 1);
        assert_eq!(position(&accounts, "work"), 0);
    }
}
