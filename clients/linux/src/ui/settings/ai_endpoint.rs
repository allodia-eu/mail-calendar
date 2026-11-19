//! The own AI endpoint, under Settings → Advanced (`docs/ai.md` rules 6 and 7), drawn from the
//! core's `ai_endpoint_form` and absent while it is `None`.
//!
//! Each gap the core finds in the fields as typed is said under its field, and Save is held while
//! one is named. The fields are drawn from the record until the person types, and then stay theirs
//! until Save sends them; they are held while a save or a removal runs. A key is never drawn back.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::{
    AiEndpointForm, AiEndpointGaps, AiEndpointIntent, AiEndpointResult, AiIntent, ControlState,
    Intent, JurisdictionClass,
};

use super::{Category, PageContext, group, writing_style_words as words};
use crate::{l10n, ui::controls};

/// What the form asks of the core for the fields as typed.
pub(super) type Gap = Rc<dyn Fn(String, String, Option<JurisdictionClass>) -> AiEndpointGaps>;
/// Where a press goes.
pub(super) type Send = Rc<dyn Fn(AiEndpointIntent)>;

/// The fields' values, as typed or as drawn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Typed {
    address: String,
    model: String,
    declared: Option<JurisdictionClass>,
}

/// The section, which follows the core's signals until the window builds its pages again.
pub(super) fn section(ctx: &PageContext) -> gtk::Box {
    let app = ctx.app.clone();
    let gap: Gap =
        Rc::new(move |address, model, declared| app.ai_endpoint_gap(address, model, declared));
    let app = ctx.app.clone();
    let send: Send = Rc::new(move |intent| {
        app.dispatch(Intent::Ai {
            intent: AiIntent::Endpoint { intent },
        });
    });
    let form = Form::new(gap, send);
    form.draw(ctx.app.ai_endpoint_form());
    ctx.sidebar
        .anchor(Category::Advanced, form.root.upcast_ref());
    // The window keeps the form for as long as it keeps the page: until it builds its pages again.
    let (app, following) = (ctx.app.clone(), Rc::clone(&form));
    ctx.ai_endpoint
        .add(move || following.draw(app.ai_endpoint_form()));
    form.root.clone()
}

pub(super) struct Form {
    pub(super) root: gtk::Box,
    address: adw::EntryRow,
    key: adw::PasswordEntryRow,
    model: adw::EntryRow,
    declared: Vec<(JurisdictionClass, gtk::CheckButton)>,
    address_note: gtk::Label,
    key_note: gtk::Label,
    model_note: gtk::Label,
    where_note: gtk::Label,
    failed: gtk::Label,
    buttons: gtk::Box,
    /// Save and Remove as last drawn, so a button is built again only when it moves.
    buttons_drawn: Cell<Option<(ControlState, ControlState)>>,
    /// The fields as last drawn or sent: typing away from them makes the fields the person's.
    baseline: RefCell<Typed>,
    drawn: RefCell<Option<AiEndpointForm>>,
    /// The newest outcome acted on.
    seen: Cell<u64>,
    gap: Gap,
    send: Send,
}

impl Form {
    pub(super) fn new(gap: Gap, send: Send) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let section = group(l10n::ai_endpoint_title(), l10n::ai_endpoint_intro());
        let fields = gtk::Box::new(gtk::Orientation::Vertical, 14);
        let address = entry_row(l10n::ai_endpoint_address());
        address.set_input_purpose(gtk::InputPurpose::Url);
        let key = adw::PasswordEntryRow::new();
        key.set_use_markup(false);
        key.set_title(l10n::ai_endpoint_key());
        let model = entry_row(l10n::ai_endpoint_model());
        let (address_note, key_note, model_note, where_note) = (note(), note(), note(), note());
        key_note.add_css_class("dim-label");
        fields.append(&field(&[address.upcast_ref()], &address_note));
        fields.append(&field(&[key.upcast_ref()], &key_note));
        fields.append(&field(&[model.upcast_ref()], &model_note));

