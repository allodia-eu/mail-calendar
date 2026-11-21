//! What Settings → Writing style draws from the core's `writing_style_page` (`docs/ai.md` rules 8
//! to 10): the library, Learn or the run in its place, why Learn cannot be pressed with its one
//! remedy, and a slot picker per account. Every control is drawn through [`controls`].

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, WritingStyleEntry, WritingStyleLearnBlock,
    WritingStyleOpens, WritingStylePage, WritingStyleRun, WritingStyleSlot,
};

use super::{group, writing_style_words as words};
use crate::{
    l10n,
    ui::{controls, icons, mailbox::plain_text_row},
};

/// What the page's controls ask for.
pub(super) struct Actions {
    pub(super) open: Rc<dyn Fn(String, WritingStyleOpens)>,
    pub(super) assign: Rc<dyn Fn(String, Option<String>)>,
    pub(super) learn: Rc<dyn Fn()>,
    pub(super) stop: Rc<dyn Fn()>,
    /// A remedy for why Learn cannot be pressed: there is no flow, so it only opens where it
    /// leads.
    pub(super) remedy: Rc<dyn Fn(AiRemedy)>,
    /// The remedy of a run that failed, which the flow takes first (`docs/ai.md` rule 11).
    pub(super) run_remedy: Rc<dyn Fn(AiRemedy)>,
}

/// Draws `page` into `body`, saying `refused` when the last change on it was refused.
pub(super) fn draw(
    body: &gtk::Box,
    page: &WritingStylePage,
    refused: Option<&str>,
    actions: &Actions,
) {
    let intro = gtk::Label::new(Some(l10n::writing_style_intro()));
    intro.set_wrap(true);
    intro.set_xalign(0.0);
    intro.add_css_class("dim-label");
    body.append(&intro);
    if let Some(refused) = refused {
        let said = gtk::Label::new(Some(refused));
        said.set_wrap(true);
        said.set_xalign(0.0);
        said.add_css_class("error");
        body.append(&said);
    }

    let library = adw::PreferencesGroup::new();
    if page.styles.is_empty() {
        let empty = plain_text_row();
        empty.set_title(l10n::writing_style_empty());
        library.add(&empty);
    }
    for entry in &page.styles {
        library.add(&style_row(entry, actions));
    }
    body.append(&library);
    body.append(&learn_area(page, actions));

    if !page.accounts.is_empty() {
        let slots = group(l10n::writing_style_accounts_heading(), "");
        let names = std::iter::once(l10n::writing_style_none())
            .chain(page.styles.iter().map(|entry| entry.name.as_str()))
            .collect::<Vec<_>>();
        let ids = page
            .styles
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Rc<[_]>>();
        for slot in &page.accounts {
            slots.add(&slot_row(slot, &names, &ids, actions));
        }
        body.append(&slots);
    }
}

fn style_row(entry: &WritingStyleEntry, actions: &Actions) -> adw::ActionRow {
    let (open, id, opens) = (Rc::clone(&actions.open), entry.id.clone(), entry.opens);
    controls::nav_row(
        &entry.name,
        &words::facts(&entry.facts),
        entry.open.enabled,
        move || {
            open(id.clone(), opens);
        },
    )
}

/// Learn, the run drawn in its place, or why it cannot be pressed.
fn learn_area(page: &WritingStylePage, actions: &Actions) -> gtk::Box {
    let area = gtk::Box::new(gtk::Orientation::Vertical, 12);
    match &page.run {
        Some(WritingStyleRun::Learning {
            email,
            done,
            total,
            stop,
        }) => {
            let list = boxed();
            let row = plain_text_row();
            row.set_title(&words::progress(*done, *total));
            row.set_subtitle(&l10n::learn_progress_account(email));
            let spinner = adw::Spinner::new();
            row.add_prefix(&spinner);
            let send = Rc::clone(&actions.stop);
            row.add_suffix(&controls::button(
                l10n::learn_stop(),
                *stop,
                false,
                move || send(),
            ));
            list.append(&row);
            area.append(&list);
            return area;
        }
        Some(WritingStyleRun::Failed { failure, remedy }) => {
            area.append(&failed_row(*failure, *remedy, actions));
        }
        None => {}
    }
    if let Some(block) = page.learn_blocked {
        let (said, remedy) = match block {
            WritingStyleLearnBlock::NoMailAccount => (l10n::writing_style_learn_no_account(), None),
            WritingStyleLearnBlock::CannotSend { failure, remedy } => {
                (words::failure(failure), remedy)
            }
        };
        let list = boxed();
        let row = plain_text_row();
        row.set_title(said);
        if let Some(remedy) = remedy {
            row.add_suffix(&remedy_button(remedy, &actions.remedy));
        }
        list.append(&row);
        area.append(&list);
    }
    let learn = Rc::clone(&actions.learn);
    let button = controls::button(l10n::writing_style_learn(), page.learn, false, move || {
        learn();
    });
    button.add_css_class("suggested-action");
    button.add_css_class("pill");
    button.set_halign(gtk::Align::Start);
    area.append(&button);
    area
}

/// A run that failed after its host was closed, with its remedy: Learn reopens the flow on it.
fn failed_row(
    failure: AiFailure,
    remedy: Option<AiRemedyControl>,
    actions: &Actions,
) -> gtk::ListBox {
    let list = boxed();
    let row = plain_text_row();
    row.set_title(l10n::writing_style_run_failed());
    row.set_subtitle(words::failure(failure));
    let mark = gtk::Image::from_icon_name(icons::WARNING);
    mark.add_css_class("warning");
    row.add_prefix(&mark);
    if let Some(remedy) = remedy {
        row.add_suffix(&remedy_button(remedy, &actions.run_remedy));
    }
    list.append(&row);
    list
}

/// A failure's remedy, drawn from its control: it opens where it leads and never runs here.
fn remedy_button(remedy: AiRemedyControl, pressed: &Rc<dyn Fn(AiRemedy)>) -> gtk::Button {
    let open = Rc::clone(pressed);
    controls::button(
        words::remedy(remedy.remedy),
        remedy.control,
        false,
        move || open(remedy.remedy),
    )
}

fn boxed() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// One account's picker, named by its address: **None** first, then the library.
fn slot_row(
    slot: &WritingStyleSlot,
    names: &[&str],
    ids: &Rc<[String]>,
    actions: &Actions,
) -> adw::ComboRow {
    let selected = slot
        .selected
        .as_deref()
        .and_then(|id| ids.iter().position(|candidate| candidate == id))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(0);
    let (assign, ids, account) = (
        Rc::clone(&actions.assign),
        Rc::clone(ids),
        slot.account_id.clone(),
    );
    controls::combo_row(&slot.email, names, selected, slot.picker, move |index| {
        let style = usize::try_from(index)
            .ok()
            .filter(|index| *index > 0)
            .and_then(|index| ids.get(index - 1))
            .cloned();
        assign(account.clone(), style);
    })
}
