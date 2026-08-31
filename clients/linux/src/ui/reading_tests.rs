//! The attachment row's two rules: it names the file each control acts on, and neither of its
//! lines is ever parsed as markup.

use adw::prelude::PreferencesRowExt;
use gtk::prelude::{ButtonExt, Cast, WidgetExt};
use mailcal_bindings::{AttachmentRow, CalendarWriteStatus};

use super::{InvitationClock, ReadingPane, attachment_button};
use crate::l10n;

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
    let (row, records) = crate::ui::mailbox::tests::glib_records(|| {
        super::attachment_row(&attachment, &window, &sender)
    });
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
