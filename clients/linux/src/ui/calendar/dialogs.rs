//! Calendar detail, editor, and delete-confirmation windows.

use std::{cell::Cell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::MailcalApp;

use super::{
    super::AppInput,
    attendees::attendee_group,
    date::date_from_wall,
    dialogs_series,
    editor::{EventDetails, EventEditor, EventForm},
    model::{CalendarDialog, DeleteRequest},
    repeat, repeat_editor,
};
use crate::l10n;

pub(super) fn present(
    parent: &adw::ApplicationWindow,
    dialog: &CalendarDialog,
    sender: relm4::Sender<AppInput>,
    app: Option<&Arc<MailcalApp>>,
) {
    match dialog {
        CalendarDialog::Detail(detail) => present_detail(parent, detail, &sender),
        CalendarDialog::Editor(editor) => present_editor(parent, editor, sender, app),
        CalendarDialog::ConfirmDelete(request) => {
            present_delete(parent, request, sender);
        }
    }
}

fn present_detail(
    parent: &adw::ApplicationWindow,
    detail: &EventDetails,
    sender: &relm4::Sender<AppInput>,
) {
    let title = title(&detail.title);
    let (window, header) = modal(parent, &title, 520, 560);
    let replacing = Rc::new(Cell::new(false));
    connect_dismiss(&window, &replacing, sender.clone());
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
        group.add(&detail_row(l10n::event_notes(), notes));
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
    // No group at all for an appointment nobody was invited to: an empty "Attendees" heading
    // would read as "we looked and found none", a different statement from "this is not a meeting".
    let attendees = attendee_group(&detail.attendees);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&group);
    if let Some(attendees) = attendees {
        attendees.set_margin_start(18);
        attendees.set_margin_end(18);
        body.append(&attendees);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&body));
    scroll.set_vexpand(true);
    shell.append(&scroll);
    window.set_child(Some(&shell));
    window.present();
}

