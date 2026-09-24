//! One writing style, shown back in plain words over six pages (`docs/ai.md`, "Learning" step 5):
//! what was read, a typical reply, greetings and sign-offs, tone and approach, phrases, then its
//! name, the person's own notes, and forgetting it.
//!
//! The same detail opens after a learning run and from a library row, so a style reads the same
//! the first time as every time after. The four pages about one language are redrawn when the
//! language control moves; the pages themselves are `writing_style_reveal_pages` and
//! `writing_style_reveal_cards`, the step logic `crate::ui::writing_style_reveal`.

use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::{accessible::Property, glib};
use mailcal_bindings::{LanguageStyleRow, WritingStyleDetail};

use super::{
    PageContext, group, pages,
    wizard::{Wizard, WizardPage},
    writing_style_reveal_cards as cards, writing_style_reveal_pages as reveal_pages,
};
use crate::{
    l10n,
    ui::writing_style_reveal::{RevealStep, WizardPager, shows_languages, source_address},
};

const NAME: &str = "writing-style-reveal";

/// The open detail. The Writing style page owns it; every handler here reaches it weakly.
pub(super) struct Reveal {
    ctx: PageContext,
    detail: WritingStyleDetail,
    wizard: Wizard,
    pager: Cell<WizardPager>,
    language: Cell<usize>,
    /// One page per step, in `RevealStep::ALL`'s order.
    pages: Vec<WizardPage>,
    /// The header's title, or the language control in its place.
    title: gtk::Stack,
    name: adw::EntryRow,
    notes: gtk::TextView,
}

/// Opens the style as a Settings detail. The caller keeps what this returns for as long as the
/// detail may be on screen.
pub(super) fn open(ctx: &PageContext, detail: &WritingStyleDetail) -> Rc<Reveal> {
    if let Some(previous) = ctx.navigation.child_by_name(NAME) {
        ctx.navigation.remove(&previous);
    }
    reveal_pages::install_styles();
    let wizard = Wizard::new(&detail.row.name);
    let pages = RevealStep::ALL
        .into_iter()
        .map(|step| WizardPage::new(step.title()))
        .collect::<Vec<_>>();
    for page in &pages {
        wizard.append(page);
    }
    let title = gtk::Stack::new();
    title.set_transition_type(gtk::StackTransitionType::Crossfade);
    title.add_named(&adw::WindowTitle::new(&detail.row.name, ""), Some("title"));
    wizard.header.set_title_widget(Some(&title));
    let name = adw::EntryRow::new();
    name.set_use_markup(false);
    name.set_title(l10n::writing_style_name_label());
    name.set_text(&detail.row.name);
    let reveal = Rc::new(Reveal {
        ctx: ctx.clone(),
        detail: detail.clone(),
        wizard,
        pager: Cell::new(WizardPager::new(RevealStep::ALL.len())),
        language: Cell::new(0),
        pages,
        title,
        name,
        notes: gtk::TextView::new(),
    });
    reveal.build();
    ctx.navigation.add_named(&reveal.wizard.root, Some(NAME));
    ctx.navigation.set_visible_child_name(NAME);
    reveal.show();
    reveal
}

impl Reveal {
    fn build(self: &Rc<Self>) {
        let zone = self.ctx.app.timezone_settings().active;
        let accounts = self.ctx.app.writing_styles().accounts;
        let address = source_address(&self.detail.row.source_account, &accounts);
        reveal_pages::read(
            &self.page(RevealStep::Read).body,
            &self.detail,
            address,
            &zone,
        );
        self.name_page();
        self.draw_language();
        if self.detail.languages.len() > 1 {
            self.title
                .add_named(&self.language_control(), Some("language"));
        }

        let weak = Rc::downgrade(self);
        self.wizard.cancel.connect_clicked(move |_| {
            if let Some(reveal) = weak.upgrade() {
                reveal.leave();
            }
        });
        let weak = Rc::downgrade(self);
        self.wizard.back.connect_clicked(move |_| {
            if let Some(reveal) = weak.upgrade() {
                reveal.turn(WizardPager::back);
            }
        });
        let weak = Rc::downgrade(self);
        self.wizard.primary.connect_clicked(move |_| {
            let Some(reveal) = weak.upgrade() else {
                return;
            };
            if reveal.pager.get().is_last() {
                reveal.save();
            } else {
                reveal.turn(WizardPager::next);
            }
        });
        // A style with no name is a picker entry nobody can tell apart, so Save waits for one.
        let weak = Rc::downgrade(self);
        self.name.connect_changed(move |_| {
            if let Some(reveal) = weak.upgrade() {
                reveal.show();
            }
        });
    }

    fn page(&self, step: RevealStep) -> &WizardPage {
        let index = RevealStep::ALL
            .iter()
            .position(|candidate| *candidate == step)
            .unwrap_or_default();
        &self.pages[index]
    }

