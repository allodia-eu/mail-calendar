//! A new style shown back once (`docs/ai.md` rule 15), as a step of the flow host: the pages the
//! core's `writing_style_reveal` lists, in a carousel the buttons move, a language control over a
//! page about one language, and the name and notes with Save on the last.
//!
//! It keeps which page and which language are on screen, and what is typed into the name and the
//! notes; everything else is the record's.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::{
    ControlState, WritingStyleIntent, WritingStyleLanguage, WritingStyleOutcome,
    WritingStyleResult, WritingStyleReveal, WritingStyleRevealPage,
};

use super::{
    writing_style_reveal_cards::section, writing_style_reveal_pages as pages,
    writing_style_stages::column, writing_style_words as words,
};
use crate::{
    l10n,
    ui::{
        controls,
        flow_host::{FlowAction, FlowLeave, FlowStep},
        flow_pager::FlowPager,
    },
};

pub(super) type Send = Rc<dyn Fn(WritingStyleIntent)>;

pub(super) struct Reveal {
    pub(super) id: String,
    record: RefCell<WritingStyleReveal>,
    pager: Rc<FlowPager>,
    language: Cell<usize>,
    /// The body of each page about one language, drawn again when the language moves.
    per_language: Vec<(WritingStyleRevealPage, gtk::Box)>,
    language_control: Option<gtk::Box>,
    name: adw::EntryRow,
    notes: gtk::TextView,
    refused: gtk::Label,
    send: Send,
    /// Asks the owner to draw the step again: a page turned or the name moved.
    reshow: RefCell<Option<Rc<dyn Fn()>>>,
    /// The newest outcome acted on.
    seen: Cell<u64>,
}

impl Reveal {
    /// The reveal of `record`, acting on outcomes newer than `seen`.
    pub(super) fn new(record: WritingStyleReveal, seen: u64, send: Send) -> Rc<Self> {
        pages::install_styles();
        let name = adw::EntryRow::new();
        name.set_use_markup(false);
        name.set_title(l10n::writing_style_name_label());
        name.set_text(&record.name);
        let notes = gtk::TextView::new();
        notes.buffer().set_text(&record.notes);
        let refused = pages::text("");
        refused.add_css_class("error");
        refused.set_visible(false);
        let mut built = Vec::new();
        let mut per_language = Vec::new();
        for page in &record.pages {
            let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
            match page {
                WritingStyleRevealPage::Read => pages::read(&body, &record.facts, true),
                WritingStyleRevealPage::Name => name_page(&body, &name, &notes, &refused),
                other => per_language.push((*other, body.clone())),
            }
            built.push(titled(page_title(*page), &body));
        }
        let pager = FlowPager::new(&built);
        let reveal = Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let language_control =
                (record.languages.len() > 1).then(|| language_control(&record.languages, weak));
            Self {
                id: record.id.clone(),
                record: RefCell::new(record),
                pager: Rc::clone(&pager),
                language: Cell::new(0),
                per_language,
                language_control,
                name,
                notes,
                refused,
                send,
                reshow: RefCell::new(None),
                seen: Cell::new(seen),
            }
        });
        reveal.draw_language();
        let weak = Rc::downgrade(&reveal);
        reveal.name.connect_changed(move |_| {
            if let Some(reveal) = weak.upgrade() {
                reveal.reshow();
            }
        });
        let weak = Rc::downgrade(&reveal);
        reveal.pager.back().connect_clicked(move |_| {
            if let Some(reveal) = weak.upgrade() {
                reveal.reshow();
            }
        });
        reveal
    }

    /// Who draws the step again when it moves.
    pub(super) fn on_reshow(&self, reshow: Rc<dyn Fn()>) {
        self.reshow.replace(Some(reshow));
    }

    fn reshow(&self) {
        let reshow = self.reshow.borrow().clone();
        if let Some(reshow) = reshow {
            reshow();
        }
    }

    /// Takes a newer read of the record: its Save and Cancel move, its pages do not.
    pub(super) fn update(&self, record: WritingStyleReveal) {
        self.record.replace(record);
    }

    /// Acts on `outcome` once: whether the reveal is done with. A refusal stays and says why.
    pub(super) fn outcome(&self, outcome: Option<&WritingStyleOutcome>) -> bool {
        let Some(outcome) = outcome.filter(|outcome| {
            outcome.serial > self.seen.get() && outcome.style.as_deref() == Some(&self.id)
        }) else {
            return false;
        };
        self.seen.set(outcome.serial);
        match outcome.result {
            WritingStyleResult::Refused { reason } => {
                self.refused.set_text(words::refusal(reason));
                self.refused.set_visible(true);
                false
            }
            WritingStyleResult::Done | WritingStyleResult::NotWritten => true,
        }
    }

    /// The step the host draws, on the page on screen.
    pub(super) fn step(self: &Rc<Self>) -> FlowStep {
        let record = self.record.borrow();
        controls::hold_field(self.pager.back(), record.paging);
        let on = record
            .pages
            .get(usize::try_from(self.pager.index()).unwrap_or_default())
            .copied()
            .unwrap_or(WritingStyleRevealPage::Read);
        let about_language = !matches!(
            on,
            WritingStyleRevealPage::Read | WritingStyleRevealPage::Name
        );
        let primary = if self.pager.is_last() {
            let weak = Rc::downgrade(self);
            FlowAction {
                label: l10n::reveal_save().to_owned(),
                control: save_control(record.save, &self.name.text()),
                suggested: true,
                pressed: Rc::new(move || {
                    if let Some(reveal) = weak.upgrade() {
                        reveal.save();
                    }
                }),
            }
        } else {
            let weak = Rc::downgrade(self);
            FlowAction {
                label: l10n::wizard_next().to_owned(),
                control: record.paging,
                suggested: true,
                pressed: Rc::new(move || {
                    if let Some(reveal) = weak.upgrade() {
                        reveal.pager.turn(true);
                        reveal.reshow();
                    }
                }),
            }
        };
        FlowStep {
            tag: "reveal",
            title: record.name.clone(),
            title_widget: self
                .language_control
                .clone()
                .filter(|_| about_language)
                .map(gtk::Box::upcast),
            content: self.pager.carousel().clone().upcast(),
            back: None,
            leave: FlowLeave {
                label: l10n::action_close().to_owned(),
                control: record.close,
            },
            own_back: Some(self.pager.back().clone()),
            primary: Some(primary),
            footer: Some(self.pager.footer().clone().upcast()),
        }
    }

    fn save(&self) {
        let buffer = self.notes.buffer();
        let (start, end) = buffer.bounds();
        self.refused.set_visible(false);
        (self.send)(WritingStyleIntent::SaveReveal {
            style: self.id.clone(),
            name: self.name.text().to_string(),
            notes: buffer.text(&start, &end, false).to_string(),
        });
    }

    fn choose(&self, index: usize) {
        if self.language.replace(index) != index {
            self.draw_language();
        }
    }

    fn draw_language(&self) {
        let record = self.record.borrow();
        let Some(language) = record.languages.get(self.language.get()) else {
            return;
        };
        for (page, body) in &self.per_language {
            while let Some(child) = body.first_child() {
                body.remove(&child);
            }
            section(body, *page, language);
        }
    }
}

