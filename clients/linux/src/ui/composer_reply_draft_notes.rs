//! What Draft a reply says among the composer's notes (`docs/ai.md` rules 18, 20 and 23): the draft
//! being made with Stop, the card a draft brings (what the message asks, then what is left to do,
//! each item ticked as the core says), a draft that did not come with its remedy, and a reason the
//! person cannot act on. Native widgets beside the editor, so none of it is ever part of the mail.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, ComposerReplyDraft, ReplyDraftItem, ReplyDraftState,
    ReplyDraftTaskKind,
};

use super::{controls, icons, mailbox::plain_text_row, settings::writing_style_words as words};
use crate::l10n;

/// What the notes' controls ask for.
pub(crate) struct Actions {
    pub(crate) stop: Rc<dyn Fn()>,
    pub(crate) tick: Rc<dyn Fn(u32, bool)>,
    pub(crate) remedy: Rc<dyn Fn(AiRemedy)>,
}

/// Draws `record`'s notes into `notes`, in place of what was there.
pub(crate) fn draw(notes: &gtk::Box, record: &ComposerReplyDraft, actions: &Actions) {
    while let Some(child) = notes.first_child() {
        notes.remove(&child);
    }
    match &record.state {
        ReplyDraftState::Drafting => {
            let row = plain_text_row();
            // No spinner: Draft a reply in the action row runs meanwhile, and one is enough.
            row.set_title(l10n::composer_drafting());
            let stop = Rc::clone(&actions.stop);
            row.add_suffix(&controls::button(
                l10n::composer_draft_stop(),
                record.stop,
                false,
                move || stop(),
            ));
            notes.append(&boxed(&row));
        }
        ReplyDraftState::Drafted { summary, checklist } => {
            if !summary.is_empty() || !checklist.is_empty() {
                notes.append(&card(summary, checklist, actions));
            }
        }
        ReplyDraftState::Failed { failure, remedy } => {
            notes.append(&boxed(&failure_row(*failure, *remedy, actions)));
        }
        // A reason the person can act on is said in the input the control opens; one they cannot
        // comes with the control held, and is a line here.
        ReplyDraftState::Unavailable { failure, remedy } if !record.control.enabled => {
            notes.append(&boxed(&failure_row(*failure, *remedy, actions)));
        }
        ReplyDraftState::Unavailable { .. } | ReplyDraftState::Ready => {}
    }
    notes.set_visible(notes.first_child().is_some());
}

fn boxed(row: &impl IsA<gtk::Widget>) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.append(row);
    list
}

fn failure_row(
    failure: AiFailure,
    remedy: Option<AiRemedyControl>,
    actions: &Actions,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(words::failure(failure));
    let mark = gtk::Image::from_icon_name(icons::WARNING);
    mark.add_css_class("warning");
    row.add_prefix(&mark);
    if let Some(remedy) = remedy {
        let open = Rc::clone(&actions.remedy);
        row.add_suffix(&controls::button(
            words::remedy(remedy.remedy),
            remedy.control,
            false,
            move || open(remedy.remedy),
        ));
    }
    row
}

/// The card: what the message asks, then what is left to do before sending.
fn card(summary: &str, checklist: &[ReplyDraftItem], actions: &Actions) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("card");
    card.add_css_class("mailcal-draft-card");
    card.set_accessible_role(gtk::AccessibleRole::Group);
    card.update_property(&[gtk::accessible::Property::Label(
        l10n::composer_draft_summary_heading(),
    )]);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(14);
    inner.set_margin_end(14);
    if !summary.is_empty() {
        inner.append(&heading(l10n::composer_draft_summary_heading()));
        let said = gtk::Label::new(Some(summary));
        said.set_wrap(true);
        said.set_xalign(0.0);
        inner.append(&said);
    }
    if !checklist.is_empty() {
        let list = heading(l10n::composer_checklist_heading());
        if !summary.is_empty() {
            list.set_margin_top(6);
        }
        inner.append(&list);
        for item in checklist {
            inner.append(&item_check(item, actions));
        }
    }
    card.append(&inner);
    card
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    label.add_css_class("heading");
    label
}

/// One item, worded by its kind and ticked as the core says. A gap has no tick, since the reply
/// decides it: its state is a mark beside the text that cannot be pressed, never a checkbox held
/// (`docs/ai.md` rule 23).
fn item_check(item: &ReplyDraftItem, actions: &Actions) -> gtk::Widget {
    let said = match item.kind {
        ReplyDraftTaskKind::FillIn => l10n::composer_task_fill_in(&item.text),
        ReplyDraftTaskKind::Attach | ReplyDraftTaskKind::Do => item.text.clone(),
    };
    let Some(control) = item.tick else {
        return gap_mark(&said, item.done).upcast();
    };
    let (tick, index) = (Rc::clone(&actions.tick), item.index);
    controls::check(&said, item.done, control, move |done| tick(index, done)).upcast()
}

/// A gap's state: a check once its text is gone from the reply, an open circle while it is there.
/// To a screen reader it is a state that can be read and not changed.
pub(super) fn gap_mark(said: &str, done: bool) -> gtk::Box {
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .accessible_role(gtk::AccessibleRole::Checkbox)
        .build();
    line.set_widget_name(GAP_MARK);
    line.set_margin_top(4);
    line.set_margin_bottom(4);
    line.update_property(&[
        gtk::accessible::Property::Label(said),
        gtk::accessible::Property::ReadOnly(true),
    ]);
    line.update_state(&[gtk::accessible::State::Checked(if done {
        gtk::AccessibleTristate::True
    } else {
        gtk::AccessibleTristate::False
    })]);
    let mark = gtk::Image::builder()
        .icon_name(if done {
            icons::GAP_FILLED
        } else {
            icons::GAP_OPEN
        })
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    mark.add_css_class(if done { "success" } else { "dim-label" });
    let text = gtk::Label::new(Some(said));
    text.set_wrap(true);
    text.set_xalign(0.0);
    text.update_state(&[gtk::accessible::State::Hidden(true)]);
    line.append(&mark);
    line.append(&text);
    line
}

/// The widget name a gap's mark carries, for a test to find it by.
pub(super) const GAP_MARK: &str = "reply-draft-gap";
