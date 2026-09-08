//! The attachment row's two rules (it names the file each control acts on, and neither of its
//! lines is ever parsed as markup) and the page the body area is drawn on.

use adw::prelude::PreferencesRowExt;
use gtk::prelude::{ButtonExt, Cast, WidgetExt};
use mailcal_bindings::{AttachmentRow, CalendarWriteStatus};

use super::{InvitationClock, ReadingPane};
use crate::{
    l10n,
    ui::reading::attachments::{attachment_button, attachment_row},
};

/// The opened-message header renders the instant in the active display zone.
pub(crate) fn the_reading_header_formats_its_timestamp() {
    let window = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<super::super::AppInput>();
    let pane = ReadingPane::new(&window, sender.clone());
    let raw = "2026-07-20T09:05:00Z";
    let mut snapshot = crate::ui::model::empty_reading();
    snapshot.key = "message".to_owned();
    snapshot.plain = Some("Body".to_owned());
    let mut state = crate::ui::model::ReadingState::new(snapshot);
    state.open(crate::ui::model::OpenedMessage {
        account: "fixture".to_owned(),
        key: "message".to_owned(),
        subject: "Subject".to_owned(),
        from: "Sender".to_owned(),
        date: raw.to_owned(),
        avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
    });

    pane.render(
        &state,
        None,
        false,
        InvitationClock {
            zone: "Europe/Amsterdam",
            use_24_hour: true,
            write_status: CalendarWriteStatus::Idle,
            generation: 1,
        },
        &sender,
    );

    assert_eq!(pane.date.text(), "2026-07-20 11:05");
    assert_ne!(pane.date.text(), raw);
}

/// The file name is the sender's text. Built through the property builder, `use-markup` lands
/// after the title, so `Q3 & Q4.pdf` is parsed as markup once before the flag turns it off: the
/// row still reads correctly and only the log record tells them apart.
pub(crate) fn an_attachment_name_is_never_parsed_as_markup() {
    let attachment = AttachmentRow {
        id: 1,
        file_name: "Q3 & Q4.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
        size: 2048,
    };
    let window = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<super::super::AppInput>();
    let (row, records) =
        crate::ui::mailbox::tests::glib_records(|| attachment_row(&attachment, &window, &sender));
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "an attachment row must not parse the file name as markup: {records:?}"
    );
    assert!(!row.uses_markup());
    let rendered = labels(row.upcast_ref::<gtk::Widget>());
    assert!(
        rendered.iter().any(|text| text == "Q3 & Q4.pdf"),
        "the file name must render as itself: {rendered:?}"
    );
}

/// The visible label stays the bare verb; a row of file names down the button column would be
/// unreadable, and the file is already the row's title.
pub(crate) fn an_attachment_button_still_reads_as_its_verb() {
    let button = attachment_button(l10n::action_save(), "Q3 report.pdf");
    assert_eq!(button.label().as_deref(), Some(l10n::action_save()));
}

/// The body area is one page for the whole of an open, so a body arriving changes the text on
/// it and never the page itself.
///
/// This drives the sequence the defect showed on: a message drawn, the next one opened while the
/// core's published snapshot is still the *previous* message's (which is every open, because the
/// core publishes once and only when the body is ready), and then that message's body. Leaving
/// the middle step transparent punched a hole in the page for as long as the open took, so the
/// body area went white, black, white on a dark theme.
///
/// The class is the assertion because
/// [`the_drawn_canvas_paints_the_page_the_core_names`](crate::ui::reading::canvas::tests) has
/// already read the pixels it paints; snapshotting the live pane instead would be asserting on
/// whether a frame had been through, which is not what this is about.
pub(crate) fn the_page_survives_the_gap_before_the_next_body_lands() {
    let window = adw::ApplicationWindow::builder().build();
    let (sender, _receiver) = relm4::channel::<super::super::AppInput>();
    let pane = ReadingPane::new(&window, sender.clone());
    let clock = InvitationClock {
        zone: "Europe/Amsterdam",
        use_24_hour: true,
        write_status: CalendarWriteStatus::Idle,
        generation: 1,
    };
    let drawn = |pane: &ReadingPane| {
        pane.body_stack
            .has_css_class(crate::ui::reading::canvas::CANVAS_CLASS)
    };

    // Nothing open: no message, so no page. The pane is chrome waiting to be given one.
    let mut state = crate::ui::model::ReadingState::new(crate::ui::model::empty_reading());
    pane.render(&state, None, false, clock, &sender);
    assert!(!drawn(&pane));

    // The first message, drawn.
    state.snapshot.key = "first".to_owned();
    state.snapshot.plain = Some("First body".to_owned());
    state.open(opened("first"));
    pane.render(&state, None, false, clock, &sender);
    assert!(drawn(&pane));

    // The second one opened. The core has published nothing for it yet, so the snapshot in hand
    // is still the first message's: the gap the defect lived in.
    state.open(opened("second"));
    assert!(!state.matches_opened());
    pane.render(&state, None, false, clock, &sender);
    assert_eq!(
        pane.body_stack.visible_child_name().as_deref(),
        Some("blank")
    );
    assert!(
        drawn(&pane),
        "the page has to outlast the gap before a body lands, or every open flickers"
    );

    // Its body arrives.
    state.snapshot.key = "second".to_owned();
    state.snapshot.plain = Some("Second body".to_owned());
    pane.render(&state, None, false, clock, &sender);
    assert!(drawn(&pane));

    // A message with no body at all, and a failed open. Both are reached straight from that gap,
    // so a page of their own on either is the same flicker.
    state.snapshot.plain = None;
    pane.render(&state, None, false, clock, &sender);
    assert!(drawn(&pane));
    state.snapshot.load_error = true;
    pane.render(&state, None, false, clock, &sender);
    assert!(drawn(&pane));

    // Closed again: back to chrome, and the idle wording rather than a message's "no content".
    state.close();
    pane.render(&state, None, false, clock, &sender);
    assert!(!drawn(&pane));
    assert_eq!(
        pane.body_stack.visible_child_name().as_deref(),
        Some("idle")
    );
}

fn opened(key: &str) -> crate::ui::model::OpenedMessage {
    crate::ui::model::OpenedMessage {
        account: "fixture".to_owned(),
        key: key.to_owned(),
        subject: "Subject".to_owned(),
        from: "Sender".to_owned(),
        date: "2026-07-20T09:05:00Z".to_owned(),
        avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
    }
}

fn labels(root: &gtk::Widget) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.text().to_string());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(labels(&node));
        child = node.next_sibling();
    }
    found
}
