//! The editor's Video meeting row (`docs/conferencing.md`, rules 9 to 14): which meetings it
//! offers, what it opens showing, and what a save sends.
//!
//! The core decides what a calendar offers (`CalendarRow::meeting_choices`) and which meeting an
//! editor opens on (`EventDetail::editor_meeting`); this only holds the person's pick and compares
//! it with what the editor opened with. Free of GTK, so the unit tests drive it directly.

use mailcal_bindings::{MeetingChange, MeetingOption, MeetingWords};

use super::editor::{EventEditor, EventForm};
use crate::l10n;

/// What the Video meeting row holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum MeetingSelection {
    /// No meeting.
    #[default]
    None,
    /// One of the calendar's `meeting_choices`, by its id.
    Choice(String),
    /// The meeting the event already has that no choice makes, kept as it is.
    Current,
}

/// One entry the row offers. `name` is the service's brand name, with the account that makes it
/// for a choice made through an account, or `None` for the screen's own words: "None" for
/// [`MeetingSelection::None`], "Video meeting" for a meeting nothing names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MeetingRowOption {
    pub(super) selection: MeetingSelection,
    pub(super) name: Option<String>,
}

impl MeetingRowOption {
    /// What the row draws for this entry.
    pub(super) fn label(&self) -> String {
        match (&self.selection, &self.name) {
            (MeetingSelection::None, _) => l10n::event_meeting_none().to_owned(),
            (_, Some(name)) => name.clone(),
            (_, None) => l10n::event_video_meeting().to_owned(),
        }
    }
}

/// A choice as the row names it: the service, and the account that makes it for one made through
/// an account, so two work accounts are two entries a person can tell apart.
pub(super) fn choice_label(choice: &MeetingOption) -> String {
    choice.through.as_ref().map_or_else(
        || choice.name.clone(),
        |account| l10n::event_meeting_through(&choice.name, account),
    )
}

impl EventEditor {
    /// What this editor opened with.
    pub(super) fn opened_meeting(&self) -> MeetingSelection {
        match self
            .editing
            .as_ref()
            .and_then(|detail| detail.editor_meeting.as_ref())
        {
            None => MeetingSelection::None,
            Some(meeting) => meeting
                .choice
                .clone()
                .map_or(MeetingSelection::Current, MeetingSelection::Choice),
        }
    }

    /// The meetings the calendar at `index` offers. An edit cannot move the event, so it reads
    /// the event's own calendar whatever the picker holds.
    fn meeting_choices(&self, index: u32) -> &[MeetingOption] {
        let calendar = match &self.editing {
            Some(detail) => self
                .choices
                .iter()
                .find(|choice| choice.account == detail.account && choice.id == detail.calendar),
            None => usize::try_from(index)
                .ok()
                .and_then(|index| self.choices.get(index)),
        };
        calendar.map_or(&[], |calendar| calendar.meeting_choices.as_slice())
    }

    /// Whether the row is drawn: the calendar offers a meeting, or the event has one to take off.
    pub(super) fn shows_meeting_row(&self, index: u32) -> bool {
        !self.meeting_choices(index).is_empty()
            || self
                .editing
                .as_ref()
                .is_some_and(|detail| detail.editor_meeting.is_some())
    }

    /// The row's entries: None first, the event's own meeting where the calendar does not offer
    /// it (one no choice makes, or a Jitsi meeting after the server was cleared), then the
    /// calendar's choices by the core's names, one made through an account with its address.
    pub(super) fn meeting_options(&self, index: u32) -> Vec<MeetingRowOption> {
        let choices = self.meeting_choices(index);
        let current = self
            .editing
            .as_ref()
            .and_then(|detail| detail.editor_meeting.as_ref())
            .filter(|meeting| {
                !choices
                    .iter()
                    .any(|choice| Some(&choice.id) == meeting.choice.as_ref())
            })
            .map(|meeting| MeetingRowOption {
                selection: self.opened_meeting(),
                name: meeting.name.clone(),
            });
        std::iter::once(MeetingRowOption {
            selection: MeetingSelection::None,
            name: None,
        })
        .chain(current)
        .chain(choices.iter().map(|choice| MeetingRowOption {
            selection: MeetingSelection::Choice(choice.id.clone()),
            name: Some(choice_label(choice)),
        }))
        .collect()
    }

    /// The pick a save reads: one the calendar at `index` does not offer goes back to None
    /// (rule 9), unless it is what the editor opened with.
    pub(super) fn effective_meeting(
        &self,
        index: u32,
        pick: &MeetingSelection,
    ) -> MeetingSelection {
        let MeetingSelection::Choice(id) = pick else {
            return pick.clone();
        };
        if *pick == self.opened_meeting()
            || self
                .meeting_choices(index)
                .iter()
                .any(|choice| choice.id == *id)
        {
            return pick.clone();
        }
        MeetingSelection::None
    }

    /// Whether the row says the link is added at the save: it holds a choice the event does not
    /// have yet, which is made only when the event is saved (rule 10).
    pub(super) fn announces_meeting(&self, index: u32, pick: &MeetingSelection) -> bool {
        let effective = self.effective_meeting(index, pick);
        matches!(effective, MeetingSelection::Choice(_)) && effective != self.opened_meeting()
    }

    /// What a save of `form` does to the meeting, or `None` to leave it alone: None takes the
    /// event's meeting off, a choice replaces it, and leaving the row as it opened changes nothing.
    pub(super) fn meeting_change(&self, form: &EventForm) -> Option<MeetingChange> {
        let pick = self.effective_meeting(form.calendar_index, &form.meeting);
        if pick == self.opened_meeting() {
            return None;
        }
        match pick {
            MeetingSelection::None => self.editing.is_some().then_some(MeetingChange::Remove),
            MeetingSelection::Current => None,
            MeetingSelection::Choice(choice) => Some(MeetingChange::Set {
                choice,
                words: meeting_words(),
            }),
        }
    }
}

/// The catalog's words for the block a meeting made elsewhere leaves in the notes, in the app's
/// language: the core has no locale.
fn meeting_words() -> MeetingWords {
    MeetingWords {
        meeting: l10n::event_video_meeting().to_owned(),
        join: l10n::event_join().to_owned(),
        meeting_id: l10n::event_meeting_id().to_owned(),
        passcode: l10n::event_meeting_passcode().to_owned(),
        dial_in: l10n::event_meeting_dial_in().to_owned(),
        dial_in_code: l10n::event_meeting_conference_id().to_owned(),
    }
}

#[cfg(test)]
#[path = "editor_meeting_tests.rs"]
mod tests;
