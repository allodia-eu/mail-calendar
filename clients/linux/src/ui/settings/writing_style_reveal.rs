//! One writing style, shown back in plain words: what was noticed per language, the person's own
//! notes, its name, and forgetting it (`docs/ai.md`, "The reveal").
//!
//! The same detail opens after a learning run and from a library row, so a style reads the same
//! the first time as every time after. Its description is a model's text about the person's mail,
//! so every value is a subtitle on a row whose markup is off.

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{LanguageStyleRow, WritingStyleDetail};

use super::{PageContext, group, group_titled, page_box, pages, signatures::named_row};
use crate::{l10n, ui::writing_style::reveal_fields};

const NAME: &str = "writing-style-reveal";

/// Opens the style as a Settings detail.
pub(super) fn open(ctx: &PageContext, detail: &WritingStyleDetail) {
    if let Some(previous) = ctx.navigation.child_by_name(NAME) {
        ctx.navigation.remove(&previous);
    }
    let page = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&detail.row.name, "")));
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let navigation = ctx.navigation.downgrade();
    cancel.connect_clicked(move |_| leave(&navigation));
    header.pack_start(&cancel);
    page.add_top_bar(&header);

    let content = page_box(l10n::reveal_title());
    let locale = l10n::active_locale();
    for language in &detail.languages {
        content.append(&language_group(language, locale));
    }

    let names = adw::PreferencesGroup::new();
    let name = adw::EntryRow::new();
    name.set_use_markup(false);
    name.set_title(l10n::writing_style_name_label());
    name.set_text(&detail.row.name);
    names.add(&name);
    content.append(&names);

    let notes_group = group(
        l10n::writing_style_notes(),
        l10n::writing_style_notes_hint(),
    );
    let notes = gtk::TextView::new();
    notes.set_wrap_mode(gtk::WrapMode::WordChar);
    notes.set_accepts_tab(false);
    notes.set_top_margin(8);
    notes.set_bottom_margin(8);
    notes.set_left_margin(8);
    notes.set_right_margin(8);
    notes.buffer().set_text(&detail.notes);
    notes.update_property(&[gtk::accessible::Property::Label(l10n::writing_style_notes())]);
    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&notes));
    frame.set_size_request(-1, 120);
    notes_group.add(&frame);
    content.append(&notes_group);

    let forget = gtk::Button::with_label(l10n::writing_style_forget());
    forget.add_css_class("destructive-action");
    forget.set_halign(gtk::Align::Start);
    content.append(&forget);

    let save = gtk::Button::with_label(l10n::reveal_save());
    save.add_css_class("suggested-action");
    // A style with no name is a picker entry nobody can tell apart, so Save waits for one.
    let gated = save.clone();
    name.connect_changed(move |entry| gated.set_sensitive(!entry.text().trim().is_empty()));
    connect_save(ctx, detail, &save, &name, &notes);
    header.pack_end(&save);
    let (ctx_forget, style) = (ctx.clone(), detail.row.clone());
    forget.connect_clicked(move |_| confirm_forget(&ctx_forget, &style.id, &style.name));

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&content));
    page.set_content(Some(&scroll));
    ctx.navigation.add_named(&page, Some(NAME));
    ctx.navigation.set_visible_child_name(NAME);
}

/// One language's description, headed by the language's own name. A label over what was noticed,
/// and nothing for a field the model left empty.
fn language_group(language: &LanguageStyleRow, locale: &str) -> adw::PreferencesGroup {
    let section = group_titled(&l10n::language_name(&language.language));
    for (label, value) in reveal_fields(language, locale) {
        let row = named_row(&label);
        if !value.is_empty() {
            row.set_subtitle(&value);
        }
        section.add(&row);
    }
    section
}

/// Save stores what changed and only that: a rename, the notes, or both.
fn connect_save(
    ctx: &PageContext,
    detail: &WritingStyleDetail,
    save: &gtk::Button,
    name: &adw::EntryRow,
    notes: &gtk::TextView,
) {
    let app = ctx.app.clone();
    let navigation = ctx.navigation.downgrade();
    let (id, stored_name, stored_notes) = (
        detail.row.id.clone(),
        detail.row.name.clone(),
        detail.notes.clone(),
    );
    let (name, notes) = (name.clone(), notes.clone());
    save.connect_clicked(move |_| {
        let chosen = name.text().trim().to_owned();
        if chosen.is_empty() {
            return;
        }
        if chosen != stored_name {
            app.rename_writing_style(id.clone(), chosen);
        }
        let buffer = notes.buffer();
        let (start, end) = buffer.bounds();
        let written = buffer.text(&start, &end, false).to_string();
        if written != stored_notes {
            app.update_writing_style_notes(id.clone(), written);
        }
        leave(&navigation);
    });
}

/// Forgetting is confirmed in a modal, as deleting a signature is, and the message says what it
/// costs beyond this list: every account drafting in it loses it.
fn confirm_forget(ctx: &PageContext, id: &str, name: &str) {
    let dialog = pages::confirm_window(
        &ctx.window,
        &l10n::writing_style_forget_title(name),
        l10n::writing_style_forget_message(),
    );
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let forget = gtk::Button::with_label(l10n::writing_style_forget());
    forget.add_css_class("destructive-action");
    let window = dialog.clone();
    let app = ctx.app.clone();
    let navigation = ctx.navigation.downgrade();
    let id = id.to_owned();
    forget.connect_clicked(move |_| {
        app.delete_writing_style(id.clone());
        window.close();
        leave(&navigation);
    });
    actions.append(&forget);
    dialog
        .child()
        .and_downcast::<gtk::Box>()
        .expect("confirmation content")
        .append(&actions);
    dialog.present();
}

/// Back to the category, on the next turn: the button that asked is inside the detail removed.
fn leave(navigation: &glib::WeakRef<gtk::Stack>) {
    let navigation = navigation.clone();
    glib::idle_add_local_once(move || {
        let Some(navigation) = navigation.upgrade() else {
            return;
        };
        if navigation.visible_child_name().as_deref() == Some(NAME) {
            navigation.set_visible_child_name("settings");
        }
        if let Some(detail) = navigation.child_by_name(NAME) {
            navigation.remove(&detail);
        }
    });
}