fn present_editor(
    parent: &adw::ApplicationWindow,
    editor: &EventEditor,
    sender: relm4::Sender<AppInput>,
    app: Option<&Arc<MailcalApp>>,
) {
    let app = app.cloned();
    let window_title = if editor.editing.is_some() {
        l10n::event_edit_title()
    } else {
        l10n::event_new_title()
    };
    let (window, header) = modal(parent, window_title, 560, 680);
    let replacing = Rc::new(Cell::new(false));
    connect_dismiss(&window, &replacing, sender.clone());
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
    header.pack_start(&cancel);
    let save = gtk::Button::with_label(l10n::action_save());
    save.add_css_class("suggested-action");
    save.update_property(&[AccessibleProperty::Label(l10n::action_save())]);
    header.pack_end(&save);

    let form = gtk::Box::new(gtk::Orientation::Vertical, 12);
    form.set_margin_start(24);
    form.set_margin_end(24);
    form.set_margin_top(18);
    form.set_margin_bottom(24);
    let title = entry(l10n::event_title_label(), &editor.title);
    form.append(&title);
    // The caret opens where the work starts; the empty title on a new event, the same rule the
    // composer's To follows (docs/calendar.md, docs/contacts.md §4). Not on edit: the event already
    // has a title, and the dates are usually what the user came to change.
    //
    // Deferred to the entry's `map`: the window is still being assembled, and `grab_focus` on a
    // widget with no window to take focus in answers `false` and does nothing: silently.
    if editor.editing.is_none() {
        title.connect_map(|entry| {
            entry.grab_focus();
        });
    }
    let all_day = gtk::CheckButton::with_label(l10n::calendar_all_day());
    all_day.set_active(editor.all_day);
    all_day.set_sensitive(editor.can_edit_form());
    form.append(&all_day);
    let start = entry(l10n::event_start(), &editor.start);
    let end = entry(l10n::event_end(), &editor.end);
    let toggle_start = start.clone();
    let toggle_end = end.clone();
    all_day.connect_toggled(move |toggle| {
        let (start, end) = EventEditor::values_for_mode(
            toggle_start.text().as_str(),
            toggle_end.text().as_str(),
            toggle.is_active(),
        );
        toggle_start.set_text(&start);
        toggle_end.set_text(&end);
    });
    form.append(&start);
    form.append(&end);
    let hint = gtk::Label::new(Some(l10n::event_editor_format_hint()));
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    form.append(&hint);
    let labels = editor
        .choices
        .iter()
        .map(|choice| choice.label.as_str())
        .collect::<Vec<_>>();
    let calendar = gtk::DropDown::from_strings(&labels);
    calendar.set_selected(editor.selected);
    calendar.set_sensitive(editor.can_edit_form());
    calendar.update_property(&[AccessibleProperty::Label(l10n::event_pick_calendar())]);
    form.append(&field_label(l10n::event_calendar()));
    form.append(&calendar);
    let location = entry(l10n::event_location(), &editor.location);
    form.append(&location);
    let notes_label = field_label(l10n::event_notes());
    form.append(&notes_label);
    let notes = gtk::TextView::new();
    notes.set_wrap_mode(gtk::WrapMode::WordChar);
    notes.set_top_margin(8);
    notes.set_bottom_margin(8);
    notes.set_left_margin(8);
    notes.set_right_margin(8);
    notes.buffer().set_text(&editor.notes);
    notes.update_property(&[AccessibleProperty::Label(l10n::event_notes())]);
    let notes_frame = gtk::Frame::new(None);
    notes_frame.set_child(Some(&notes));
    form.append(&notes_frame);
    let repeat_draft = repeat_editor::append_repeat_section(&form, editor);
    // Only when the answer is settled. An editor opened on one occurrence asks at Save which
    // occurrences were meant, so stating the answer up here would tell the user something the
    // next dialog contradicts.
    if editor
        .editing
        .as_ref()
        .is_some_and(|detail| detail.is_recurring)
        && !editor.asks_about_the_series()
    {
        let note = gtk::Label::new(Some(l10n::event_series_note()));
        note.set_wrap(true);
        note.set_xalign(0.0);
        form.append(&note);
    }
    // Attendees: shown so an edit is not made blind to who is coming, and stated to be read-only
    // rather than offered as a field that would quietly drop the change; editing them means
    // sending iTIP updates, which is its own feature.
    if let Some(attendees) = editor
        .editing
        .as_ref()
        .and_then(|detail| attendee_group(&detail.attendees))
    {
        form.append(&attendees);
        let note = gtk::Label::new(Some(l10n::event_attendees_read_only()));
        note.add_css_class("dim-label");
        note.set_wrap(true);
        note.set_xalign(0.0);
        form.append(&note);
    }
    let error = gtk::Label::new(Some(l10n::event_editor_invalid()));
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_visible(false);
    form.append(&error);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&form));
    scroll.set_vexpand(true);
    shell.append(&scroll);
    window.set_child(Some(&shell));

    let editor = editor.clone();
    let closing = Rc::clone(&replacing);
    let dialog = window.clone();
    let host = parent.clone();
    save.connect_clicked(move |_| {
        let buffer = notes.buffer();
        let form = EventForm {
            title: title.text().to_string(),
            start: start.text().to_string(),
            end: end.text().to_string(),
            all_day: all_day.is_active(),
            location: location.text().to_string(),
            notes: buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string(),
            calendar_index: calendar.selected(),
            repeat: repeat_draft.borrow().clone(),
        };
        if editor.intent(&form, false).is_err() {
            error.set_visible(true);
            return;
        }
        let submit: Rc<dyn Fn(EventForm, bool)> = {
            let closing = Rc::clone(&closing);
            let dialog = dialog.clone();
            let sender = sender.clone();
            Rc::new(move |form: EventForm, this_occurrence_only: bool| {
                closing.set(true);
                sender.emit(AppInput::SubmitEventForm(
                    Box::new(form),
                    this_occurrence_only,
                ));
                dialog.close();
            })
        };
        // A whole-series save: what it costs the occurrences the user changed on their own goes
        // first when there is anything to say. Straight through when there is not; a server that
        // keeps them, a series holding none, or an edit that touches nothing they would lose.
        // Cancelling the question leaves this form exactly as it is.
        let commit_series: Rc<dyn Fn(EventForm)> = {
            let submit = Rc::clone(&submit);
            let host = host.clone();
            let app = app.clone();
            let editor = editor.clone();
            Rc::new(move |form: EventForm| {
                match dialogs_series::series_warning_for(app.as_ref(), &editor, &form) {
                    Some(warning) => {
                        let submit = Rc::clone(&submit);
                        dialogs_series::present_series_warning(&host, &warning, move || {
                            submit(form.clone(), false);
                        });
                    }
                    None => submit(form, false),
                }
            })
        };
        // Which occurrences the save meant. Only an editor opened on one occurrence has a
        // question to ask; anything else can only mean the series, and so does a changed repeat,
        // because a rule belongs to the series rather than to one instance of it.
        if editor.save_asks_about_the_series(&form) {
            let this_event = {
                let submit = Rc::clone(&submit);
                let form = form.clone();
                move || submit(form.clone(), true)
            };
            let all_events = {
                let commit = Rc::clone(&commit_series);
                let form = form.clone();
                move || commit(form.clone())
            };
            dialogs_series::present_edit_scope(&host, this_event, all_events);
            return;
        }
        commit_series(form);
    });
    window.present();
}

