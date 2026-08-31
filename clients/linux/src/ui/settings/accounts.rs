//! Per-account credentials, fetch depth, live delivery, watched folders, and removal.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::{AccountProvider, AccountSyncRow, SyncSettingsSnapshot, SyncStrategyKind};

use super::{PageContext, dialog_box, group, page_box};
use crate::{
    l10n,
    ui::{folder_pane::folder_label, mailbox::plain_text_row},
};

pub(super) fn accounts(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_accounts());
    let add = gtk::Button::with_label(l10n::action_add_account());
    add.add_css_class("suggested-action");
    add.set_halign(gtk::Align::Start);
    let sender = ctx.sender.clone();
    add.connect_clicked(move |_| sender.emit(super::super::AppInput::OpenAccountSetup));
    content.append(&add);
    // What the person's other devices have to say, above their own accounts: an offer becomes one
    // of them, and it is drawn even when this device has none; which is exactly when an offer is
    // worth the most.
    if let Some(section) = super::allodia_sync::allodia_sync(ctx) {
        content.append(&section);
    }
    let snapshot = ctx.app.sync_settings();
    let expired = ctx
        .app
        .connectivity()
        .signin_expired_accounts
        .into_iter()
        .collect::<HashSet<_>>();
    if snapshot.accounts.is_empty() {
        let empty = gtk::Label::new(Some(l10n::settings_accounts_empty()));
        empty.set_xalign(0.0);
        content.append(&empty);
        return content;
    }
    for account in &snapshot.accounts {
        // Whether this one travels. First in the group, because it decides whether anything below
        // it is anybody else's business.
        if let Some(status) = ctx
            .allodia_accounts_synced
            .get(&account.account_id)
            .copied()
        {
            content.append(&super::account_sync_mode::synced_group(
                &ctx.sender,
                &account.account_id,
                status,
            ));
        }
        let provider = ctx.app.account_provider(account.account_id.clone());
        content.append(&account_group(
            ctx,
            account,
            &snapshot,
            expired.contains(&account.account_id),
            provider.as_ref(),
        ));
    }
    content
}

fn account_group(
    ctx: &PageContext,
    account: &AccountSyncRow,
    snapshot: &SyncSettingsSnapshot,
    signin_expired: bool,
    provider: Option<&AccountProvider>,
) -> adw::PreferencesGroup {
    let can_replace_secret = signin_expired
        && matches!(
            provider,
            Some(AccountProvider::Password | AccountProvider::Jmap)
        );
    let description = if can_replace_secret {
        if ctx.credential_repair_failed.as_deref() == Some(account.account_id.as_str()) {
            l10n::signin_expired_failed().to_owned()
        } else {
            l10n::signin_expired_prompt(&account.email)
        }
    } else {
        l10n::settings_sync_description().to_owned()
    };
    let section = group(&account.email, &description);
    if can_replace_secret {
        add_secret_remedy(
            &section,
            &account.account_id,
            provider.expect("secret remedy has a provider"),
            &ctx.sender,
        );
    }
    let depth_labels = snapshot
        .sync_depths
        .iter()
        .map(|months| {
            if *months == 0 {
                l10n::sync_depth_all().to_owned()
            } else {
                l10n::sync_depth_months(i64::from(*months))
            }
        })
        .collect::<Vec<_>>();
    let selected = snapshot
        .sync_depths
        .iter()
        .position(|months| *months == account.sync_depth_months)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    let row = adw::ActionRow::builder()
        .title(l10n::settings_sync_depth_heading())
        .subtitle(l10n::settings_sync_depth_description())
        .use_markup(false)
        .build();
    let labels = depth_labels.iter().map(String::as_str).collect::<Vec<_>>();
    let picker = gtk::DropDown::from_strings(&labels);
    picker.set_selected(selected);
    let app = ctx.app.clone();
    let account_id = account.account_id.clone();
    let depths = snapshot.sync_depths.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(months) = depths.get(picker.selected() as usize) {
            app.set_account_sync_depth(account_id.clone(), *months);
        }
    });
    row.add_suffix(&picker);
    section.add(&row);

    // Message size; the largest message kept offline (per-account).
    let size_labels = snapshot
        .message_size_limits_mb
        .iter()
        .map(|megabytes| {
            if *megabytes == 0 {
                l10n::message_size_unlimited().to_owned()
            } else {
                l10n::message_size_megabytes(i64::from(*megabytes))
            }
        })
        .collect::<Vec<_>>();
    let selected = snapshot
        .message_size_limits_mb
        .iter()
        .position(|megabytes| *megabytes == account.message_size_limit_mb)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    // Built with the setters rather than the builder: a translated title or subtitle may hold a
    // bare ampersand, and the builder cannot promise `use-markup` is applied before the text.
    let row = plain_text_row();
    row.set_title(l10n::settings_message_size_heading());
    row.set_subtitle(l10n::settings_message_size_description());
    let labels = size_labels.iter().map(String::as_str).collect::<Vec<_>>();
    let picker = gtk::DropDown::from_strings(&labels);
    picker.set_selected(selected);
    let app = ctx.app.clone();
    let account_id = account.account_id.clone();
    let limits = snapshot.message_size_limits_mb.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(megabytes) = limits.get(picker.selected() as usize) {
            app.set_account_message_size_limit(account_id.clone(), *megabytes);
        }
    });
    row.add_suffix(&picker);
    section.add(&row);

    if account.idle_supported {
        let row = adw::ActionRow::builder()
            .title(l10n::settings_sync_strategy_label())
            .use_markup(false)
            .build();
        let picker = gtk::DropDown::from_strings(&[
            l10n::settings_sync_strategy_push(),
            l10n::settings_sync_strategy_poll(),
        ]);
        picker.set_selected(match account.strategy {
            SyncStrategyKind::Push => 0,
            SyncStrategyKind::Poll => 1,
        });
        let app = ctx.app.clone();
        let account_id = account.account_id.clone();
        picker.connect_selected_notify(move |picker| {
            app.set_sync_strategy(
                account_id.clone(),
                if picker.selected() == 0 {
                    SyncStrategyKind::Push
                } else {
                    SyncStrategyKind::Poll
                },
            );
        });
        row.add_suffix(&picker);
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

    match account.strategy {
        SyncStrategyKind::Poll => add_poll_row(ctx, account, snapshot, &section),
        SyncStrategyKind::Push => add_folder_rows(ctx, account, snapshot, &section),
    }
    add_remove_row(ctx, account, &section);
    section
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

fn add_poll_row(
    ctx: &PageContext,
    account: &AccountSyncRow,
    snapshot: &SyncSettingsSnapshot,
    section: &adw::PreferencesGroup,
) {
    let labels = snapshot
        .poll_intervals
        .iter()
        .map(|minutes| l10n::settings_sync_interval_minutes(i64::from(*minutes)))
        .collect::<Vec<_>>();
    let refs = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let picker = gtk::DropDown::from_strings(&refs);
    let selected = snapshot
        .poll_intervals
        .iter()
        .position(|minutes| *minutes == account.poll_interval_mins)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    picker.set_selected(selected);
    let app = ctx.app.clone();
    let account_id = account.account_id.clone();
    let intervals = snapshot.poll_intervals.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(minutes) = intervals.get(picker.selected() as usize) {
            app.set_poll_interval(account_id.clone(), *minutes);
        }
    });
    let row = adw::ActionRow::builder()
        .title(l10n::settings_sync_interval_label())
        .use_markup(false)
        .build();
    row.add_suffix(&picker);
    section.add(&row);
}

