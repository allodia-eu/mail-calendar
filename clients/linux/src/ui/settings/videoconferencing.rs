//! Settings → Videoconferencing (`docs/settings.md`, category 4): the accounts that make video
//! meetings, and the person's Jitsi server (`docs/conferencing.md`, rule 13).
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
use mailcal_bindings::{CapabilityState, MeetingAccount};

use super::{PageContext, account_names::linked_name, group, page_box};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput},
};

pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_videoconferencing());
    let accounts = ctx.app.accounts_snapshot().meeting_accounts;
    if let Some(section) = meeting_accounts_group(&accounts, &ctx.sender) {
        content.append(&section);
    }
    let read = ctx.app.clone();
    let write = ctx.app.clone();
    content.append(&jitsi_server_group(
        move || read.jitsi_server(),
        move |typed| write.set_jitsi_server(Some(typed)),
    ));
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

#[cfg(test)]
#[path = "videoconferencing_tests.rs"]
pub(crate) mod tests;
