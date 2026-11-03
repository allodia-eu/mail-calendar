//! An answer given from an event's detail: Accept and Maybe leave the window open and it follows
//! the store, Decline asks first and closes it (`docs/calendar.md`, "Answering from the calendar").

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    ConferenceDialIn, ConferenceState, EventConference, InvitationResponse, ResponseStatus,
};

use super::{
    super::AppInput,
    answer::{DECLINE, confirm_decline},
    conference::{COPY_LINK, JOIN},
    detail::present_detail,
    editor::EventDetails,
};
use crate::{
    l10n,
    ui::mailbox::{test_tree::every_row_belongs_to_a_list, tests::rendered_labels},
};

fn invited(answer: ResponseStatus) -> EventDetails {
    EventDetails {
        account: "bob@test.local".to_owned(),
        key: "planning".to_owned(),
        calendar: "calendar-b".to_owned(),
        calendar_name: "Work".to_owned(),
        title: "Planning".to_owned(),
        all_day: false,
        timezone: "Europe/Amsterdam".to_owned(),
        start: "2026-10-15T09:00:00".to_owned(),
        end: "2026-10-15T10:00:00".to_owned(),
        location: None,
        notes: None,
        display_location: None,
        display_notes: None,
        conferences: Vec::new(),
        reminder_minutes: None,
        recurrence: None,
        repeat_summary: None,
        repeat_draft: None,
        is_recurring: false,
        can_write: false,
        can_edit_invitees: false,
        my_answer: Some(answer),
        occurrence: String::new(),
        attendees: Vec::new(),
    }
}

fn buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    if let Some(button) = root.downcast_ref::<gtk::Button>() {
        found.push(button.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.extend(buttons(&widget));
        child = widget.next_sibling();
    }
    found
}

fn button(root: &gtk::Widget, label: &str) -> gtk::Button {
    buttons(root)
        .into_iter()
        .find(|button| button.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no {label} button"))
}

pub(crate) fn maybe_leaves_the_detail_open_and_it_follows_the_store() {
    let parent = adw::ApplicationWindow::builder().build();
    let (sender, receiver) = relm4::channel::<AppInput>();
    let view = present_detail(&parent, &invited(ResponseStatus::Accepted), &sender);
    let body = view.window().child().expect("the detail has a body");

    button(&body, l10n::invitation_tentative()).emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::RespondToEvent(
            _,
            InvitationResponse::Tentative,
            _
        ))
    ));
    assert!(view.window().is_visible(), "a maybe closed the detail");

    // While the answer is on its way, a second cannot race it.
    view.show(&invited(ResponseStatus::Accepted), true);
    let body = view.window().child().expect("the detail has a body");
    for answer in [
        l10n::invitation_accept(),
        l10n::invitation_tentative(),
        l10n::invitation_decline(),
    ] {
        assert!(
            !button(&body, answer).is_sensitive(),
            "{answer} while saving"
        );
    }

    // The store holds the new answer: the same window says so, and Accept is offered again.
    view.show(&invited(ResponseStatus::Tentative), false);
    let body = view.window().child().expect("the detail has a body");
    assert!(
        rendered_labels(&body)
            .iter()
            .any(|label| label == l10n::invitation_response_tentative())
    );
    assert!(button(&body, l10n::invitation_accept()).is_sensitive());
    assert!(!button(&body, l10n::invitation_tentative()).is_sensitive());
    view.window().close();
}

pub(crate) fn decline_asks_before_it_answers_and_then_closes_the_detail() {
    let parent = adw::ApplicationWindow::builder().build();
    let (sender, receiver) = relm4::channel::<AppInput>();
    let view = present_detail(&parent, &invited(ResponseStatus::Accepted), &sender);
    let body = view.window().child().expect("the detail has a body");

    button(&body, l10n::invitation_decline()).emit_clicked();
    sender.emit(AppInput::BeginNewEvent);
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::BeginNewEvent)),
        "Decline answered without asking"
    );

    let confirmed = Rc::new(Cell::new(0));
    let count = Rc::clone(&confirmed);
    let question = confirm_decline(&body, move || count.set(count.get() + 1));
    question.close();
    assert_eq!(confirmed.get(), 0, "dismissing the question declined");
    let count = Rc::clone(&confirmed);
    let question = confirm_decline(&body, move || count.set(count.get() + 1));
    question.emit_by_name::<()>("response", &[&DECLINE]);
    assert_eq!(confirmed.get(), 1);

    (view.on_answer(&invited(ResponseStatus::Accepted)))(
        InvitationResponse::Decline,
        "Declined: Planning".to_owned(),
    );
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::RespondToEvent(_, InvitationResponse::Decline, _))
    ));
    assert!(
        !view.window().is_visible(),
        "a decline left the detail open"
    );
}

fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn conference(service: Option<&str>, state: ConferenceState) -> EventConference {
    EventConference {
        service: service.map(str::to_owned),
        join_url: matches!(state, ConferenceState::Ready)
            .then(|| "https://teams.microsoft.com/meet/123?p=x".to_owned()),
        meeting_id: Some("123 456 789".to_owned()),
        passcode: Some(String::new()),
        dial_in: vec![ConferenceDialIn {
            number: "+31 20 555 0100".to_owned(),
            uri: Some("tel:+31205550100".to_owned()),
            pin: Some("990#".to_owned()),
        }],
        notes: None,
        state,
    }
}

/// The detail offers each meeting the core sent, Join first, and shows the notes and the place
/// without what repeats it (`docs/conferencing.md`, rules 2 to 6).
pub(crate) fn the_detail_offers_each_meeting_and_shows_the_notes_without_its_block() {
    let parent = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut detail = invited(ResponseStatus::Accepted);
    detail.location = Some("https://teams.microsoft.com/meet/123?p=x".to_owned());
    detail.notes = Some("Agenda\n\nMicrosoft Teams meeting\nJoin: https://…".to_owned());
    detail.display_location = None;
    detail.display_notes = Some("Agenda".to_owned());
    detail.conferences = vec![
        conference(Some("Microsoft Teams"), ConferenceState::Ready),
        conference(None, ConferenceState::Preparing),
    ];
    let view = present_detail(&parent, &detail, &sender);
    let body = view.window().child().expect("the detail has a body");
    let labels = rendered_labels(&body);

    assert!(named(&body, JOIN).is_some(), "a ready meeting offers Join");
    assert!(named(&body, COPY_LINK).is_some());
    assert!(
        named(&body, &format!("{JOIN}-1")).is_none(),
        "a meeting still being made offers nothing to press"
    );
    for expected in [
        "Microsoft Teams",
        l10n::event_video_meeting(),
        l10n::event_meeting_preparing(),
        "123 456 789",
        "Agenda",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "no {expected}"
        );
    }
    assert!(
        labels
            .iter()
            .any(|label| label.contains(l10n::event_meeting_pin("990#").as_str())),
        "the dial-in row carries its PIN"
    );
    assert!(
        !labels.iter().any(|label| label.contains("Join: https")),
        "the invitation's meeting block repeated the meeting"
    );
    assert!(
        !labels
            .iter()
            .any(|label| label == l10n::event_meeting_passcode()),
        "an empty passcode was drawn as a caption over nothing"
    );
    assert!(
        !labels.iter().any(|label| label.starts_with("https://")),
        "a location that is only the link was shown as a place"
    );
    every_row_belongs_to_a_list(&body);
    view.window().close();
}
