//! The card a drafted reply brings (`docs/ai.md`, "Where they show"): what the message asks and
//! what is left to do, between the composer's buttons and the editor, and the one question Send
//! asks while something on it is open.
//!
//! Native widgets beside the editor, so the card is never part of the mail. What it says is
//! `super::draft_checklist`; the control that owns it and follows the editor is
//! `super::composer_draft_reply`.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{
    accessible::{Property, State},
    pango,
};
use mailcal_bindings::DraftTaskKind;

use super::draft_checklist::{ChecklistItem, DraftChecklist};
use crate::l10n;

/// Told which item the person ticked or unticked, and how it now stands.
pub(super) type OnTick = Rc<dyn Fn(usize, bool)>;

pub(super) struct DraftCard {
    root: gtk::Box,
    expander: gtk::Expander,
    heading: gtk::Label,
    summary: gtk::Label,
    checklist_heading: gtk::Label,
    rows: gtk::Box,
    /// Each item's check and title, in the checklist's order.
    items: RefCell<Vec<(gtk::CheckButton, gtk::Label)>>,
}

impl DraftCard {
    /// Built hidden: a draft with nothing to say shows no card.
    pub(super) fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("card");
        root.set_visible(false);
        let heading = gtk::Label::new(None);
        heading.add_css_class("heading");
        let expander = gtk::Expander::new(None);
        expander.set_label_widget(Some(&heading));
        expander.set_expanded(true);
        expander.set_margin_top(10);
        expander.set_margin_bottom(10);
        expander.set_margin_start(12);
        expander.set_margin_end(12);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(8);
        let summary = wrapped();
        let checklist_heading = gtk::Label::builder()
            .accessible_role(gtk::AccessibleRole::Heading)
            .label(l10n::composer_checklist_heading())
            .xalign(0.0)
            .build();
        checklist_heading.add_css_class("heading");
        checklist_heading.set_margin_top(4);
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.append(&summary);
        content.append(&checklist_heading);
        content.append(&rows);
        expander.set_child(Some(&content));
        root.append(&expander);
        Self {
            root,
            expander,
            heading,
            summary,
            checklist_heading,
            rows,
            items: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replaces the card with a draft's. The person's collapse or expand stays as it was.
    pub(super) fn show(&self, checklist: &DraftChecklist, on_tick: &OnTick) {
        let summary = checklist.summary();
        let heading = if summary.is_empty() {
            l10n::composer_checklist_heading()
        } else {
            l10n::composer_draft_summary_heading()
        };
        self.heading.set_text(heading);
        // Named by its heading and described by the summary: a button's name and description
        // are otherwise read from everything inside it, which here is the whole card.
        self.expander
            .update_property(&[Property::Label(heading), Property::Description(summary)]);
        self.summary.set_text(summary);
        self.summary.set_visible(!summary.is_empty());
        self.checklist_heading
            .set_visible(!summary.is_empty() && !checklist.items().is_empty());
        while let Some(child) = self.rows.first_child() {
            self.rows.remove(&child);
        }
        let mut items = self.items.borrow_mut();
        items.clear();
        for (index, item) in checklist.items().iter().enumerate() {
            let (check, title) = row(item);
            let on_tick = Rc::clone(on_tick);
            check.connect_toggled(move |check| on_tick(index, check.is_active()));
            self.rows.append(&check);
            items.push((check, title));
        }
        drop(items);
        self.refresh(checklist);
        self.root.set_visible(!checklist.is_empty());
    }

    /// Brings every row to the checklist's ticks without rebuilding it. A check set here to what
    /// the checklist already says tells `on_tick` nothing new.
    pub(super) fn refresh(&self, checklist: &DraftChecklist) {
        for ((check, title), item) in self.items.borrow().iter().zip(checklist.items()) {
            if check.is_active() != item.ticked {
                check.set_active(item.ticked);
            }
            let struck = item.ticked.then(struck_through);
            title.set_attributes(struck.as_ref());
            if item.ticked {
                title.add_css_class("dim-label");
            } else {
                title.remove_css_class("dim-label");
            }
        }
    }
}

/// One item: a check, an icon for its kind, and what it says. A fill-in item ticks itself as the
/// reply changes, so it is shown and never taken by a click or the keyboard.
fn row(item: &ChecklistItem) -> (gtk::CheckButton, gtk::Label) {
    let title = wrapped();
    title.set_text(&item.title());
    let icon = gtk::Image::from_icon_name(match item.kind {
        DraftTaskKind::FillIn => super::icons::TASK_FILL_IN,
        DraftTaskKind::Attach => super::icons::TASK_ATTACH,
        DraftTaskKind::Do => super::icons::TASK_DO,
    });
    icon.add_css_class("dim-label");
    icon.set_valign(gtk::Align::Start);
    icon.set_margin_top(2);
    icon.update_state(&[State::Hidden(true)]);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&icon);
    content.append(&title);
    let check = gtk::CheckButton::new();
    check.set_child(Some(&content));
    check.update_property(&[Property::Label(&item.title())]);
    match item.kind {
        DraftTaskKind::Attach => {
            check.update_property(&[Property::Description(l10n::a11y_task_attach())]);
        }
        DraftTaskKind::Do => {
            check.update_property(&[Property::Description(l10n::a11y_task_do())]);
        }
        // `can-focus`, not `focusable`: a check button that cannot take focus still claims it
        // when Tab arrives, so the card's expander above it would keep the keyboard for good.
        DraftTaskKind::FillIn => {
            check.set_can_target(false);
            check.set_can_focus(false);
        }
    }
    (check, title)
}

fn wrapped() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label
}

fn struck_through() -> pango::AttrList {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrInt::new_strikethrough(true));
    attributes
}

/// Send's one question while an item is open: send anyway, or keep editing. Neither answer stops
/// a later Send.
pub(super) fn confirm_send(parent: &gtk::Window, open: usize, send: Rc<dyn Fn()>) {
    let (dialog, _) = super::modal::new(parent, l10n::composer_send_open_title(), 420, None);
    dialog.set_resizable(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let message = wrapped();
    message.set_text(&l10n::composer_send_open_message(
        i64::try_from(open).unwrap_or(i64::MAX),
    ));
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let keep = gtk::Button::with_label(l10n::composer_keep_editing());
    let window = dialog.clone();
    keep.connect_clicked(move |_| window.close());
    actions.append(&keep);
    let anyway = gtk::Button::with_label(l10n::composer_send_anyway());
    anyway.add_css_class("suggested-action");
    let window = dialog.clone();
    anyway.connect_clicked(move |_| {
        window.close();
        send();
    });
    actions.append(&anyway);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.present();
}
