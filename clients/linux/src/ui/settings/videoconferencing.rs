//! Settings → Videoconferencing (`docs/settings.md`, Videoconferencing): the accounts that make video
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
//!
//! The page follows the core's calendar and settings signals, each part drawn again only when
//! what it was drawn from changed.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::{CapabilityState, MeetingAccount, MeetingDefault, PreferenceIntent};

use super::{PageContext, account_names::linked_name, group, page_box};
use crate::{
    l10n,
    ui::{
        AppInput, account_settings::AccountsInput, calendar::editor_meeting::choice_label,
        preference,
    },
};

pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_videoconferencing());
    let accounts = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let defaults = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let read = ctx.app.clone();
    let send = ctx.app.clone();
    let (jitsi, follow_jitsi) = jitsi_server_group(
        move || {
            let page = read.videoconferencing_page();
            (page.jitsi_server, page.jitsi_refused)
        },
        move |server| preference::set(&send, PreferenceIntent::SetJitsiServer { server }),
    );
    content.append(&accounts);
    content.append(&jitsi);
    content.append(&defaults);
    // Each part is drawn again only when what it was drawn from changed: a redraw would take a
    // picker from under the person who just used it.
    let drawn_accounts: RefCell<Option<Vec<MeetingAccount>>> = RefCell::default();
    let drawn_defaults: RefCell<Option<Vec<MeetingDefault>>> = RefCell::default();
    // Set while a picker is moved to the core's choice, so the move stores nothing.
    let moving = Rc::new(Cell::new(false));
    let (app, sender) = (ctx.app.clone(), ctx.sender.clone());
    let follow = move || {
        follow_jitsi();
        let now = app.accounts_snapshot().meeting_accounts;
        if drawn_accounts.borrow().as_ref() != Some(&now) {
            replace(&accounts, meeting_accounts_group(&now, &sender));
            drawn_accounts.replace(Some(now));
        }
        let now = app.videoconferencing_page().defaults;
        if offered(drawn_defaults.borrow().as_deref()) == offered(Some(&now)) {
            moving.set(true);
            move_defaults(&defaults, &now);
            moving.set(false);
            return;
        }
        let store = app.clone();
        let held = Rc::clone(&moving);
        replace(
            &defaults,
            default_meeting_group(&now, move |account, choice| {
                if !held.get() {
                    preference::set(
                        &store,
                        PreferenceIntent::SetDefaultMeeting { account, choice },
                    );
                }
            }),
        );
        drawn_defaults.replace(Some(now));
    };
    follow();
    ctx.calendar.add(follow);
    content
}

/// Puts `section` in `holder` in place of what it held, or leaves it empty.
fn replace(holder: &gtk::Box, section: Option<adw::PreferencesGroup>) {
    if let Some(drawn) = holder.first_child() {
        holder.remove(&drawn);
    }
    holder.set_visible(section.is_some());
    if let Some(section) = section {
        holder.append(&section);
    }
}

/// What the default pickers offer, without what each has chosen.
fn offered(defaults: Option<&[MeetingDefault]>) -> Option<Vec<MeetingDefault>> {
    defaults.map(|defaults| {
        defaults
            .iter()
            .map(|default| MeetingDefault {
                chosen: None,
                ..default.clone()
            })
            .collect()
    })
}

/// Moves each account's picker to the core's choice, in the order the group drew them.
fn move_defaults(holder: &gtk::Box, defaults: &[MeetingDefault]) {
    let mut rows = Vec::new();
    combo_rows(holder.upcast_ref(), &mut rows);
    for (row, default) in rows.iter().zip(defaults) {
        let chosen = chosen_index(default);
        if row.selected() != chosen {
            row.set_selected(chosen);
        }
    }
}

/// Every picker under `widget`, in the order they are drawn.
fn combo_rows(widget: &gtk::Widget, rows: &mut Vec<adw::ComboRow>) {
    let mut child = widget.first_child();
    while let Some(at) = child {
        if let Some(row) = at.downcast_ref::<adw::ComboRow>() {
            rows.push(row.clone());
        } else {
            combo_rows(&at, rows);
        }
        child = at.next_sibling();
    }
}

/// The picker's position for an account's chosen meeting: None first, then its choices.
fn chosen_index(default: &MeetingDefault) -> u32 {
    default
        .chosen
        .as_ref()
        .and_then(|id| default.choices.iter().position(|choice| choice.id == *id))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(0)
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

/// The Jitsi server's group, and what brings it to the core. `read` answers the stored server and
/// the text the core last refused; `send` hands the core the field's text.
///
/// The text is sent on **apply** (Return, or the row's own apply button) and on losing focus,
/// never per keystroke: a half-typed address is one the core refuses. Whether it changed is the
/// core's question. After a send the field shows the stored form, which the core may have tidied
/// (a trailing `/` dropped), and the refusal shows while the field holds the text refused.
pub(super) fn jitsi_server_group(
    read: impl Fn() -> (Option<String>, Option<String>) + 'static,
    send: impl Fn(String) + 'static,
) -> (adw::PreferencesGroup, impl Fn() + 'static) {
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
    let (stored, refused_text) = read();
    row.set_text(&stored.unwrap_or_default());
    let refused = gtk::Label::new(Some(l10n::settings_jitsi_server_invalid()));
    refused.add_css_class("error");
    refused.set_xalign(0.0);
    refused.set_wrap(true);
    refused.set_margin_top(6);
    section.add(&row);
    section.add(&refused);
    let held = Rc::new(RefCell::new(refused_text));
    let say = Rc::new({
        let (row, refused, held) = (row.clone(), refused.clone(), Rc::clone(&held));
        move || {
            let shown = held.borrow().as_deref() == Some(row.text().as_str());
            refused.set_visible(shown);
            if shown {
                row.add_css_class("error");
            } else {
                row.remove_css_class("error");
            }
        }
    });
    say();
    let read = Rc::new(read);
    let commit = Rc::new({
        let (read, held, say) = (Rc::clone(&read), Rc::clone(&held), Rc::clone(&say));
        move |row: &adw::EntryRow| {
            let typed = row.text().to_string();
            send(typed.clone());
            let (stored, refused) = read();
            let kept = refused.as_deref() != Some(typed.as_str());
            held.replace(refused);
            let stored = stored.unwrap_or_default();
            if kept && stored != typed {
                row.set_text(&stored);
            }
            say();
        }
    });
    let changed = Rc::clone(&say);
    row.connect_changed(move |_| changed());
    let on_apply = Rc::clone(&commit);
    row.connect_apply(move |row| on_apply(row));
    let controller = gtk::EventControllerFocus::new();
    let focused = row.clone();
    controller.connect_leave(move |_| commit(&focused));
    row.add_controller(controller);
    let follow = move || {
        held.replace(read().1);
        say();
    };
    (section, follow)
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
        row.set_selected(chosen_index(default));
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
