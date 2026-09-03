//! Neither series question repeats its own title inside its body.
//!
//! `modal::new` renders the title in the client-side title bar, once: so a `title-2` label
//! saying the same thing again is the title twice on one small window. Every other client asks
//! the question once, in the dialog's title, and the rule is already pinned for the shared
//! chrome by `modal::tests::a_modal_renders_its_title_in_native_chrome_only`; these two windows
//! are where it can be broken again, because each builds its own body.

use adw::prelude::*;
use mailcal_bindings::SeriesEditWarning;

use super::{
    super::AppInput,
    dialogs::present,
    dialogs_series::{present_delete_scope, present_edit_scope, present_series_warning},
    editor::{EventDetails, EventEditor},
    model::{CalendarDialog, DeleteRequest, EventIdentity},
};
use crate::{l10n, ui::mailbox::tests::rendered_labels};

fn delete_request() -> DeleteRequest {
    DeleteRequest {
        identity: EventIdentity {
            account: "alice@test.local".to_owned(),
            key: "onboarding".to_owned(),
            occurrence: "2026-08-27T13:00:00".to_owned(),
        },
        is_recurring: true,
    }
}

/// The visible window whose title bar carries `title`, and every label in its body.
fn body_labels(title: &str) -> Vec<String> {
    let window = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
        .find(|window| window.title().as_deref() == Some(title))
        .expect("the dialog put a window on screen");
    let body = window.child().expect("the dialog has a body");
    let labels = rendered_labels(&body);
    window.close();
    labels
}

pub(crate) fn neither_series_question_states_its_title_twice() {
    let parent = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<AppInput>();

    present_delete_scope(&parent, &delete_request(), sender.clone());
    let scope = l10n::event_series_scope_delete_title();
    let labels = body_labels(scope);
    assert!(
        !labels.iter().any(|label| label == scope),
        "the scope question states its title in the body as well as the title bar: {labels:?}"
    );
    // The answers are what the body is for, so this is not an empty-window assertion.
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::event_series_scope_this())
    );
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::event_series_scope_all())
    );

    present_series_warning(&parent, &SeriesEditWarning::RenamesSpread, || {});
    let warning = l10n::event_series_warning_title();
    let labels = body_labels(warning);
    assert!(
        !labels.iter().any(|label| label == warning),
        "the series warning states its title in the body as well as the title bar: {labels:?}"
    );
    // What it costs still has to be said, or the question is unanswerable.
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::event_series_warning_renames())
    );

    // The edit's own scope question. It is a third window built by hand, so it can break the
    // same rule independently of the two above; and unlike them it is raised over the still-open
    // editor, which is the shape a stray title-2 label is most obviously wrong in.
    present_edit_scope(&parent, || {}, || {});
    let edit_scope = l10n::event_series_scope_title();
    let labels = body_labels(edit_scope);
    assert!(
        !labels.iter().any(|label| label == edit_scope),
        "the edit scope question states its title in the body as well as the title bar: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::event_series_scope_this())
    );
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::event_series_scope_all())
    );
}

/// A repeating event, as the core resolved it for `occurrence`: empty for the series itself.
fn recurring_detail(occurrence: &str) -> EventDetails {
    EventDetails {
        account: "alice@test.local".to_owned(),
        key: "team-sync".to_owned(),
        calendar: "calendar-a".to_owned(),
        calendar_name: "Work".to_owned(),
        title: "Team sync".to_owned(),
        all_day: false,
        timezone: "Europe/Amsterdam".to_owned(),
        start: "2026-08-26T11:30:00".to_owned(),
        end: "2026-08-26T12:00:00".to_owned(),
        location: None,
        notes: None,
        reminder_minutes: None,
        recurrence: None,
        repeat_summary: None,
        repeat_draft: None,
        is_recurring: true,
        can_write: true,
        occurrence: occurrence.to_owned(),
        attendees: Vec::new(),
    }
}

/// The editor does not answer the question it is about to ask.
///
/// "Changes apply to the whole series." is true of an editor opened on the series, and states
/// something the *This event · All events* dialog then contradicts when it was opened on one
/// occurrence. The note and the question are decided by the same fact, so they can only
/// disagree if one of them forgets to read it; which is what happened here on two clients
/// while the third had the rule written down.
pub(crate) fn the_editor_never_pre_empts_the_scope_question() {
    let parent = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let note = l10n::event_series_note();

    // Opened on the series: nothing will be asked, so the note is the only thing that says how
    // far the save reaches.
    present(
        &parent,
        &CalendarDialog::Editor(EventEditor::edit(recurring_detail(""), Vec::new())),
        sender.clone(),
        None,
    );
    let labels = body_labels(l10n::event_edit_title());
    assert!(
        labels.iter().any(|label| label == note),
        "an editor on the series says how far a save reaches: {labels:?}"
    );

    // Opened on one occurrence: Save asks, so saying it here would be answering for the user.
    present(
        &parent,
        &CalendarDialog::Editor(EventEditor::edit(
            recurring_detail("2026-08-26T11:30:00"),
            Vec::new(),
        )),
        sender,
        None,
    );
    let labels = body_labels(l10n::event_edit_title());
    assert!(
        !labels.iter().any(|label| label == note),
        "the editor answers the scope question it is about to ask: {labels:?}"
    );
}