fn add_folder_rows(
    ctx: &PageContext,
    account: &AccountSyncRow,
    snapshot: &SyncSettingsSnapshot,
    section: &adw::PreferencesGroup,
) {
    section.add(
        &adw::ActionRow::builder()
            .title(l10n::settings_sync_folders_heading())
            .subtitle(l10n::settings_sync_folders_note(i64::from(
                snapshot.max_push_folders,
            )))
            .use_markup(false)
            .build(),
    );
    for folder in &account.folders {
        let row = adw::SwitchRow::new();
        // A folder name is the server's text, not ours: `PreferencesRow` titles are Pango markup
        // by default, so "Sales & Marketing" would fail to parse and render blank. A setter,
        // before the title; the property builder applies properties in GObject's order, not the
        // written one, so a builder can set the title while markup is still on.
        row.set_use_markup(false);
        // The same word the pane uses. A folder called two things in one app is worse than one
        // called something odd in both (`docs/folder-pane.md` rule 13).
        row.set_title(&folder_label(folder.role.as_ref(), &folder.name));
        row.set_active(folder.subscribed);
        row.set_sensitive(folder.subscribed || !account.at_push_limit);
        let app = ctx.app.clone();
        let account_id = account.account_id.clone();
        let folder_id = folder.key.clone();
        row.connect_active_notify(move |row| {
            app.set_push_folder(account_id.clone(), folder_id.clone(), row.is_active());
        });
        section.add(&row);
    }
}

fn add_remove_row(ctx: &PageContext, account: &AccountSyncRow, section: &adw::PreferencesGroup) {
    let row = adw::ActionRow::builder()
        .title(l10n::action_remove_account())
        .use_markup(false)
        .build();
    let remove = gtk::Button::with_label(l10n::action_remove());
    remove.add_css_class("destructive-action");
    let parent = ctx.window.clone();
    let sender = ctx.sender.clone();
    let account_id = account.account_id.clone();
    let email = account.email.clone();
    remove.connect_clicked(move |_| {
        confirm_remove(&parent, &email, account_id.clone(), sender.clone());
    });
    row.add_suffix(&remove);
    section.add(&row);
}

fn confirm_remove(
    parent: &gtk::Window,
    email: &str,
    account_id: String,
    sender: relm4::Sender<super::super::AppInput>,
) {
    let (dialog, _) = crate::ui::modal::new(parent, l10n::remove_account_title(), 440, None);
    let content = dialog_box();
    let message = gtk::Label::new(Some(&l10n::remove_account_message(email)));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let remove = gtk::Button::with_label(l10n::action_remove());
    remove.add_css_class("destructive-action");
    let window = dialog.clone();
    remove.connect_clicked(move |_| {
        sender.emit(super::super::AppInput::RemoveAccount(account_id.clone()));
        window.close();
    });
    actions.append(&remove);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.present();
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
