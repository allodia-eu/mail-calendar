//! The two questions a repeating event puts to the user, and the sentence one of them carries.
//!
//! Both are windows rather than inline notices on purpose: each is asked at the moment of a write
//! and has to be answerable, so a notice the user can scroll past would not do. They live apart
//! from `dialogs.rs` to keep that file inside the 500-line cap.

use std::{cell::Cell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{Intent, MailcalApp, ProposedEdit, SeriesEditWarning};

use super::{
    super::AppInput,
    dialogs::{connect_dismiss, modal},
    editor::{EventEditor, EventForm},
    model::DeleteRequest,
};
use crate::l10n;

/// "This event, or all of them?": asked before a delete on a **repeating** event removes
/// anything.
///
/// The core deliberately has no default here, and neither does this: cancelling one Tuesday
/// standup and cancelling the standup are different requests, and only the user knows which they
/// meant. Closing the window writes nothing, so the safe answer is always available without
/// choosing one of the two.
///
/// This replaces the ordinary delete confirmation rather than following it; the question already
/// carries a way out, and one delete should raise one window.
pub(super) fn present_delete_scope(
    parent: &adw::ApplicationWindow,
    request: &DeleteRequest,
    sender: relm4::Sender<AppInput>,
) {
    // The title is the question, and the header bar already asks it; every other client puts it
    // once too. The body carries the answers.
    let (window, _) = modal(parent, l10n::event_series_scope_delete_title(), 420, 160);
    let replacing = Rc::new(Cell::new(false));
    connect_dismiss(&window, &replacing, sender.clone());
    let shell = body_box();

    let actions = action_row();

    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.update_property(&[AccessibleProperty::Label(l10n::action_cancel())]);
    let closing = Rc::clone(&replacing);
    let dialog = window.clone();
    let input = sender.clone();
    cancel.connect_clicked(move |_| {
        closing.set(true);
        input.emit(AppInput::DismissCalendarDialog);
        dialog.close();
    });
    actions.append(&cancel);

    // *This event* first: it is the narrower of the two, and the one a mis-click leaves something
    // to recover from; the rest of the series is still there.
    let this_event = destructive_button(l10n::event_series_scope_this());
    let closing = Rc::clone(&replacing);
    let dialog = window.clone();
    let input = sender.clone();
    let one = request.identity.clone();
    this_event.connect_clicked(move |_| {
        closing.set(true);
        input.emit(AppInput::DeleteCalendarEvent(one.clone()));
        dialog.close();
    });
    actions.append(&this_event);

    let all_events = destructive_button(l10n::event_series_scope_all());
    let closing = Rc::clone(&replacing);
    let dialog = window.clone();
    // An empty token is what the dispatch reads as "the whole series", so answering *All events*
    // is the same identity with the occurrence cleared.
    let mut series = request.identity.clone();
    series.occurrence = String::new();
    all_events.connect_clicked(move |_| {
        closing.set(true);
        sender.emit(AppInput::DeleteCalendarEvent(series.clone()));
        dialog.close();
    });
    actions.append(&all_events);

    shell.append(&actions);
    window.set_child(Some(&shell));
    window.present();
}

/// "This event, or all of them?": asked between the editor's Save and the write, when the
/// editor was opened on one occurrence of a series.
///
/// Asked **before** the warning, because the answer decides whether a warning is owed at all:
/// *This event* writes an override of its own and costs no other occurrence anything.
///
/// Deliberately **not** wired to `DismissCalendarDialog`, for the same reason
/// [`present_series_warning`] is not: this window sits over the still-open editor, and clearing
/// the model's dialog state would leave that editor on screen with nothing behind it to submit.
/// Closing here writes nothing and returns to the form with what the user typed still in it.
pub(super) fn present_edit_scope(
    parent: &adw::ApplicationWindow,
    on_this_event: impl Fn() + 'static,
    on_all_events: impl Fn() + 'static,
) {
    // The header bar asks the question; the body carries the answers. Neither is destructive,
    // both are saves; so neither gets the destructive style the delete question's answers wear.
    let (window, _) = modal(parent, l10n::event_series_scope_title(), 420, 160);
    let shell = body_box();
    let actions = action_row();

    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.update_property(&[AccessibleProperty::Label(l10n::action_cancel())]);
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    actions.append(&cancel);

    // *This event* first: it is the narrower of the two, and a mis-click leaves the rest of the
    // series as it was.
    let this_event = gtk::Button::with_label(l10n::event_series_scope_this());
    this_event.update_property(&[AccessibleProperty::Label(l10n::event_series_scope_this())]);
    let dialog = window.clone();
    this_event.connect_clicked(move |_| {
        on_this_event();
        dialog.close();
    });
    actions.append(&this_event);

    let all_events = gtk::Button::with_label(l10n::event_series_scope_all());
    all_events.update_property(&[AccessibleProperty::Label(l10n::event_series_scope_all())]);
    let dialog = window.clone();
    all_events.connect_clicked(move |_| {
        on_all_events();
        dialog.close();
    });
    actions.append(&all_events);

    shell.append(&actions);
    window.set_child(Some(&shell));
    window.present();
}

/// What saving a series-level edit costs the occurrences the user changed on their own, asked
/// between the editor's Save and the write.
///
/// Deliberately **not** wired to `DismissCalendarDialog`: this window sits over the still-open
/// editor, and clearing the model's dialog state would leave that editor on screen with nothing
/// behind it to submit. Cancelling or closing here writes nothing and returns to the form with
/// what the user typed still in it.
pub(super) fn present_series_warning(
    parent: &adw::ApplicationWindow,
    warning: &SeriesEditWarning,
    on_confirm: impl Fn() + 'static,
) {
    // The header bar asks the question; the body says what answering it costs.
    let (window, _) = modal(parent, l10n::event_series_warning_title(), 460, 200);
    let shell = body_box();

    let detail = gtk::Label::new(Some(&series_warning_text(warning)));
    detail.set_wrap(true);
    detail.set_xalign(0.0);
    shell.append(&detail);

    let actions = action_row();

    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.update_property(&[AccessibleProperty::Label(l10n::action_cancel())]);
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    actions.append(&cancel);

    let save = gtk::Button::with_label(l10n::action_save());
    save.add_css_class("suggested-action");
    save.update_property(&[AccessibleProperty::Label(l10n::action_save())]);
    let dialog = window.clone();
    save.connect_clicked(move |_| {
        on_confirm();
        dialog.close();
    });
    actions.append(&save);

    shell.append(&actions);
    window.set_child(Some(&shell));
    window.present();
}

/// What a **whole-series** save of `form` would cost the occurrences the user changed on their
/// own, or `None` when there is nothing to say.
///
/// Read off the intent the save is about to dispatch rather than off the form, so the question and
/// the write cannot describe different edits. The core compares it against what is stored, so a
/// field the user typed and typed back is not a change.
pub(super) fn series_warning_for(
    app: Option<&Arc<MailcalApp>>,
    editor: &EventEditor,
    form: &EventForm,
) -> Option<SeriesEditWarning> {
    let app = app?;
    let Ok(Intent::UpdateEvent {
        account,
        key,
        title,
        start,
        end,
        notes,
        location,
        recurrence,
        ..
    }) = editor.intent(form, false)
    else {
        return None;
    };
    app.series_edit_warning(
        account,
        key,
        ProposedEdit {
            title,
            start,
            end,
            notes,
            location,
            // The real one: a rule change is the edit two of the four providers answer by
            // discarding every override, so the warning has to be asked knowing about it.
            recurrence,
        },
    )
}

/// The sentence a verdict gets.
///
/// Wording only: the core decided *whether* there is anything to say and *which* of the three
/// things it is, so this is a `match` over a closed set and a catalog lookup. No provider is ever
/// named; what the user needs is what is about to happen to their own calendar, and the transport
/// it happens on is not their concern.
pub(super) fn series_warning_text(warning: &SeriesEditWarning) -> String {
    match warning {
        SeriesEditWarning::OccurrencesReset => l10n::event_series_warning_reset().to_owned(),
        SeriesEditWarning::RenamesSpread => l10n::event_series_warning_renames().to_owned(),
        SeriesEditWarning::OccurrencesResetAndRenamesSpread => {
            l10n::event_series_warning_reset_and_renames().to_owned()
        }
    }
}

fn body_box() -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 18);
    shell.set_margin_start(24);
    shell.set_margin_end(24);
    shell.set_margin_top(24);
    shell.set_margin_bottom(24);
    shell
}

fn action_row() -> gtk::Box {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    actions
}

fn destructive_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("destructive-action");
    button.update_property(&[AccessibleProperty::Label(label)]);
    button
}

#[cfg(test)]
#[path = "dialogs_series_tests.rs"]
mod tests;
