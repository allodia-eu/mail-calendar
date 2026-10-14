//! The meeting's invitee list inside the event editor: who it invites, and whether each one is
//! required or optional, as the organiser edits it.
//!
//! Offered only where the core says a meeting may be organised (`docs/calendar.md`, "Organising a
//! meeting"): on a create, a calendar whose server sends the invitations; on an edit, a meeting
//! this account organises. Somebody else's meeting shows its list read-only, with the sentence
//! saying so, and a create on a calendar that cannot invite says why nobody can be added.
//!
//! Addresses are typed into the composer's own recipient field, so they complete from the same
//! people index as To does, and each finished address moves into a row of its own. The list is all
//! the section keeps: what a Save sends is worked out from it by the core
//! (`meeting_invitee_patch`).

use std::{cell::RefCell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, glib};
use mailcal_bindings::{
    EventAttendee, InviteeCheck, MailcalApp, MeetingInvitee, MeetingInviteeRole,
    check_meeting_invitee,
};

use super::{
    attendees::{attendee_group, attendee_response, attendee_row},
    editor::EventEditor,
};
use crate::{
    l10n,
    ui::{icons, mailbox, recipients::RecipientField},
};

/// The list a Save reads, shared with the rows that change it.
pub(super) type Invitees = Rc<RefCell<Vec<MeetingInvitee>>>;

/// Takes in an address typed but not yet added, which a Save does first; `false` when it cannot
/// be invited, and the section has said why.
pub(super) type TakeTyped = Rc<dyn Fn() -> bool>;

/// Appends the attendee section to `form`, beneath the calendar picker it depends on.
pub(super) fn append_invitee_section(
    form: &gtk::Box,
    editor: &EventEditor,
    calendar: &gtk::DropDown,
    app: Option<Arc<MailcalApp>>,
) -> (Invitees, TakeTyped) {
    let invitees: Invitees = Rc::new(RefCell::new(editor.initial_invitees()));
    let attendees = editor
        .editing
        .as_ref()
        .map(|detail| detail.attendees.clone())
        .unwrap_or_default();
    if editor.editing.is_some() && !editor.can_invite(calendar.selected()) {
        // Shown so an edit is not made blind to who is coming, and stated to be read-only rather
        // than offered as a field that would quietly drop the change.
        if let Some(group) = attendee_group(&attendees) {
            form.append(&group);
            form.append(&dim_note(l10n::event_attendees_read_only()));
        }
        return (invitees, Rc::new(|| true));
    }

    let group = adw::PreferencesGroup::builder()
        .title(l10n::event_attendees())
        .build();
    // The rows no control here may change: the organiser, and a role the editor cannot state.
    for attendee in attendees
        .iter()
        .filter(|attendee| attendee.is_organizer || attendee.role.is_none())
    {
        group.add(&attendee_row(attendee));
    }
    let field = RecipientField::new(l10n::event_invitee_field(), app);
    field.set_placeholder(l10n::event_invitee_field());
    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    let organiser: Box<dyn Fn() -> String> = {
        let editor = editor.clone();
        let calendar = calendar.clone();
        Box::new(move || editor.organiser(calendar.selected()))
    };
    let section = Rc::new(Section {
        organiser,
        attendees,
        invitees: Rc::clone(&invitees),
        group: group.clone(),
        rows: RefCell::new(Vec::new()),
        field,
        error: error.clone(),
    });
    section.rebuild();
    section.connect();

    let invite = gtk::Button::with_label(l10n::event_invitee_add());
    invite.set_valign(gtk::Align::Start);
    let weak = Rc::downgrade(&section);
    invite.connect_clicked(move |_| {
        if let Some(section) = weak.upgrade() {
            section.take_typed();
        }
    });
    let input = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    section.field.widget().set_hexpand(true);
    input.append(section.field.widget());
    input.append(&invite);

    let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
    controls.append(&group);
    // Above the field rather than under it: a refusal puts the caret back in the field, which
    // scrolls the dialog to it, and a reason underneath can still be cut off by the window's edge.
    controls.append(&error);
    controls.append(&input);
    controls.append(&dim_note(l10n::event_invitees_send_note()));
    form.append(&controls);
    if editor.editing.is_none() {
        // A create can move to a calendar whose server sends nothing, and back.
        let unavailable = dim_note(l10n::event_invitees_unavailable());
        form.append(&unavailable);
        let show = move |editor: &EventEditor, index: u32| {
            let on = editor.can_invite(index);
            controls.set_visible(on);
            unavailable.set_visible(!on);
        };
        show(editor, calendar.selected());
        let editor = editor.clone();
        calendar.connect_selected_notify(move |calendar| show(&editor, calendar.selected()));
    }
    let take: TakeTyped = Rc::new(move || section.take_typed());
    (invitees, take)
}

