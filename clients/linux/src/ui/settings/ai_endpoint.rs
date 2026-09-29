//! Settings → Advanced → Own AI endpoint: the server writing style and drafted replies go to
//! instead of Allodia's relay (`docs/ai.md`, "Where requests go").
//!
//! On every build, because it is how a build without the relay gets AI at all. Saving and removing
//! both reach the keyring, which on this desktop is a D-Bus exchange that can wait on an unlock
//! prompt, so both run off the main thread. Whether the Writing style category appears follows
//! from the core's signal, not from this page.

use std::{rc::Rc, sync::Arc};

use adw::prelude::*;
use mailcal_bindings::{JurisdictionClass, MailcalApp, OwnAiEndpoint};

use super::{PageContext, group, group_titled, signatures::named_row};
use crate::{
    l10n,
    ui::{blocking::off_main_thread, writing_style::endpoint_error_text},
};

/// The fields, the three declarations, and what Save and Remove change on the page.
struct Form {
    address: gtk::Entry,
    key: gtk::PasswordEntry,
    key_row: adw::ActionRow,
    model: gtk::Entry,
    declared: [(JurisdictionClass, gtk::CheckButton); 3],
    save: gtk::Button,
    remove: gtk::Button,
    error: gtk::Label,
}

pub(super) fn section(ctx: &PageContext) -> gtk::Box {
    let stored = ctx.app.own_ai_endpoint();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let fields = group(l10n::ai_endpoint_title(), l10n::ai_endpoint_intro());
    let (address_row, address) = field(
        l10n::ai_endpoint_address(),
        Some(l10n::ai_endpoint_address_hint()),
        gtk::Entry::new(),
    );
    let (key_row, key) = field(l10n::ai_endpoint_key(), None, gtk::PasswordEntry::new());
    key.set_show_peek_icon(true);
    let (model_row, model) = field(l10n::ai_endpoint_model(), None, gtk::Entry::new());
    fields.add(&address_row);
    fields.add(&key_row);
    fields.add(&model_row);
    content.append(&fields);

    let places = group_titled(l10n::ai_endpoint_where());
    let first = gtk::CheckButton::new();
    let declared = [
        (JurisdictionClass::EuNative, first.clone()),
        (JurisdictionClass::EuHosted, gtk::CheckButton::new()),
        (JurisdictionClass::NonEu, gtk::CheckButton::new()),
    ];
    for (index, (class, check)) in declared.iter().enumerate() {
        if index > 0 {
            check.set_group(Some(&first));
        }
        let row = named_row(place_label(*class));
        check.set_valign(gtk::Align::Center);
        row.add_prefix(check);
        row.set_activatable_widget(Some(check));
        places.add(&row);
    }
    content.append(&places);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save = gtk::Button::with_label(l10n::ai_endpoint_save());
    save.add_css_class("suggested-action");
    let remove = gtk::Button::with_label(l10n::ai_endpoint_remove());
    remove.add_css_class("destructive-action");
    actions.append(&save);
    actions.append(&remove);
    content.append(&actions);
    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    content.append(&error);

    let form = Rc::new(Form {
        address,
        key,
        key_row,
        model,
        declared,
        save,
        remove,
        error,
    });
    form.show(stored.as_ref());
    let (saving, app) = (Rc::clone(&form), ctx.app.clone());
    form.save.connect_clicked(move |_| saving.store(&app));
    let (removing, app) = (Rc::clone(&form), ctx.app.clone());
    form.remove.connect_clicked(move |_| removing.clear(&app));
    content
}

/// One labelled field: the label and its hint on the row, the entry beside them carrying its own
/// accessible name, since the row cannot lend it its title. The entry is given a width of its own,
/// so a long hint wraps rather than leaving an address too narrow to read.
fn field<E: IsA<gtk::Widget> + IsA<gtk::Accessible> + IsA<gtk::Editable>>(
    title: &str,
    hint: Option<&str>,
    entry: E,
) -> (adw::ActionRow, E) {
    let row = named_row(title);
    if let Some(hint) = hint {
        row.set_subtitle(hint);
    }
    entry.update_property(&[gtk::accessible::Property::Label(title)]);
    entry.set_width_chars(28);
    entry.set_valign(gtk::Align::Center);
    entry.set_hexpand(true);
    row.add_suffix(&entry);
    (row, entry)
}

fn place_label(class: JurisdictionClass) -> &'static str {
    match class {
        JurisdictionClass::EuNative => l10n::ai_endpoint_where_eu_native(),
        JurisdictionClass::EuHosted => l10n::ai_endpoint_where_eu_hosted(),
        JurisdictionClass::NonEu | JurisdictionClass::Unknown => l10n::ai_endpoint_where_non_eu(),
    }
}

impl Form {
    /// Draws what is stored. The key never comes back: a stored one is said to be there, and an
    /// empty field then keeps it.
    fn show(&self, stored: Option<&OwnAiEndpoint>) {
        self.address
            .set_text(stored.map_or("", |endpoint| endpoint.base_url.as_str()));
        self.model
            .set_text(stored.map_or("", |endpoint| endpoint.model.as_str()));
        self.key.set_text("");
        let has_key = stored.is_some_and(|endpoint| endpoint.has_key);
        self.key_row.set_subtitle(if has_key {
            l10n::ai_endpoint_key_stored()
        } else {
            l10n::ai_endpoint_key_hint()
        });
        let declared = stored.and_then(|endpoint| endpoint.declared);
        for (class, check) in &self.declared {
            check.set_active(declared == Some(*class));
        }
        self.remove.set_visible(stored.is_some());
    }

    fn declared(&self) -> Option<JurisdictionClass> {
        self.declared
            .iter()
            .find(|(_, check)| check.is_active())
            .map(|(class, _)| *class)
    }

    fn store(self: &Rc<Self>, app: &Arc<MailcalApp>) {
        let base_url = self.address.text().trim().to_owned();
        let model = self.model.text().trim().to_owned();
        let declared = self.declared();
        // Empty keeps what is stored, and with nothing stored there is nothing to keep.
        let typed = self.key.text().trim().to_owned();
        let key = (!typed.is_empty()).then_some(typed);
        self.busy(true);
        let (worker, reader, form) = (Arc::clone(app), Arc::clone(app), Rc::downgrade(self));
        off_main_thread(
            move || worker.set_own_ai_endpoint(base_url, model, declared, key),
            move |saved| {
                let Some(form) = form.upgrade() else {
                    return;
                };
                form.busy(false);
                match saved {
                    Ok(()) => form.show(reader.own_ai_endpoint().as_ref()),
                    Err(error) => form.fail(endpoint_error_text(&error)),
                }
            },
        );
    }

    fn clear(self: &Rc<Self>, app: &Arc<MailcalApp>) {
        self.busy(true);
        let (worker, form) = (Arc::clone(app), Rc::downgrade(self));
        off_main_thread(
            move || worker.clear_own_ai_endpoint(),
            move |removed| {
                let Some(form) = form.upgrade() else {
                    return;
                };
                form.busy(false);
                // The settings are gone either way; only the keyring can refuse.
                form.show(None);
                if let Err(error) = removed {
                    form.fail(endpoint_error_text(&error));
                }
            },
        );
    }

    fn busy(&self, busy: bool) {
        self.save.set_sensitive(!busy);
        self.remove.set_sensitive(!busy);
        if busy {
            self.error.set_visible(false);
        }
    }

    fn fail(&self, message: &str) {
        self.error.set_text(message);
        self.error.set_visible(true);
    }
}
