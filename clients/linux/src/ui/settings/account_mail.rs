//! An account's mail settings, on its page: credentials, fetch depth, message size, live delivery
//! and watched folders, as the core's page record says them. Drawn only while the account is used
//! for mail.

use adw::prelude::*;
use mailcal_bindings::{AccountMailSettings, AccountPageIntent, AccountProvider};

use super::{PageContext, account_page::change, group};
use crate::{
    l10n,
    ui::{controls, folder_names::folder_label},
};

pub(super) fn mail_group(
    ctx: &PageContext,
    account: &str,
    mail: &AccountMailSettings,
    name: &str,
    signin_expired: bool,
    provider: Option<&AccountProvider>,
) -> adw::PreferencesGroup {
    let can_replace_secret = signin_expired
        && matches!(
            provider,
            Some(AccountProvider::Password | AccountProvider::Jmap)
        );
    let description = if can_replace_secret {
        if ctx.credential_repair_failed.as_deref() == Some(account) {
            l10n::signin_expired_failed().to_owned()
        } else {
            l10n::signin_expired_prompt(name)
        }
    } else {
        l10n::settings_sync_description().to_owned()
    };
    let section = group(l10n::nav_mail(), &description);
    if can_replace_secret {
        add_secret_remedy(
            &section,
            account,
            provider.expect("secret remedy has a provider"),
            &ctx.sender,
        );
    }
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

fn add_secret_remedy(
    section: &adw::PreferencesGroup,
    account_id: &str,
    provider: &AccountProvider,
    sender: &relm4::Sender<super::super::AppInput>,
) {
    section.add(&secret_remedy_row(account_id, provider, sender));
}

fn secret_remedy_row(
    account_id: &str,
    provider: &AccountProvider,
    sender: &relm4::Sender<super::super::AppInput>,
) -> adw::ActionRow {
    let label = match provider {
        AccountProvider::Jmap => l10n::setup_jmap_secret_placeholder(),
        _ => l10n::setup_field_password(),
    };
    let row = adw::ActionRow::builder()
        .title(label)
        .use_markup(false)
        .build();
    let secret = gtk::PasswordEntry::new();
    secret.set_placeholder_text(Some(label));
    secret.set_show_peek_icon(true);
    secret.set_valign(gtk::Align::Center);
    secret.set_width_chars(24);
    let save = gtk::Button::with_label(l10n::action_save());
    save.add_css_class("suggested-action");
    save.set_valign(gtk::Align::Center);
    save.set_sensitive(false);
    let button = save.clone();
    secret.connect_changed(move |entry| button.set_sensitive(!entry.text().trim().is_empty()));
    let input = sender.clone();
    let account = account_id.to_owned();
    let entered = secret.clone();
    save.connect_clicked(move |button| {
        let value = entered.text().to_string();
        if value.trim().is_empty() {
            return;
        }
        button.set_sensitive(false);
        entered.set_sensitive(false);
        input.emit(super::super::AppInput::ReplaceAccountSecret {
            account: account.clone(),
            secret: value,
        });
    });
    row.add_suffix(&secret);
    row.add_suffix(&save);
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

#[cfg(test)]
pub(crate) mod tests {
    use adw::prelude::*;

    use super::secret_remedy_row;
    use crate::ui::{AppInput, setup_widget_tests::descendants};

    pub(crate) fn an_expired_password_is_replaced_without_removing_the_account() {
        let (sender, receiver) = relm4::channel();
        let row = secret_remedy_row(
            "account-id",
            &mailcal_bindings::AccountProvider::Password,
            &sender,
        );
        let entry = descendants::<gtk::PasswordEntry>(row.upcast_ref())
            .into_iter()
            .next()
            .expect("password entry");
        let save = descendants::<gtk::Button>(row.upcast_ref())
            .into_iter()
            .find(|button| button.label().as_deref() == Some(crate::l10n::action_save()))
            .expect("save button");

        assert!(!save.is_sensitive());
        entry.set_text("replacement-secret");
        assert!(save.is_sensitive());
        save.emit_clicked();

        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::ReplaceAccountSecret { account, secret })
                if account == "account-id" && secret == "replacement-secret"
        ));
        assert!(!entry.is_sensitive());
    }
}
