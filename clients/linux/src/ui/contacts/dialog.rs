//! The contact editor window, and the small question that precedes it when a person is filed
//! in more than one account.
//!
//! Built from `adw::EntryRow` rather than a label over a `gtk::Entry`. Two reasons, and the
//! second is not cosmetic: it is the GNOME form widget, and a heading beside an entry gives
//! **two** accessibility nodes the same name, so a screen reader announces the field twice and
//! anything driving by name (the AT-SPI harness included) resolves to the label, which cannot
//! be typed into.
//!
//! The email and phone fields are **lists**, which is the only structurally awkward part: GTK
//! has no repeating-field widget, so each value is a row the dialog adds and removes, and the
//! rows are held in one shared vector so Save can read them in the order they are on screen.
//! That order is not cosmetic either: the first address is the person's primary one, and it is
//! what the avatar and the list row are keyed on.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    super::AppInput,
    editor::{ContactEditor, ContactForm, FormError},
    model::CardChoice,
};
use crate::l10n;

/// Presents the editor for `editor`, create or edit.
pub(super) fn present_editor(
    parent: &adw::ApplicationWindow,
    editor: &ContactEditor,
    sender: relm4::Sender<AppInput>,
) {
    let title = if editor.editing.is_some() {
        l10n::contacts_edit()
    } else {
        l10n::contacts_new()
    };
    let (window, header) = crate::ui::modal::new(parent, title, 520, Some(640));
    let dismissing = Rc::new(RefCell::new(false));
    connect_dismiss(&window, &dismissing, sender.clone());

    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.update_property(&[AccessibleProperty::Label(l10n::action_cancel())]);
    let closing = Rc::clone(&dismissing);
    let dialog = window.clone();
    let input = sender.clone();
    cancel.connect_clicked(move |_| {
        *closing.borrow_mut() = true;
        input.emit(AppInput::DismissContactEditor);
        dialog.close();
    });
    header.pack_start(&cancel);
    let save = gtk::Button::with_label(l10n::action_save());
    save.add_css_class("suggested-action");
    save.update_property(&[AccessibleProperty::Label(l10n::action_save())]);
    header.pack_end(&save);

    let form = gtk::Box::new(gtk::Orientation::Vertical, 18);
    form.set_margin_start(18);
    form.set_margin_end(18);
    form.set_margin_top(18);
    form.set_margin_bottom(24);

    let identity = adw::PreferencesGroup::new();
    let given = entry_row(l10n::contacts_first_name(), &editor.seed.given_name);
    identity.add(&given);
    // The caret opens where the work starts: the empty first field of a new contact. Deferred
    // to `map` for the reason the calendar editor defers it; a widget with no window to take
    // focus in answers `false` and does nothing, silently.
    if editor.editing.is_none() {
        given.connect_map(|row| {
            row.grab_focus();
        });
    }
    let surname = entry_row(l10n::contacts_last_name(), &editor.seed.surname);
    identity.add(&surname);
    let organization = entry_row(
        l10n::contacts_section_organizations(),
        &editor.seed.organization,
    );
    identity.add(&organization);
    let title_row = entry_row(l10n::contacts_section_titles(), &editor.seed.title);
    identity.add(&title_row);
    form.append(&identity);

    let emails = value_group(
        &form,
        l10n::contacts_section_emails(),
        l10n::contacts_add_email(),
        l10n::contacts_remove_email(),
        &editor.seed.emails,
    );
    let phones = value_group(
        &form,
        l10n::contacts_section_phones(),
        l10n::contacts_add_phone(),
        l10n::contacts_remove_phone(),
        &editor.seed.phones,
    );

    // Only a create files a contact somewhere new, and only when there is a choice to make:
    // one book is not a decision, it is a fact the user has no use for.
    let book = (editor.editing.is_none() && editor.choices.len() > 1).then(|| {
        let group = adw::PreferencesGroup::new();
        let labels = editor
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        // The row's own title is the accessible name; a `ComboRow` is not an `Accessible` in
        // the bindings, so there is nothing to override it with and nothing that needs it.
        let picker = adw::ComboRow::new();
        picker.set_title(l10n::contacts_address_book());
        picker.set_use_markup(false);
        picker.set_model(Some(&gtk::StringList::new(&labels)));
        picker.set_selected(editor.selected);
        group.add(&picker);
        form.append(&group);
        picker
    });

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    form.append(&error);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&form));
    scroll.set_vexpand(true);
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.append(&scroll);
    window.set_child(Some(&shell));

    let editor = editor.clone();
    let closing = Rc::clone(&dismissing);
    let dialog = window.clone();
    let selected = editor.selected;
    save.connect_clicked(move |_| {
        let form = ContactForm {
            given_name: given.text().to_string(),
            surname: surname.text().to_string(),
            organization: organization.text().to_string(),
            title: title_row.text().to_string(),
            emails: values(&emails),
            phones: values(&phones),
            book_index: book.as_ref().map_or(selected, adw::ComboRow::selected),
        };
        match editor.intent(&form) {
            Ok(intent) => {
                *closing.borrow_mut() = true;
                sender.emit(AppInput::SubmitContactForm(Box::new(intent)));
                dialog.close();
            }
            Err(reason) => {
                error.set_text(match reason {
                    FormError::Empty => l10n::contacts_editor_invalid(),
                    FormError::Email => l10n::contacts_editor_invalid_email(),
                });
                error.set_visible(true);
            }
        }
    });
    window.present();
}

