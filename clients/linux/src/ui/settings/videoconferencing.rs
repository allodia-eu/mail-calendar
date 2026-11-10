//! Settings → Videoconferencing (`docs/settings.md`, category 4): the accounts that make video
//! meetings, the person's Jitsi server (`docs/conferencing.md`, rule 13) and each account's
//! default video meeting (rule 10).
//!
//! An account's row only opens its page, where the one switch for its meetings is: two switches
//! for one setting would be two places to change it, and two places to ask a provider for its
//! permission.
//!
//! The Jitsi server is empty until the person fills it, and checked by the core before it is
//! stored, so an address it refuses is said here and nothing changes. Once one is stored, every
//! calendar whose provider writes a meeting offers Jitsi in the editor.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{CapabilityState, MeetingAccount, MeetingDefault};

use super::{PageContext, account_names::linked_name, group, page_box};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, calendar::editor_meeting::choice_label},
};

pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_videoconferencing());
    let accounts = ctx.app.accounts_snapshot().meeting_accounts;
    if let Some(section) = meeting_accounts_group(&accounts, &ctx.sender) {
        content.append(&section);
    }
    // Drawn again after the Jitsi server is stored: every calendar that offers Jitsi then does,
    // or no longer does.
    let defaults = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let draw_defaults = {
        let app = ctx.app.clone();
        let defaults = defaults.clone();
        move || {
            if let Some(drawn) = defaults.first_child() {
                defaults.remove(&drawn);
            }
            let store = app.clone();
            let section = default_meeting_group(&app.meeting_defaults(), move |account, choice| {
                store.set_default_meeting(account, choice);
            });
            defaults.set_visible(section.is_some());
            if let Some(section) = section {
                defaults.append(&section);
            }
        }
    };
    draw_defaults();
    let read = ctx.app.clone();
    let write = ctx.app.clone();
    content.append(&jitsi_server_group(
        move || read.jitsi_server(),
        move |typed| {
            let stored = write.set_jitsi_server(Some(typed));
            if stored {
                draw_defaults();
            }
            stored
        },
    ));
    content.append(&defaults);
    content
}

/// An account's Video meetings use, as its row says it.
pub(super) fn state_word(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::On => l10n::settings_toggle_on(),
        CapabilityState::Off => l10n::settings_toggle_off(),
        CapabilityState::NeedsPermission => l10n::settings_meeting_account_needs_permission(),
        CapabilityState::NotOffered => l10n::settings_meeting_account_not_offered(),
    }
}

/// The accounts that make video meetings, each opening its page; none while no account does.
pub(super) fn meeting_accounts_group(
    accounts: &[MeetingAccount],
    sender: &relm4::Sender<AppInput>,
) -> Option<adw::PreferencesGroup> {
    if accounts.is_empty() {
        return None;
    }
    let section = group(
        l10n::settings_meeting_accounts(),
        l10n::settings_meeting_accounts_hint(),
    );
    for account in accounts {
        let row = adw::ActionRow::new();
        // A service's and an account's names are not ours to mark up.
        row.set_use_markup(false);
        row.set_title(&account.service);
        row.set_subtitle(&linked_name(&account.account));
        let state = gtk::Label::new(Some(state_word(account.state)));
        state.add_css_class("dim-label");
        row.add_suffix(&state);
        row.add_suffix(&gtk::Image::from_icon_name(crate::ui::icons::NEXT));
        row.set_activatable(true);
        let sender = sender.clone();
        let id = account.account.id.clone();
        row.connect_activated(move |_| {
            sender.emit(AppInput::Accounts(AccountsInput::Visit(id.clone())));
        });
        section.add(&row);
    }
    Some(section)
}

/// The Jitsi server's group: `stored` reads what the core holds, `store` asks it to keep the text
/// and answers `false` for an address it refuses.
///
/// The text is sent on **apply** (Return, or the row's own apply button) and on losing focus,
/// never per keystroke: a half-typed address is one the core refuses. After a store the field
/// shows the stored form, which the core may have tidied (a trailing `/` dropped).
pub(super) fn jitsi_server_group(
    stored: impl Fn() -> Option<String> + 'static,
    store: impl Fn(String) -> bool + 'static,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_jitsi_server(),
        l10n::settings_jitsi_server_hint(),
    );
    // An entry row rather than a label over an entry: one accessible node carries the name.
    let row = adw::EntryRow::builder()
        .title(l10n::settings_jitsi_server())
        .use_markup(false)
        .show_apply_button(true)
        .input_purpose(gtk::InputPurpose::Url)
        .build();
    row.set_text(&stored().unwrap_or_default());
    let refused = gtk::Label::new(Some(l10n::settings_jitsi_server_invalid()));
    refused.add_css_class("error");
    refused.set_xalign(0.0);
    refused.set_wrap(true);
    refused.set_margin_top(6);
    refused.set_visible(false);
    section.add(&row);
    section.add(&refused);

    let commit = Rc::new({
        let refused = refused.clone();
        move |row: &adw::EntryRow| {
            let typed = row.text().to_string();
            let held = stored().unwrap_or_default();
            // Nothing changed: tabbing through the page costs no write.
            if typed == held {
                return;
            }
            if store(typed.clone()) {
                let held = stored().unwrap_or_default();
                if held != typed {
                    row.set_text(&held);
                }
            } else {
                refused.set_visible(true);
                row.add_css_class("error");
            }
        }
    });
    row.connect_changed(move |row| {
        refused.set_visible(false);
        row.remove_css_class("error");
    });
    let on_apply = Rc::clone(&commit);
    row.connect_apply(move |row| on_apply(row));
    let controller = gtk::EventControllerFocus::new();
    let focused = row.clone();
    controller.connect_leave(move |_| commit(&focused));
    row.add_controller(controller);
    section
}

/// Each account's default video meeting, None first and then what its calendars offer; no group
/// while no account's calendars offer one. `store` is asked to keep an account's new choice.
pub(super) fn default_meeting_group(
    defaults: &[MeetingDefault],
    store: impl Fn(String, Option<String>) + 'static,
) -> Option<adw::PreferencesGroup> {
    if defaults.is_empty() {
        return None;
    }
    let section = group(
        l10n::settings_default_meeting(),
        l10n::settings_default_meeting_hint(),
    );
    let store = Rc::new(store);
    for default in defaults {
        let labels = std::iter::once(l10n::event_meeting_none().to_owned())
            .chain(default.choices.iter().map(choice_label))
            .collect::<Vec<_>>();
        let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
        // A combo row carries its own title, so a screen reader hears whose default this is.
        let row = adw::ComboRow::new();
        row.set_use_markup(false);
        row.set_title(&default.account_address);
        row.set_model(Some(&gtk::StringList::new(&labels)));
        let chosen = default
            .chosen
            .as_ref()
            .and_then(|id| default.choices.iter().position(|choice| choice.id == *id))
            .and_then(|index| u32::try_from(index + 1).ok())
            .unwrap_or(0);
        row.set_selected(chosen);
        let store = Rc::clone(&store);
        let account = default.account.clone();
        let ids = default
            .choices
            .iter()
            .map(|choice| choice.id.clone())
            .collect::<Vec<_>>();
        // Connected after the stored choice is selected, so drawing the row stores nothing.
        row.connect_selected_notify(move |row| {
            let choice = usize::try_from(row.selected())
                .ok()
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| ids.get(index).cloned());
            store(account.clone(), choice);
        });
        section.add(&row);
    }
    Some(section)
}

#[cfg(test)]
#[path = "videoconferencing_tests.rs"]
pub(crate) mod tests;
