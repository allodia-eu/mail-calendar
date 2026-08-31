//! Settings → Advanced controls for local AI-assistant access (`docs/mcp.md`).

use adw::prelude::*;
use mailcal_bindings::McpSettings;

use super::{PageContext, group};
use crate::{l10n, ui::mcp as endpoint};

pub(super) fn section(ctx: &PageContext) -> Option<gtk::Box> {
    let settings = ctx.app.mcp_settings();
    let socket = settings.endpoint.as_deref()?;
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let master = group(
        l10n::settings_mcp_heading(),
        l10n::settings_mcp_description(),
    );
    let toggle = adw::SwitchRow::builder()
        .title(l10n::settings_mcp_toggle())
        .active(settings.enabled)
        .use_markup(false)
        .build();
    toggle.set_widget_name("mcp-enabled-toggle");
    master.add(&toggle);
    let status = adw::ActionRow::builder()
        .title(status_text(&settings))
        .use_markup(false)
        .build();
    status.set_widget_name("mcp-status");
    master.add(&status);
    content.append(&master);

    let details = gtk::Box::new(gtk::Orientation::Vertical, 18);
    details.append(&accounts(ctx, &settings));
    details.append(&send(ctx, &settings));
    details.append(&configuration(socket));
    let disclosure = gtk::Revealer::builder()
        .reveal_child(settings.enabled)
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .child(&details)
        .build();
    content.append(&disclosure);

    let app = ctx.app.clone();
    let status = status.downgrade();
    let disclosure = disclosure.downgrade();
    toggle.connect_active_notify(move |row| {
        app.set_mcp_enabled(row.is_active());
        let current = app.mcp_settings();
        if let Some(status) = status.upgrade() {
            status.set_title(status_text(&current));
        }
        if let Some(disclosure) = disclosure.upgrade() {
            disclosure.set_reveal_child(current.enabled);
        }
    });
    Some(content)
}

fn status_text(settings: &McpSettings) -> &'static str {
    if !settings.enabled {
        l10n::settings_mcp_status_off()
    } else if settings.running {
        l10n::settings_mcp_status_running()
    } else {
        l10n::settings_mcp_status_unavailable()
    }
}

fn accounts(ctx: &PageContext, settings: &McpSettings) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_mcp_accounts_heading(),
        l10n::settings_mcp_accounts_description(),
    );
    let empty = adw::ActionRow::builder()
        .title(l10n::settings_mcp_accounts_empty())
        .use_markup(false)
        .visible(!settings.accounts.iter().any(|account| account.exposed))
        .build();
    empty.set_widget_name("mcp-accounts-empty");
    for account in &settings.accounts {
        let row = adw::SwitchRow::builder()
            .title(&account.email)
            .active(account.exposed)
            .use_markup(false)
            .build();
        let app = ctx.app.clone();
        let account_id = account.account_id.clone();
        let empty = empty.downgrade();
        row.connect_active_notify(move |row| {
            app.set_mcp_account_exposed(account_id.clone(), row.is_active());
            if let Some(empty) = empty.upgrade() {
                empty.set_visible(
                    !app.mcp_settings()
                        .accounts
                        .iter()
                        .any(|account| account.exposed),
                );
            }
        });
        section.add(&row);
    }
    if settings.accounts.is_empty() {
        section.add(
            &adw::ActionRow::builder()
                .title(l10n::settings_accounts_empty())
                .use_markup(false)
                .build(),
        );
    }
    section.add(&empty);
    section
}

fn send(ctx: &PageContext, settings: &McpSettings) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_mcp_send_heading(),
        l10n::settings_mcp_send_note(),
    );
    let direct = adw::SwitchRow::builder()
        .title(l10n::settings_mcp_send_toggle())
        .active(settings.allow_direct_send)
        .use_markup(false)
        .build();
    direct.set_widget_name("mcp-direct-send-toggle");
    let known = adw::SwitchRow::builder()
        .title(l10n::settings_mcp_known_recipient_toggle())
        .subtitle(l10n::settings_mcp_known_recipient_note())
        .active(settings.require_known_recipient)
        .sensitive(settings.allow_direct_send)
        .use_markup(false)
        .build();
    known.set_widget_name("mcp-known-recipient-toggle");
    let app = ctx.app.clone();
    let known_row = known.downgrade();
    direct.connect_active_notify(move |row| {
        app.set_mcp_allow_direct_send(row.is_active());
        if let Some(known) = known_row.upgrade() {
            known.set_sensitive(row.is_active());
        }
    });
    let app = ctx.app.clone();
    known.connect_active_notify(move |row| {
        app.set_mcp_require_known_recipient(row.is_active());
    });
    section.add(&direct);
    section.add(&known);
    section
}

fn configuration(socket: &str) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_mcp_config_heading(),
        l10n::settings_mcp_config_description(),
    );
    let snippet = endpoint::configuration_snippet(socket);
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(&snippet);
    let text = gtk::TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .build();
    text.set_widget_name("mcp-config-snippet");
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(150)
        .max_content_height(220)
        .propagate_natural_height(true)
        .child(&text)
        .build();
    section.add(&scroll);
    let copy = gtk::Button::with_label(l10n::settings_mcp_copy());
    copy.set_widget_name("mcp-copy-config");
    let copy_row = adw::ActionRow::new();
    copy_row.add_suffix(&copy);
    copy.connect_clicked(move |button| {
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&snippet);
            button.set_label(l10n::settings_mcp_copied());
            let button = button.downgrade();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                if let Some(button) = button.upgrade() {
                    button.set_label(l10n::settings_mcp_copy());
                }
            });
        }
    });
    section.add(&copy_row);
    section
}