/// Save as drawn: the record's, held besides while the core refuses the name typed.
pub(super) fn save_control(save: ControlState, name: &str) -> ControlState {
    ControlState {
        enabled: save.enabled
            && mailcal_bindings::writing_style_name_refusal(name.to_owned()).is_none(),
        running: save.running,
    }
}

fn page_title(page: WritingStyleRevealPage) -> &'static str {
    match page {
        WritingStyleRevealPage::Read => l10n::reveal_title(),
        WritingStyleRevealPage::Letter => l10n::reveal_step_letter(),
        WritingStyleRevealPage::Habits => l10n::reveal_step_habits(),
        WritingStyleRevealPage::Voice => l10n::reveal_step_voice(),
        WritingStyleRevealPage::Phrases => l10n::reveal_phrases(),
        WritingStyleRevealPage::Name => l10n::reveal_step_name(),
    }
}

/// A page: its heading over its body.
fn titled(title: &str, body: &gtk::Box) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let heading = pages::text(title);
    heading.add_css_class("title-2");
    heading.set_accessible_role(gtk::AccessibleRole::Heading);
    content.append(&heading);
    content.append(body);
    column(&content)
}

/// The control that picks the language the pages about one language show: a toggle per
/// language, named in its own language.
fn language_control(
    languages: &[WritingStyleLanguage],
    reveal: &std::rc::Weak<Reveal>,
) -> gtk::Box {
    let control = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    control.add_css_class("linked");
    control.set_accessible_role(gtk::AccessibleRole::Group);
    control.update_property(&[gtk::accessible::Property::Label(
        l10n::a11y_reveal_language(),
    )]);
    let mut first: Option<gtk::ToggleButton> = None;
    for (index, language) in languages.iter().enumerate() {
        let toggle = gtk::ToggleButton::with_label(&l10n::language_name(&language.language));
        if let Some(anchor) = &first {
            toggle.set_group(Some(anchor));
        } else {
            toggle.set_active(true);
            first = Some(toggle.clone());
        }
        let reveal = reveal.clone();
        toggle.connect_toggled(move |toggle| {
            if toggle.is_active()
                && let Some(reveal) = reveal.upgrade()
            {
                reveal.choose(index);
            }
        });
        control.append(&toggle);
    }
    control
}

/// The last page: the name, the person's notes, and what a refused save said.
fn name_page(body: &gtk::Box, name: &adw::EntryRow, notes: &gtk::TextView, refused: &gtk::Label) {
    let names = gtk::ListBox::new();
    names.add_css_class("boxed-list");
    names.set_selection_mode(gtk::SelectionMode::None);
    names.append(name);
    body.append(&names);
    let heading = pages::heading(l10n::writing_style_notes());
    let hint = pages::text(l10n::writing_style_notes_hint());
    hint.add_css_class("dim-label");
    notes.set_wrap_mode(gtk::WrapMode::WordChar);
    notes.set_accepts_tab(false);
    notes.set_top_margin(8);
    notes.set_bottom_margin(8);
    notes.set_left_margin(10);
    notes.set_right_margin(10);
    notes.set_size_request(-1, 120);
    let frame = gtk::Frame::new(None);
    frame.set_child(Some(notes));
    notes.update_property(&[gtk::accessible::Property::Label(l10n::writing_style_notes())]);
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    group.append(&heading);
    group.append(&hint);
    group.append(&frame);
    body.append(&group);
    body.append(refused);
}