fn present_delete(
    parent: &adw::ApplicationWindow,
    request: &DeleteRequest,
    sender: relm4::Sender<AppInput>,
) {
    // Opened from one occurrence of a series: which occurrences the delete meant is the question
    // to put, and it replaces this confirmation rather than following it.
    if request.identity.asks_about_the_series() {
        dialogs_series::present_delete_scope(parent, request, sender);
        return;
    }
    let height = if request.is_recurring { 250 } else { 190 };
    let (window, _) = modal(parent, l10n::action_delete_event(), 420, height);
    let replacing = Rc::new(Cell::new(false));
    connect_dismiss(&window, &replacing, sender.clone());
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 18);
    shell.set_margin_start(24);
    shell.set_margin_end(24);
    shell.set_margin_top(24);
    shell.set_margin_bottom(24);
    let question = gtk::Label::new(Some(l10n::event_delete_confirm()));
    question.add_css_class("title-2");
    question.set_wrap(true);
    shell.append(&question);
    if request.is_recurring {
        let note = gtk::Label::new(Some(l10n::event_series_note()));
        note.set_wrap(true);
        note.set_xalign(0.0);
        shell.append(&note);
    }
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
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
    let delete = gtk::Button::with_label(l10n::action_delete());
    delete.add_css_class("destructive-action");
    delete.update_property(&[AccessibleProperty::Label(l10n::action_delete())]);
    let closing = Rc::clone(&replacing);
    let dialog = window.clone();
    let identity = request.identity.clone();
    delete.connect_clicked(move |_| {
        closing.set(true);
        sender.emit(AppInput::DeleteCalendarEvent(identity.clone()));
        dialog.close();
    });
    actions.append(&delete);
    shell.append(&actions);
    window.set_child(Some(&shell));
    window.present();
}

pub(super) fn modal(
    parent: &adw::ApplicationWindow,
    title: &str,
    width: i32,
    height: i32,
) -> (gtk::Window, adw::HeaderBar) {
    crate::ui::modal::new(parent, title, width, Some(height))
}

pub(super) fn connect_dismiss(
    window: &gtk::Window,
    replacing: &Rc<Cell<bool>>,
    sender: relm4::Sender<AppInput>,
) {
    let replacing = Rc::clone(replacing);
    window.connect_close_request(move |_| {
        if !replacing.get() {
            sender.emit(AppInput::DismissCalendarDialog);
        }
        gtk::glib::Propagation::Proceed
    });
}

fn entry(label: &str, value: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_placeholder_text(Some(label));
    entry.update_property(&[AccessibleProperty::Label(label)]);
    entry
}

fn field_label(label: &str) -> gtk::Label {
    let value = gtk::Label::new(Some(label));
    value.set_xalign(0.0);
    value.add_css_class("heading");
    value
}

fn detail_row(label: &str, value: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(label)
        .subtitle(value)
        .use_markup(false)
        .build()
}

fn title(value: &str) -> String {
    if value.trim().is_empty() {
        l10n::event_no_title().to_owned()
    } else {
        value.to_owned()
    }
}

fn detail_time(detail: &EventDetails, start: bool) -> String {
    if start {
        return detail.start.clone();
    }
    if detail.all_day {
        return date_from_wall(&detail.end)
            .and_then(time::Date::previous_day)
            .map_or_else(|| detail.end.clone(), |date| date.to_string());
    }
    detail.end.clone()
}

fn reminder(minutes: Option<i32>) -> String {
    match minutes {
        None => l10n::event_reminder_none().to_owned(),
        Some(value) if value <= 0 => l10n::event_reminder_at_start().to_owned(),
        Some(value) if value % 1440 == 0 => l10n::event_reminder_days(i64::from(value / 1440)),
        Some(value) if value % 60 == 0 => l10n::event_reminder_hours(i64::from(value / 60)),
        Some(value) => l10n::event_reminder_minutes(i64::from(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::reminder;

    #[test]
    fn reminder_uses_the_coarsest_exact_unit() {
        assert_eq!(reminder(Some(90)), "90 minutes before");
        assert_eq!(reminder(Some(120)), "2 hours before");
        assert_eq!(reminder(Some(2880)), "2 days before");
    }
}
