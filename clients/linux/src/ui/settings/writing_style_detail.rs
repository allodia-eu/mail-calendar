//! An existing style's detail (`docs/ai.md` rule 16), drawn in place of the Writing style list
//! from the core's `writing_style_detail`: what it was learned from, the reveal's sections on one
//! page, the name with its Save, the notes sent on Return and on leaving the field, and Forget,
//! asked first.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    ControlState, WritingStyleDetail, WritingStyleRevealPage, WritingStyleSection,
};

use super::{
    writing_style_reveal::save_control,
    writing_style_reveal_cards::section,
    writing_style_reveal_pages::{self as pages, heading, text},
};
use crate::{
    l10n,
    ui::{controls, icons},
};

/// What the detail's controls ask for.
pub(super) struct Actions {
    pub(super) back: Rc<dyn Fn()>,
    pub(super) rename: Rc<dyn Fn(String)>,
    pub(super) notes: Rc<dyn Fn(String)>,
    pub(super) forget: Rc<dyn Fn()>,
    /// Another language picked, by its position.
    pub(super) language: Rc<dyn Fn(usize)>,
}

/// A section as the reveal's page about it, and its heading.
fn section_of(section: WritingStyleSection) -> (WritingStyleRevealPage, &'static str) {
    match section {
        WritingStyleSection::Letter => (WritingStyleRevealPage::Letter, l10n::reveal_step_letter()),
        WritingStyleSection::Habits => (WritingStyleRevealPage::Habits, l10n::reveal_step_habits()),
        WritingStyleSection::Voice => (WritingStyleRevealPage::Voice, l10n::reveal_step_voice()),
        WritingStyleSection::Phrases => (WritingStyleRevealPage::Phrases, l10n::reveal_phrases()),
    }
}

/// Draws `detail` into `body`, on the language at `language`, saying `refused` when the last
/// change to it was refused.
pub(super) fn draw(
    body: &gtk::Box,
    detail: &WritingStyleDetail,
    language: usize,
    refused: Option<&str>,
    actions: &Actions,
) {
    pages::install_styles();
    let back = gtk::Button::from_icon_name(icons::PREVIOUS);
    back.add_css_class("flat");
    back.add_css_class("circular");
    back.set_tooltip_text(Some(l10n::a11y_back()));
    back.update_property(&[gtk::accessible::Property::Label(l10n::a11y_back())]);
    let go_back = Rc::clone(&actions.back);
    back.connect_clicked(move |_| go_back());
    back.set_valign(gtk::Align::Center);
    let title = text(&detail.name);
    title.add_css_class("title-2");
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    top.append(&back);
    top.append(&title);
    body.append(&top);
    if let Some(refused) = refused {
        let said = text(refused);
        said.add_css_class("error");
        body.append(&said);
    }
    pages::read(body, &detail.facts, false);

    body.append(&name_group(detail, actions));

    if detail.languages.len() > 1 {
        body.append(&language_picker(detail, language, actions));
    }
    if let Some(shown) = detail
        .languages
        .get(language)
        .or_else(|| detail.languages.first())
    {
        for (page, title) in detail.sections.iter().copied().map(section_of) {
            let part = gtk::Box::new(gtk::Orientation::Vertical, 12);
            let named = heading(title);
            named.add_css_class("title-4");
            part.append(&named);
            section(&part, page, shown);
            body.append(&part);
        }
    }

    let forget = Rc::clone(&actions.forget);
    let button = controls::button(
        l10n::writing_style_forget(),
        detail.forget,
        true,
        move || {
            forget();
        },
    );
    button.set_halign(gtk::Align::Start);
    button.set_margin_top(12);
    body.append(&button);
}

/// The name with its Save, and the notes.
fn name_group(detail: &WritingStyleDetail, actions: &Actions) -> gtk::Box {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let name = adw::EntryRow::new();
    name.set_use_markup(false);
    name.set_title(l10n::writing_style_name_label());
    name.set_text(&detail.name);
    controls::hold_field(
        &name,
        ControlState {
            running: false,
            ..detail.rename
        },
    );
    let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    slot.set_valign(gtk::Align::Center);
    name.add_suffix(&slot);
    let (drawn, rename, state) = (
        detail.name.clone(),
        Rc::clone(&actions.rename),
        detail.rename,
    );
    let draw_save = Rc::new(move |row: &adw::EntryRow| {
        let typed = row.text().to_string();
        controls::form_unsent(row, typed != drawn);
        while let Some(child) = slot.first_child() {
            slot.remove(&child);
        }
        let shown = save_control(state, &typed);
        let (rename, sent, field) = (Rc::clone(&rename), typed.clone(), row.downgrade());
        let button = controls::button(l10n::writing_style_rename(), shown, false, move || {
            // Sent: the page may be drawn again under the field.
            if let Some(field) = field.upgrade() {
                controls::form_unsent(&field, false);
            }
            rename(sent.clone());
        });
        slot.append(&button);
    });
    draw_save(&name);
    let on_change = Rc::clone(&draw_save);
    name.connect_changed(move |row| on_change(row));
    let (on_return, held) = (Rc::clone(&actions.rename), detail.rename);
    name.connect_entry_activated(move |row| {
        let typed = row.text().to_string();
        if save_control(held, &typed).enabled {
            controls::form_unsent(row, false);
            on_return(typed);
        }
    });
    list.append(&name);
    let notes = Rc::clone(&actions.notes);
    list.append(&controls::entry_row(
        l10n::writing_style_notes(),
        &detail.notes,
        detail.notes_control,
        move |typed| notes(typed),
    ));
    group.append(&list);
    let hint = text(l10n::writing_style_notes_hint());
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    hint.set_margin_start(12);
    group.append(&hint);
    group
}

/// A toggle per language, named in its own language.
fn language_picker(detail: &WritingStyleDetail, language: usize, actions: &Actions) -> gtk::Box {
    let control = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    control.add_css_class("linked");
    control.set_halign(gtk::Align::Start);
    control.set_accessible_role(gtk::AccessibleRole::Group);
    control.update_property(&[gtk::accessible::Property::Label(
        l10n::a11y_reveal_language(),
    )]);
    let mut first: Option<gtk::ToggleButton> = None;
    for (index, shown) in detail.languages.iter().enumerate() {
        let toggle = gtk::ToggleButton::with_label(&l10n::language_name(&shown.language));
        match &first {
            Some(anchor) => toggle.set_group(Some(anchor)),
            None => first = Some(toggle.clone()),
        }
        toggle.set_active(index == language);
        let pick = Rc::clone(&actions.language);
        toggle.connect_toggled(move |toggle| {
            if toggle.is_active() && index != language {
                pick(index);
            }
        });
        control.append(&toggle);
    }
    control
}
