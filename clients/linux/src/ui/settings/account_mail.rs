//! An account's mail settings, on its page: fetch depth, message size, live delivery
//! and watched folders, as the core's page record says them. Drawn only while the account is used
//! for mail.

use adw::prelude::*;
use mailcal_bindings::{AccountMailSettings, AccountPageIntent};

use super::{PageContext, account_page::change, group};
use crate::{
    l10n,
    ui::{controls, folder_names::folder_label},
};

pub(super) fn mail_group(
    ctx: &PageContext,
    account: &str,
    mail: &AccountMailSettings,
) -> adw::PreferencesGroup {
    let section = group(l10n::nav_mail(), l10n::settings_sync_description());
    let sender = &ctx.sender;

    let labels: Vec<String> = mail
        .depth
        .options
        .iter()
        .map(|months| match months {
            0 => l10n::sync_depth_all().to_owned(),
            months => l10n::sync_depth_months(i64::from(*months)),
        })
        .collect();
    let row = pick(
        l10n::settings_sync_depth_heading(),
        &labels,
        &mail.depth,
        sender,
        account,
        |account, months| AccountPageIntent::SetSyncDepth { account, months },
    );
    row.set_subtitle(l10n::settings_sync_depth_description());
    section.add(&row);

    // Message size: the largest message kept offline.
    let labels: Vec<String> = mail
        .size
        .options
        .iter()
        .map(|megabytes| match megabytes {
            0 => l10n::message_size_unlimited().to_owned(),
            megabytes => l10n::message_size_megabytes(i64::from(*megabytes)),
        })
        .collect();
    let row = pick(
        l10n::settings_message_size_heading(),
        &labels,
        &mail.size,
        sender,
        account,
        |account, megabytes| AccountPageIntent::SetMessageSizeLimit { account, megabytes },
    );
    row.set_subtitle(l10n::settings_message_size_description());
    section.add(&row);

    if let Some(push) = mail.push {
        let (sender, account) = (sender.clone(), account.to_owned());
        let (row, _) = controls::picker_row(
            l10n::settings_sync_strategy_label(),
            &[
                l10n::settings_sync_strategy_push(),
                l10n::settings_sync_strategy_poll(),
            ],
            u32::from(!push.on),
            push.control,
            move |at| {
                change(
                    &sender,
                    AccountPageIntent::SetPush {
                        account: account.clone(),
                        on: at == 0,
                    },
                );
            },
        );
        section.add(&row);
    } else {
        section.add(
            &adw::ActionRow::builder()
                .title(l10n::settings_sync_strategy_poll())
                .subtitle(l10n::settings_sync_idle_unsupported())
                .use_markup(false)
                .build(),
        );
    }
    if let Some(interval) = &mail.interval {
        let labels: Vec<String> = interval
            .options
            .iter()
            .map(|minutes| l10n::settings_sync_interval_minutes(i64::from(*minutes)))
            .collect();
        section.add(&pick(
            l10n::settings_sync_interval_label(),
            &labels,
            interval,
            sender,
            account,
            |account, minutes| AccountPageIntent::SetPollInterval { account, minutes },
        ));
    }
    if mail.push.is_some_and(|push| push.on) {
        add_folder_rows(sender, account, mail, &section);
    }
    section
}

/// A picker row over `choice`'s counts, worded as `labels`, asking for the count picked.
fn pick(
    title: &str,
    labels: &[String],
    choice: &mailcal_bindings::AccountCountChoice,
    sender: &relm4::Sender<super::super::AppInput>,
    account: &str,
    intent: impl Fn(String, u16) -> AccountPageIntent + 'static,
) -> adw::ActionRow {
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let (sender, account, options) = (sender.clone(), account.to_owned(), choice.options.clone());
    let (row, _) =
        controls::picker_row(title, &labels, choice.selected, choice.control, move |at| {
            if let Some(count) = options.get(at as usize) {
                change(&sender, intent(account.clone(), *count));
            }
        });
    row
}

fn add_folder_rows(
    sender: &relm4::Sender<super::super::AppInput>,
    account: &str,
    mail: &AccountMailSettings,
    section: &adw::PreferencesGroup,
) {
    section.add(
        &adw::ActionRow::builder()
            .title(l10n::settings_sync_folders_heading())
            .subtitle(l10n::settings_sync_folders_note(i64::from(
                mail.watch_limit,
            )))
            .use_markup(false)
            .build(),
    );
    for folder in &mail.folders {
        let (sender, account, key) = (sender.clone(), account.to_owned(), folder.key.clone());
        // The same word the pane uses. A folder called two things in one app is worse than one
        // called something odd in both (`docs/folder-pane.md` rule 13).
        section.add(&controls::switch_row(
            &folder_label(folder.role.as_ref(), &folder.name),
            None,
            folder.on,
            folder.control,
            move |_, on| {
                change(
                    &sender,
                    AccountPageIntent::SetWatchedFolder {
                        account: account.clone(),
                        key: key.clone(),
                        on,
                    },
                );
            },
        ));
    }
}