/// Asks which account's card to edit, when the person is filed in more than one.
///
/// Its own step rather than a picker inside the editor, because the answer decides what the
/// form is *seeded with*: a merged person's values belong to different cards, and an editor
/// that let the user change accounts mid-edit would have to throw away what they had typed.
pub(super) fn present_card_choice(
    parent: &adw::ApplicationWindow,
    choices: &[CardChoice],
    sender: &relm4::Sender<AppInput>,
) {
    let (window, _) = crate::ui::modal::new(parent, l10n::contacts_edit(), 420, None);
    window.set_resizable(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let question = gtk::Label::new(Some(l10n::contacts_pick_card()));
    question.set_wrap(true);
    question.set_xalign(0.0);
    content.append(&question);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    for choice in choices {
        let row = adw::ActionRow::builder()
            .title(&choice.label)
            .use_markup(false)
            .activatable(true)
            .build();
        let input = sender.clone();
        let dialog = window.clone();
        let account = choice.account.clone();
        let card = choice.card.clone();
        row.connect_activated(move |_| {
            input.emit(AppInput::BeginEditContact(account.clone(), card.clone()));
            dialog.close();
        });
        list.append(&row);
    }
    content.append(&list);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.set_halign(gtk::Align::End);
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    content.append(&cancel);
    window.set_child(Some(&content));
    window.present();
}

/// Builds one headed, repeating value field, returning its rows in screen order.
fn value_group(
    form: &gtk::Box,
    heading: &'static str,
    add_label: &'static str,
    remove_label: &'static str,
    seed: &[String],
) -> Rc<RefCell<Vec<adw::EntryRow>>> {
    let group = adw::PreferencesGroup::new();
    group.set_title(heading);
    let rows: Rc<RefCell<Vec<adw::EntryRow>>> = Rc::new(RefCell::new(Vec::new()));
    // One empty row on a contact that has none, so the field is something to type in rather
    // than a heading over a button.
    if seed.is_empty() {
        append_value_row(&group, &rows, heading, remove_label, "");
    }
    for value in seed {
        append_value_row(&group, &rows, heading, remove_label, value);
    }
    let add = gtk::Button::with_label(add_label);
    add.set_halign(gtk::Align::Start);
    add.add_css_class("flat");
    add.update_property(&[AccessibleProperty::Label(add_label)]);
    let held = Rc::clone(&rows);
    let container = group.clone();
    add.connect_clicked(move |_| {
        let row = append_value_row(&container, &held, heading, remove_label, "");
        row.grab_focus();
    });
    group.set_header_suffix(Some(&add));
    form.append(&group);
    rows
}

/// Appends one value row, returning it.
fn append_value_row(
    group: &adw::PreferencesGroup,
    rows: &Rc<RefCell<Vec<adw::EntryRow>>>,
    heading: &str,
    remove_label: &'static str,
    value: &str,
) -> adw::EntryRow {
    let row = entry_row(heading, value);
    let remove = gtk::Button::from_icon_name("user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_valign(gtk::Align::Center);
    remove.set_tooltip_text(Some(remove_label));
    remove.update_property(&[AccessibleProperty::Label(remove_label)]);
    let held = Rc::clone(rows);
    let removed = row.clone();
    let container = group.clone();
    remove.connect_clicked(move |_| {
        held.borrow_mut().retain(|held| held != &removed);
        container.remove(&removed);
    });
    row.add_suffix(&remove);
    group.add(&row);
    rows.borrow_mut().push(row.clone());
    row
}

/// The values on screen, in the order they are drawn.
fn values(rows: &Rc<RefCell<Vec<adw::EntryRow>>>) -> Vec<String> {
    rows.borrow()
        .iter()
        .map(|row| row.text().to_string())
        .collect()
}

fn connect_dismiss(
    window: &gtk::Window,
    dismissing: &Rc<RefCell<bool>>,
    sender: relm4::Sender<AppInput>,
) {
    let dismissing = Rc::clone(dismissing);
    window.connect_close_request(move |_| {
        if !*dismissing.borrow() {
            sender.emit(AppInput::DismissContactEditor);
        }
        gtk::glib::Propagation::Proceed
    });
}

/// One labelled, editable field: the title is the accessible name, so the field is announced
/// (and reachable) exactly once.
fn entry_row(title: &str, value: &str) -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_title(title);
    // The title is this app's own copy rather than a server's, so nothing hostile reaches it;
    // markup is still off because a translation holding a bare `&` would otherwise render as
    // an unterminated entity, which is a blank field label.
    row.set_use_markup(false);
    row.set_text(value);
    row
}