struct Section {
    organiser: Box<dyn Fn() -> String>,
    /// The meeting as stored: each row's answer, and the rows nobody edits here.
    attendees: Vec<EventAttendee>,
    invitees: Invitees,
    group: adw::PreferencesGroup,
    rows: RefCell<Vec<adw::ActionRow>>,
    field: RecipientField,
    error: gtk::Label,
}

impl Section {
    fn connect(self: &Rc<Self>) {
        // An address finished in the field, by a comma or an accepted suggestion, moves straight
        // into the list. Deferred: this runs inside the field's own change.
        let weak = Rc::downgrade(self);
        self.field.connect_changed(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(section) = weak.upgrade() {
                    for address in section.field.take_finished() {
                        section.add(&address);
                    }
                }
            });
        });
        let weak = Rc::downgrade(self);
        self.field.connect_activate(move || {
            if let Some(section) = weak.upgrade() {
                section.take_typed();
            }
        });
    }

    /// Adds what is still in the field; an empty field is not a refusal.
    fn take_typed(self: &Rc<Self>) -> bool {
        let typed = self.field.typed();
        if typed.is_empty() {
            return true;
        }
        let added = self.add(&typed);
        if added {
            self.field.clear();
        }
        added
    }

    fn add(self: &Rc<Self>, typed: &str) -> bool {
        let invited = self
            .attendees
            .iter()
            .filter(|attendee| !attendee.is_organizer && attendee.role.is_none())
            .map(|attendee| attendee.email.clone())
            .chain(
                self.invitees
                    .borrow()
                    .iter()
                    .map(|invitee| invitee.email.clone()),
            )
            .collect();
        match check_meeting_invitee(typed.to_owned(), invited, (self.organiser)()) {
            InviteeCheck::Valid { address } => {
                self.invitees.borrow_mut().push(MeetingInvitee {
                    name: None,
                    email: address,
                    role: MeetingInviteeRole::Required,
                });
                self.error.set_visible(false);
                self.rebuild();
                true
            }
            refused => {
                self.error.set_text(refusal(&refused).unwrap_or_default());
                self.error.set_visible(true);
                self.field.focus_entry();
                false
            }
        }
    }

    fn rebuild(self: &Rc<Self>) {
        for row in self.rows.take() {
            self.group.remove(&row);
        }
        let last = keeps_its_last_invitee(&self.attendees, self.invitees.borrow().len());
        let rows: Vec<adw::ActionRow> = self
            .invitees
            .borrow()
            .iter()
            .enumerate()
            .map(|(index, invitee)| self.invitee_row(index, invitee, last))
            .collect();
        for row in &rows {
            self.group.add(row);
        }
        self.rows.replace(rows);
    }

    fn invitee_row(
        self: &Rc<Self>,
        index: usize,
        invitee: &MeetingInvitee,
        last: bool,
    ) -> adw::ActionRow {
        let row = mailbox::plain_text_row();
        let name = invitee.name.as_deref().filter(|name| !name.is_empty());
        let shown = name.unwrap_or(&invitee.email);
        row.set_title(shown);
        if name.is_some() {
            row.set_subtitle(&invitee.email);
        }
        if let Some(stored) = self
            .attendees
            .iter()
            .find(|attendee| attendee.email.eq_ignore_ascii_case(&invitee.email))
        {
            let answer = gtk::Label::new(Some(attendee_response(stored.response)));
            answer.add_css_class("dim-label");
            row.add_suffix(&answer);
        }
        let optional = gtk::CheckButton::with_label(l10n::event_attendee_optional());
        optional.set_active(invitee.role == MeetingInviteeRole::Optional);
        optional.set_valign(gtk::Align::Center);
        // Every row's box says "Optional"; the person tells a screen reader which one this is.
        optional.update_property(&[AccessibleProperty::Description(shown)]);
        let weak = Rc::downgrade(self);
        optional.connect_toggled(move |toggle| {
            let Some(section) = weak.upgrade() else {
                return;
            };
            if let Some(invitee) = section.invitees.borrow_mut().get_mut(index) {
                invitee.role = if toggle.is_active() {
                    MeetingInviteeRole::Optional
                } else {
                    MeetingInviteeRole::Required
                };
            }
        });
        row.add_suffix(&optional);
        let remove = gtk::Button::from_icon_name(icons::CLOSE);
        remove.add_css_class("flat");
        remove.set_valign(gtk::Align::Center);
        // Named for the person, so the fourth of four identical buttons is still distinguishable
        // to a screen reader.
        let spoken = l10n::event_invitee_remove(shown);
        remove.set_tooltip_text(Some(&spoken));
        remove.update_property(&[AccessibleProperty::Label(&spoken)]);
        if last {
            // The core refuses a meeting with nobody invited, so the control says so instead.
            remove.set_sensitive(false);
            remove.set_tooltip_text(Some(l10n::event_invitee_last()));
            remove.update_property(&[AccessibleProperty::Description(l10n::event_invitee_last())]);
        }
        let weak = Rc::downgrade(self);
        remove.connect_clicked(move |_| {
            let weak = weak.clone();
            // Deferred: the rebuild takes away the row this button sits in.
            glib::idle_add_local_once(move || {
                let Some(section) = weak.upgrade() else {
                    return;
                };
                {
                    let mut invitees = section.invitees.borrow_mut();
                    if index < invitees.len() {
                        invitees.remove(index);
                    }
                }
                section.rebuild();
            });
        });
        row.add_suffix(&remove);
        row
    }
}

