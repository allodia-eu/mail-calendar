//! The instruction's input (`docs/ai.md` rules 18 and 19), in the popover from Draft a reply: one
//! line holding the last instruction, the core's suggestions under it, which fill the line, and
//! Draft. Where the core says it cannot draft, the reason and its one remedy stand in the line's
//! place.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{AiRemedy, ComposerReplyDraft, ReplyDraftSuggestion};

use super::{controls, settings::writing_style_words as words};
use crate::l10n;

/// What the input's controls ask for.
pub(crate) struct Actions {
    pub(crate) draft: Rc<dyn Fn(String)>,
    pub(crate) remedy: Rc<dyn Fn(AiRemedy)>,
}

pub(crate) struct Input {
    root: gtk::Box,
    entry: Option<gtk::Entry>,
    /// Where Draft stands, drawn again when its control moves.
    draft_slot: gtk::Box,
    draft: Rc<dyn Fn(String)>,
}

impl Input {
    pub(crate) fn new(record: &ComposerReplyDraft, actions: Actions) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_start(6);
        root.set_margin_end(6);
        root.set_margin_top(6);
        root.set_margin_bottom(6);
        root.set_width_request(340);
        let draft_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        draft_slot.set_halign(gtk::Align::End);
        let entry = if let Some(hold) = record.held_by {
            let reason = gtk::Label::new(Some(words::failure(hold.failure)));
            reason.set_wrap(true);
            reason.set_xalign(0.0);
            reason.set_max_width_chars(40);
            root.append(&reason);
            if let Some(remedy) = hold.remedy {
                let open = Rc::clone(&actions.remedy);
                let button = controls::button(
                    words::remedy(remedy.remedy),
                    remedy.control,
                    false,
                    move || open(remedy.remedy),
                );
                button.add_css_class("suggested-action");
                button.set_halign(gtk::Align::End);
                root.append(&button);
            }
            None
        } else {
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some(l10n::composer_draft_intent_hint()));
            entry.set_text(&record.instruction);
            entry.update_property(&[gtk::accessible::Property::Label(
                l10n::composer_draft_intent_hint(),
            )]);
            root.append(&entry);
            root.append(&suggestions(&record.suggestions, &entry));
            root.append(&draft_slot);
            Some(entry)
        };
        let input = Rc::new(Self {
            root,
            entry,
            draft_slot,
            draft: actions.draft,
        });
        if let Some(entry) = &input.entry {
            let weak = Rc::downgrade(&input);
            entry.connect_activate(move |_| {
                if let Some(input) = weak.upgrade() {
                    input.press();
                }
            });
            entry.grab_focus();
        }
        input.update(record);
        input
    }

    pub(crate) fn widget(&self) -> gtk::Widget {
        // No wider than a line of instruction reads well, however long the suggestions run.
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(420);
        clamp.set_child(Some(&self.root));
        clamp.upcast()
    }

    /// Draws Draft from the record.
    pub(crate) fn update(self: &Rc<Self>, record: &ComposerReplyDraft) {
        if self.entry.is_none() {
            return;
        }
        while let Some(child) = self.draft_slot.first_child() {
            self.draft_slot.remove(&child);
        }
        let weak = Rc::downgrade(self);
        let button = controls::button(
            l10n::composer_draft_create(),
            record.draft,
            false,
            move || {
                if let Some(input) = weak.upgrade() {
                    input.send();
                }
            },
        );
        button.add_css_class("suggested-action");
        self.draft_slot.append(&button);
    }

    /// Return in the line: Draft, when Draft may be pressed.
    fn press(&self) {
        let button = self
            .draft_slot
            .first_child()
            .and_downcast::<gtk::Button>()
            .filter(WidgetExt::is_sensitive);
        if let Some(button) = button {
            button.emit_clicked();
        }
    }

    fn send(&self) {
        if let Some(entry) = &self.entry {
            (self.draft)(entry.text().to_string());
        }
    }
}

/// The core's suggestions, each a button whose words fill the line.
fn suggestions(offered: &[ReplyDraftSuggestion], entry: &gtk::Entry) -> adw::WrapBox {
    let row = adw::WrapBox::new();
    row.set_child_spacing(6);
    row.set_line_spacing(6);
    for suggestion in offered {
        let said = suggestion_words(*suggestion);
        let button = gtk::Button::with_label(said);
        let entry = entry.clone();
        button.connect_clicked(move |_| {
            entry.set_text(said);
            entry.grab_focus();
            entry.set_position(-1);
        });
        row.append(&button);
    }
    row
}

pub(crate) fn suggestion_words(suggestion: ReplyDraftSuggestion) -> &'static str {
    match suggestion {
        ReplyDraftSuggestion::Yes => l10n::composer_draft_chip_yes(),
        ReplyDraftSuggestion::No => l10n::composer_draft_chip_no(),
        ReplyDraftSuggestion::MoreInformation => l10n::composer_draft_chip_more_info(),
        ReplyDraftSuggestion::Holding => l10n::composer_draft_chip_later(),
    }
}