        let heading = gtk::Label::new(Some(l10n::ai_endpoint_where()));
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        heading.set_margin_top(4);
        fields.append(&heading);
        let mut first: Option<gtk::CheckButton> = None;
        let mut rows = Vec::new();
        let mut declared = Vec::new();
        for class in words::DECLARATIONS {
            let check = gtk::CheckButton::new();
            match &first {
                Some(anchor) => check.set_group(Some(anchor)),
                None => first = Some(check.clone()),
            }
            let row = super::super::mailbox::plain_text_row();
            row.set_title(words::declaration(class));
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            rows.push(row);
            declared.push((class, check));
        }
        let row_refs = rows.iter().map(Cast::upcast_ref).collect::<Vec<_>>();
        fields.append(&field(&row_refs, &where_note));

        let failed = gtk::Label::new(None);
        failed.add_css_class("error");
        failed.set_wrap(true);
        failed.set_xalign(0.0);
        fields.append(&failed);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        fields.append(&buttons);
        section.add(&fields);
        root.append(&section);

        let form = Rc::new(Self {
            root,
            address,
            key,
            model,
            declared,
            address_note,
            key_note,
            model_note,
            where_note,
            failed,
            buttons,
            buttons_drawn: Cell::new(None),
            baseline: RefCell::default(),
            drawn: RefCell::new(None),
            seen: Cell::new(0),
            gap,
            send,
        });
        let typed = Rc::downgrade(&form);
        let changed = move || {
            if let Some(form) = typed.upgrade() {
                form.judge();
            }
        };
        let on_text = changed.clone();
        for field in [&form.address, &form.model] {
            let on_text = on_text.clone();
            field.connect_changed(move |_| on_text());
        }
        for (_, check) in &form.declared {
            let changed = changed.clone();
            check.connect_toggled(move |check| {
                if check.is_active() {
                    changed();
                }
            });
        }
        let pressed = Rc::downgrade(&form);
        form.key.connect_entry_activated(move |_| {
            if let Some(form) = pressed.upgrade() {
                form.save_on_return();
            }
        });
        for field in [&form.address, &form.model] {
            let pressed = Rc::downgrade(&form);
            field.connect_entry_activated(move |_| {
                if let Some(form) = pressed.upgrade() {
                    form.save_on_return();
                }
            });
        }
        form
    }

    /// Draws `record`, or takes the section away while there is none.
    pub(super) fn draw(self: &Rc<Self>, record: Option<AiEndpointForm>) {
        let Some(record) = record else {
            self.root.set_visible(false);
            self.drawn.replace(None);
            return;
        };
        self.root.set_visible(true);
        if self.drawn.borrow().as_ref() == Some(&record) {
            return;
        }
        let first = self.drawn.borrow().is_none();
        if first {
            self.seen
                .set(record.outcome.map_or(0, |outcome| outcome.serial));
        }
        if self.typed() == *self.baseline.borrow() {
            let typed = Typed {
                address: record.address.clone(),
                model: record.model.clone(),
                declared: record.declared,
            };
            self.address.set_text(&typed.address);
            self.model.set_text(&typed.model);
            for (class, check) in &self.declared {
                check.set_active(Some(*class) == typed.declared);
            }
            self.baseline.replace(typed);
        }
        self.key_note.set_text(if record.key_kept {
            l10n::ai_endpoint_key_stored()
        } else {
            l10n::ai_endpoint_key_hint()
        });
        let held = ControlState {
            running: false,
            ..record.save
        };
        controls::hold_field(&self.address, held);
        controls::hold_field(&self.key, held);
        controls::hold_field(&self.model, held);
        for (_, check) in &self.declared {
            controls::hold_field(check, held);
        }
        if let Some(outcome) = record
            .outcome
            .filter(|outcome| outcome.serial > self.seen.get())
        {
            self.seen.set(outcome.serial);
            // The key went where it belongs, or with the endpoint: the field is empty again.
            if matches!(
                outcome.result,
                AiEndpointResult::Saved | AiEndpointResult::Removed
            ) {
                self.key.set_text("");
            }
        }
        let key_lost = record
            .outcome
            .is_some_and(|outcome| outcome.result == AiEndpointResult::KeyNotStored);
        self.failed.set_text(l10n::ai_endpoint_key_not_stored());
        self.failed.set_visible(key_lost);
        self.drawn.replace(Some(record));
        self.judge();
    }

    fn typed(&self) -> Typed {
        Typed {
            address: self.address.text().to_string(),
            model: self.model.text().to_string(),
            declared: self
                .declared
                .iter()
                .find(|(_, check)| check.is_active())
                .map(|(class, _)| *class),
        }
    }

    /// Says what a save of the fields as typed would refuse, under each field, and draws Save held
    /// while anything is named.
    fn judge(self: &Rc<Self>) {
        let Some(record) = self.drawn.borrow().clone() else {
            return;
        };
        let typed = self.typed();
        controls::form_unsent(&self.root, typed != *self.baseline.borrow());
        let gaps = (self.gap)(typed.address, typed.model, typed.declared);
        // An address not given yet is asked for with the example beside it.
        let missing = gaps.address == Some(mailcal_bindings::AiEndpointAddressGap::Missing);
        let asked = missing.then(|| {
            format!(
                "{} {}",
                l10n::ai_endpoint_gap_address_missing(),
                l10n::ai_endpoint_address_hint()
            )
        });
        say(
            &self.address_note,
            asked
                .as_deref()
                .or_else(|| gaps.address.map(words::address_gap)),
            Some(l10n::ai_endpoint_address_hint()),
            missing,
        );
        say(
            &self.model_note,
            gaps.model_missing.then(l10n::ai_endpoint_gap_model_missing),
            None,
            true,
        );
        say(
            &self.where_note,
            gaps.declaration.map(words::declaration_gap),
            None,
            gaps.declaration == Some(mailcal_bindings::AiEndpointDeclarationGap::Missing),
        );
        let complete = gaps.address.is_none() && !gaps.model_missing && gaps.declaration.is_none();
        let save = ControlState {
            enabled: record.save.enabled && complete,
            running: record.save.running,
        };
        if self.buttons_drawn.replace(Some((save, record.remove))) == Some((save, record.remove)) {
            return;
        }
        while let Some(child) = self.buttons.first_child() {
            self.buttons.remove(&child);
        }
        if record.remove.enabled || record.remove.running {
            let weak = Rc::downgrade(self);
            self.buttons.append(&controls::button(
                l10n::ai_endpoint_remove(),
                record.remove,
                true,
                move || {
                    if let Some(form) = weak.upgrade() {
                        (form.send)(AiEndpointIntent::Remove);
                    }
                },
            ));
        }
        let weak = Rc::downgrade(self);
        let button = controls::button(l10n::ai_endpoint_save(), save, false, move || {
            if let Some(form) = weak.upgrade() {
                form.save();
            }
        });
        button.add_css_class("suggested-action");
        self.buttons.append(&button);
    }

    fn save_on_return(self: &Rc<Self>) {
        let save = self
            .buttons
            .last_child()
            .and_downcast::<gtk::Button>()
            .filter(WidgetExt::is_sensitive);
        if let Some(save) = save {
            save.emit_clicked();
        }
    }

    /// Sends the fields as typed. An empty key field sends no key, which keeps the one stored.
    fn save(&self) {
        let typed = self.typed();
        let key = self.key.text().to_string();
        self.baseline.replace(typed.clone());
        controls::form_unsent(&self.root, false);
        (self.send)(AiEndpointIntent::Save {
            address: typed.address,
            model: typed.model,
            declared: typed.declared,
            key: (!key.is_empty()).then_some(key),
        });
    }
}

fn entry_row(title: &str) -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    row
}

fn note() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("caption");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label
}

/// One field: its rows in a boxed list, and its note under it.
fn field(rows: &[&gtk::Widget], note: &gtk::Label) -> gtk::Box {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for row in rows {
        list.append(*row);
    }
    let field = gtk::Box::new(gtk::Orientation::Vertical, 6);
    field.append(&list);
    field.append(note);
    field
}

/// Puts `gap` under its field, or `hint` when there is none. A field nobody has filled in yet is
/// said quietly; one that holds something the save would refuse, in the error colour.
fn say(note: &gtk::Label, gap: Option<&str>, hint: Option<&str>, quiet: bool) {
    note.remove_css_class("error");
    note.remove_css_class("dim-label");
    match (gap, hint) {
        (Some(gap), _) => {
            note.set_text(gap);
            note.add_css_class(if quiet { "dim-label" } else { "error" });
            note.set_visible(true);
        }
        (None, Some(hint)) => {
            note.set_text(hint);
            note.add_css_class("dim-label");
            note.set_visible(true);
        }
        (None, None) => note.set_visible(false),
    }
}

#[cfg(test)]
#[path = "ai_endpoint_tests.rs"]
pub(crate) mod tests;