/// Whether the one invitee left on an existing meeting must stay: a meeting is called off by
/// deleting it, and a roster change that leaves nobody invited is refused. `attendees` is the
/// stored meeting, empty on a create; a row nobody edits here, such as a chair, still counts.
pub(super) fn keeps_its_last_invitee(attendees: &[EventAttendee], invitees: usize) -> bool {
    invitees == 1
        && !attendees.is_empty()
        && !attendees
            .iter()
            .any(|attendee| !attendee.is_organizer && attendee.role.is_none())
}

/// What the field says when an address cannot be added; `None` for one that can.
pub(super) fn refusal(check: &InviteeCheck) -> Option<&'static str> {
    match check {
        InviteeCheck::Valid { .. } => None,
        InviteeCheck::NotAnAddress => Some(l10n::event_invitee_not_an_address()),
        InviteeCheck::AlreadyInvited => Some(l10n::event_invitee_already_invited()),
        InviteeCheck::Organiser => Some(l10n::event_invitee_organiser()),
    }
}

fn dim_note(text: &str) -> gtk::Label {
    let note = gtk::Label::new(Some(text));
    note.add_css_class("dim-label");
    note.set_wrap(true);
    note.set_xalign(0.0);
    note
}

#[cfg(test)]
#[path = "invitees_editor_tests.rs"]
pub(crate) mod tests;