    fn step(&self) -> RevealStep {
        RevealStep::ALL[self.pager.get().index()]
    }

    fn turn(&self, move_to: fn(&mut WizardPager)) {
        let mut pager = self.pager.get();
        move_to(&mut pager);
        self.pager.set(pager);
        self.show();
    }

    /// Brings the frame to the page on screen: the dots, Back, Next or Save, and the language
    /// control over a page about one language.
    fn show(&self) {
        let pager = self.pager.get();
        self.wizard.show(pager);
        if pager.is_last() {
            let named = !self.name.text().trim().is_empty();
            self.wizard.set_primary(l10n::reveal_save(), true, named);
        } else {
            self.wizard.set_primary(l10n::wizard_next(), true, true);
        }
        let languages = shows_languages(self.step(), self.detail.languages.len());
        self.title
            .set_visible_child_name(if languages { "language" } else { "title" });
    }

    /// The control that picks the language pages 2 to 5 are about: one toggle per language, named
    /// in its own language.
    fn language_control(self: &Rc<Self>) -> gtk::Box {
        let control = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .accessible_role(gtk::AccessibleRole::Group)
            .build();
        control.add_css_class("linked");
        control.update_property(&[Property::Label(l10n::a11y_reveal_language())]);
        let mut first: Option<gtk::ToggleButton> = None;
        for (index, language) in self.detail.languages.iter().enumerate() {
            let toggle = gtk::ToggleButton::with_label(&l10n::language_name(&language.language));
            if let Some(first) = &first {
                toggle.set_group(Some(first));
            } else {
                toggle.set_active(true);
                first = Some(toggle.clone());
            }
            let weak = Rc::downgrade(self);
            toggle.connect_toggled(move |toggle| {
                if toggle.is_active() {
                    choose_language(&weak, index);
                }
            });
            control.append(&toggle);
        }
        control
    }

    /// Redraws the four pages about one language for the language chosen.
    fn draw_language(&self) {
        let Some(style) = self.detail.languages.get(self.language.get()) else {
            return;
        };
        let steps: [(RevealStep, fn(&gtk::Box, &LanguageStyleRow)); 4] = [
            (RevealStep::Letter, reveal_pages::letter),
            (RevealStep::Habits, reveal_pages::habits),
            (RevealStep::Voice, cards::voice),
            (RevealStep::Phrases, cards::phrases),
        ];
        for (step, draw) in steps {
            let page = self.page(step);
            page.clear();
            draw(&page.body, style);
        }
    }

    /// Step 6: the name, the person's own notes, and forgetting the style.
    fn name_page(self: &Rc<Self>) {
        let body = &self.page(RevealStep::Name).body;
        let names = adw::PreferencesGroup::new();
        names.add(&self.name);
        body.append(&names);

        let notes_group = group(
            l10n::writing_style_notes(),
            l10n::writing_style_notes_hint(),
        );
        let notes = &self.notes;
        notes.set_wrap_mode(gtk::WrapMode::WordChar);
        notes.set_accepts_tab(false);
        notes.set_top_margin(8);
        notes.set_bottom_margin(8);
        notes.set_left_margin(8);
        notes.set_right_margin(8);
        notes.buffer().set_text(&self.detail.notes);
        notes.update_property(&[Property::Label(l10n::writing_style_notes())]);
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(notes));
        frame.set_size_request(-1, 120);
        notes_group.add(&frame);
        body.append(&notes_group);

        let forget = gtk::Button::with_label(l10n::writing_style_forget());
        forget.add_css_class("destructive-action");
        forget.set_halign(gtk::Align::Start);
        forget.set_margin_top(12);
        let weak = Rc::downgrade(self);
        forget.connect_clicked(move |_| {
            if let Some(reveal) = weak.upgrade() {
                confirm_forget(&reveal.ctx, &reveal.detail.row.id, &reveal.detail.row.name);
            }
        });
        body.append(&forget);
    }

    /// Save stores what changed and only that: a rename, the notes, or both.
    fn save(&self) {
        let chosen = self.name.text().trim().to_owned();
        if chosen.is_empty() {
            return;
        }
        let id = &self.detail.row.id;
        if chosen != self.detail.row.name {
            self.ctx.app.rename_writing_style(id.clone(), chosen);
        }
        let buffer = self.notes.buffer();
        let (start, end) = buffer.bounds();
        let written = buffer.text(&start, &end, false).to_string();
        if written != self.detail.notes {
            self.ctx.app.update_writing_style_notes(id.clone(), written);
        }
        self.leave();
    }

    fn leave(&self) {
        leave(&self.ctx.navigation.downgrade());
    }
}

/// Draws the pages about one language for another, once it is a different one.
fn choose_language(reveal: &Weak<Reveal>, index: usize) {
    if let Some(reveal) = reveal.upgrade()
        && reveal.language.replace(index) != index
    {
        reveal.draw_language();
    }
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
