//! An event's detail window, and what keeps it current while it is open.
//!
//! Answering a meeting leaves the window open, so the body is rebuilt in place from each fresh
//! read rather than the window being presented again: the answer, and the invitee's own row in
//! the attendee list, then say what the server holds. A decline closes it, because a declined
//! meeting is no longer in the calendar (`docs/calendar.md`, "Answering from the calendar").

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::InvitationResponse;

use super::{
    super::AppInput,
    answer,
    attendees::attendee_group,
    dialogs::{connect_dismiss, detail_row, detail_time, modal, reminder, title},
    editor::EventDetails,
    model::EventIdentity,
    repeat,
};
use crate::l10n;

/// An open detail window, which [`DetailView::show`] brings up to date without replacing it.
pub(super) struct DetailView {
    window: gtk::Window,
    body: gtk::Box,
    replacing: Rc<Cell<bool>>,
    sender: relm4::Sender<AppInput>,
    shown: RefCell<Option<(EventDetails, bool)>>,
}

pub(super) fn present_detail(
    parent: &adw::ApplicationWindow,
    detail: &EventDetails,
    sender: &relm4::Sender<AppInput>,
) -> DetailView {
    let (window, header) = modal(parent, &title(&detail.title), 520, 560);
    let replacing = Rc::new(Cell::new(false));
    connect_dismiss(&window, &replacing, sender.clone());
    if detail.can_write {
        let edit = gtk::Button::with_label(l10n::action_edit());
        edit.update_property(&[AccessibleProperty::Label(l10n::action_edit())]);
        let input = sender.clone();
        let closing = Rc::clone(&replacing);
        let dialog = window.clone();
        edit.connect_clicked(move |_| {
            closing.set(true);
            input.emit(AppInput::BeginEditEvent);
            dialog.close();
        });
        header.pack_start(&edit);
        let delete = gtk::Button::with_label(l10n::action_delete());
        delete.add_css_class("destructive-action");
        delete.update_property(&[AccessibleProperty::Label(l10n::action_delete())]);
        let input = sender.clone();
        let closing = Rc::clone(&replacing);
        let dialog = window.clone();
        delete.connect_clicked(move |_| {
            closing.set(true);
            input.emit(AppInput::RequestDeleteCurrentEvent);
            dialog.close();
        });
        header.pack_end(&delete);
    }
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&body));
    scroll.set_vexpand(true);
    window.set_child(Some(&scroll));
    let view = DetailView {
        window,
        body,
        replacing,
        sender: sender.clone(),
        shown: RefCell::new(None),
    };
    view.show(detail, false);
    view.window.present();
    view
}

impl DetailView {
    /// Shows `detail`, with the answer held back while `saving`; nothing is rebuilt when that is
    /// what the window already shows.
    pub(super) fn show(&self, detail: &EventDetails, saving: bool) {
        let mut shown = self.shown.borrow_mut();
        if shown
            .as_ref()
            .is_some_and(|(open, held)| open == detail && *held == saving)
        {
            return;
        }
        *shown = Some((detail.clone(), saving));
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.body.append(&facts(detail));
        if let Some(current) = detail.my_answer {
            let answer = answer::answer_group(
                current,
                &title(&detail.title),
                saving,
                &self.on_answer(detail),
            );
            answer.set_margin_start(18);
            answer.set_margin_end(18);
            self.body.append(&answer);
        }
        // No group at all for an appointment nobody was invited to: an empty "Attendees" heading
        // would read as "we looked and found none", a different statement from "this is not a
        // meeting".
        if let Some(attendees) = attendee_group(&detail.attendees) {
            attendees.set_margin_start(18);
            attendees.set_margin_end(18);
            self.body.append(&attendees);
        }
    }

    pub(super) fn on_answer(&self, detail: &EventDetails) -> answer::OnAnswer {
        let (input, closing, dialog) = (
            self.sender.clone(),
            Rc::clone(&self.replacing),
            self.window.clone(),
        );
        let event = EventIdentity {
            account: detail.account.clone(),
            key: detail.key.clone(),
            occurrence: String::new(),
        };
        Rc::new(move |response, subject| {
            let declined = matches!(response, InvitationResponse::Decline);
            if declined {
                closing.set(true);
            }
            input.emit(AppInput::RespondToEvent(event.clone(), response, subject));
            if declined {
                dialog.close();
            }
        })
    }

    #[cfg(test)]
    pub(super) fn window(&self) -> &gtk::Window {
        &self.window
    }
}

fn facts(detail: &EventDetails) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_margin_start(18);
    group.set_margin_end(18);
    group.set_margin_top(18);
    group.add(&detail_row(l10n::event_start(), &detail_time(detail, true)));
    group.add(&detail_row(l10n::event_end(), &detail_time(detail, false)));
    group.add(&detail_row(l10n::event_calendar(), &detail.calendar_name));
    if let Some(location) = detail.location.as_deref().filter(|value| !value.is_empty()) {
        group.add(&detail_row(l10n::event_location(), location));
    }
    if let Some(notes) = detail.notes.as_deref().filter(|value| !value.is_empty()) {
        group.add(&crate::ui::linked_text::row(l10n::event_notes(), notes));
    }
    group.add(&detail_row(
        l10n::event_reminder(),
        &reminder(detail.reminder_minutes),
    ));
    group.add(&detail_row(
        l10n::event_repeat(),
        &repeat::sentence(detail.repeat_summary.as_ref(), detail.is_recurring),
    ));
    if detail.is_recurring {
        let note = gtk::Label::new(Some(l10n::event_series_note()));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.set_margin_top(12);
        group.add(&note);
    }
    group
}
