//! Widget-level regressions for conversation rows and the unread weight.
//!
//! These are functions rather than `#[test]`s, and [`super::tests`] calls them: GTK initializes
//! **once, on one thread**, and libtest gives every `#[test]` a thread of its own: two of them
//! racing into `gtk::init` abort the process in the IM module ("Two different plugins tried to
//! register 'IBusIMContext'"). So the crate keeps exactly one GTK test, and what it covers lives
//! in files beside it.

use adw::prelude::*;
use gtk::pango::Weight;
use mailcal_bindings::{
    AccountRow, FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage, ThreadRow, ViewMode,
};

use super::{
    ThreadKey, render_messages,
    tests::{glib_records, labels},
};
use crate::ui::AppInput;

fn message(key: &str, from: &str, preview: &str, unread: bool, outgoing: bool) -> ThreadMessage {
    ThreadMessage {
        avatar: crate::ui::model::blank_avatar(),
        account: "fixture".to_owned(),
        key: key.to_owned(),
        from: from.to_owned(),
        date: "2026-07-20".to_owned(),
        preview: preview.to_owned(),
        unread,
        outgoing,
        has_attachment: false,
    }
}

fn thread(subject: &str, messages: Vec<ThreadMessage>) -> ThreadRow {
    let unread_count = u32::try_from(messages.iter().filter(|message| message.unread).count())
        .expect("a fixture thread is small");
    ThreadRow {
        avatar: crate::ui::model::blank_avatar(),
        account: "fixture".to_owned(),
        thread_id: "thread-1".to_owned(),
        latest_key: messages.first().map(|m| m.key.clone()).unwrap_or_default(),
        subject: subject.to_owned(),
        latest_from: messages.first().map(|m| m.from.clone()).unwrap_or_default(),
        latest_date: "2026-07-20".to_owned(),
        message_count: u32::try_from(messages.len()).expect("a fixture thread is small"),
        unread_count,
        has_attachment: false,
        preview: String::new(),
        messages,
    }
}

fn snapshot(rows: Vec<SnapshotRow>) -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![AccountRow {
            id: "fixture".to_owned(),
            email: "person@example.test".to_owned(),
            expanded: true,
        }],
        selected_account: None,
        folders: Vec::new(),
        account_folders: Vec::new(),
        unified_unread: 0,
        selected: None,
        mode: ViewMode::Threaded,
        total: u64::try_from(rows.len()).expect("a fixture list is small"),
        rows,
        search_horizon: None,
    }
}

fn flat(subject: &str, from: &str, unread: bool) -> SnapshotRow {
    SnapshotRow::Flat {
        row: FlatRow {
            avatar: crate::ui::model::blank_avatar(),
            account: "fixture".to_owned(),
            key: subject.to_owned(),
            subject: subject.to_owned(),
            from: from.to_owned(),
            date: "2026-07-19".to_owned(),
            unread,
            flagged: false,
            has_attachment: false,
            preview: String::new(),
        },
    }
}

/// The weight the given text is drawn at, as the CSS resolved it; never the property we set,
/// which says nothing about what reached the screen.
fn weight_of(root: &gtk::Widget, text: &str) -> Weight {
    labels(root)
        .into_iter()
        .find(|label| label.text() == text)
        .unwrap_or_else(|| panic!("no label renders {text:?}"))
        .pango_context()
        .font_description()
        .expect("a rendered label resolves a font")
        .weight()
}

/// A list in a presented window, so GTK resolves each row's style against the real display.
fn rendered(
    snapshot: &MailboxListSnapshot,
    expanded: &[ThreadKey],
) -> (
    gtk::ListBox,
    relm4::Sender<AppInput>,
    relm4::Receiver<AppInput>,
) {
    let list = gtk::ListBox::new();
    let (sender, receiver) = relm4::channel::<AppInput>();
    render_messages(
        &list,
        &[],
        snapshot,
        &expanded.iter().cloned().collect(),
        false,
        "UTC",
        &sender,
    );
    let window = gtk::Window::new();
    window.set_child(Some(&list));
    window.present();
    while gtk::glib::MainContext::default().iteration(false) {}
    (list, sender, receiver)
}

/// Everything the rows have reported, read behind a sentinel so a missing report fails the test
/// rather than blocking it.
fn reported(sender: &relm4::Sender<AppInput>, receiver: &relm4::Receiver<AppInput>) -> Vec<String> {
    sender.emit(AppInput::CancelComposer);
    let mut reports = Vec::new();
    while let Some(input) = receiver.recv_sync() {
        match input {
            AppInput::CancelComposer => break,
            AppInput::OpenThreadMessage(message) => reports.push(format!("open {}", message.key)),
            AppInput::SetThreadExpanded { expanded, .. } => {
                reports.push(format!("expanded {expanded}"));
            }
            other => reports.push(format!("{other:?}")),
        }
    }
    reports
}

fn expander_at(list: &gtk::ListBox, index: i32) -> adw::ExpanderRow {
    list.row_at_index(index)
        .expect("the list has a row there")
        .downcast::<adw::ExpanderRow>()
        .expect("a conversation renders as an expander")
}

/// The glyphs the app draws are the ones we ship, and they resolve.
///
/// `Image::from_icon_name` is happy with a name nothing provides: it renders the
/// missing-image placeholder and says nothing; so asking the row for its icon name proves
/// only that we asked. The theme has to be asked whether it can find them, which is what
/// fails if the GResource stops being compiled in or the resource path drifts.
pub(super) fn the_apps_glyphs_are_bundled_with_the_app() {
    let display = gtk::gdk::Display::default().expect("a display");
    let theme = gtk::IconTheme::for_display(&display);
    for icon in ["mailcal-archive-symbolic", "mailcal-inbox-symbolic"] {
        assert!(theme.has_icon(icon), "the app must carry {icon}");
    }
    assert!(
        !theme.has_icon("mailcal-not-an-icon-symbolic"),
        "a theme that answers yes to everything would make the check above meaningless"
    );
    assert!(
        theme
            .resource_path()
            .iter()
            .any(|path| path == super::ICON_RESOURCE_PATH),
        "the bundle must be on the icon theme's resource path"
    );
}

/// A conversation draws its summary and, opened, every message on it; and unread mail is bold
/// wherever it sits.
pub(super) fn conversation_rows_expand_and_unread_mail_is_bold() {
    let list = snapshot(vec![
        SnapshotRow::Thread {
            row: thread(
                "Research & Development",
                vec![
                    message(
                        "newest",
                        "Allodia Mail & Calendar",
                        "Latest reply",
                        true,
                        false,
                    ),
                    message("mine", "<b>Wire transfer</b>", "My answer", false, true),
                    // A real preview carries the body's own newline.
                    message(
                        "oldest",
                        "Anna Bakker",
                        "Where it started\nand where it went",
                        false,
                        false,
                    ),
                ],
            ),
        },
        flat("Quarterly planning", "Bram de Vries", true),
        flat("Receipt", "shop@example.test", false),
    ]);

    let ((widgets, _sender, _receiver), records) = glib_records(|| rendered(&list, &[]));
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "a conversation must not parse the server's text as markup: {records:?}"
    );
    let expander = expander_at(&widgets, 0);
    let shown = expander.upcast_ref::<gtk::Widget>();

    // The summary, then the whole conversation; one sub-row per message, the owner's own reply
    // included, each showing who sent it and what it says.
    assert_eq!(weight_of(shown, "Research & Development"), Weight::Bold);
    assert_eq!(
        weight_of(shown, "Allodia Mail & Calendar"),
        Weight::Bold,
        "an unread conversation carries its weight on the subject and the sender"
    );
    assert_eq!(weight_of(shown, "3"), Weight::Normal);
    for (sender, preview) in [
        ("Allodia Mail & Calendar", "Latest reply"),
        ("<b>Wire transfer</b>", "My answer"),
        // One line, its newline collapsed; a row may not stand a line taller than its neighbours.
        ("Anna Bakker", "Where it started and where it went"),
    ] {
        assert_eq!(
            weight_of(shown, preview),
            Weight::Normal,
            "the preview line stays regular so an unread mailbox is not one solid block"
        );
        // A markup-shaped sender is shown, never applied; an ampersand is not an entity.
        assert!(
            labels(shown).iter().any(|label| label.text() == sender),
            "the conversation must render {sender:?} verbatim"
        );
    }
    // The trap the nesting creates: a read reply sits *inside* the row carrying the unread
    // conversation's weight, and would inherit it.
    assert_eq!(weight_of(shown, "Anna Bakker"), Weight::Normal);
    assert_eq!(weight_of(shown, crate::l10n::thread_sent()), Weight::Normal);

    let flat_unread = widgets.row_at_index(1).expect("the flat row").upcast();
    assert_eq!(weight_of(&flat_unread, "Quarterly planning"), Weight::Bold);
    assert_eq!(weight_of(&flat_unread, "Bram de Vries"), Weight::Bold);
    assert_eq!(
        weight_of(&flat_unread, "2026-07-19"),
        Weight::Normal,
        "the date is chrome, not the message"
    );
    let flat_read = widgets.row_at_index(2).expect("the read row").upcast();
    assert_eq!(weight_of(&flat_read, "Receipt"), Weight::Normal);
    assert_eq!(weight_of(&flat_read, "shop@example.test"), Weight::Normal);
}

/// Flat rows, conversation summaries and conversation messages all use the relative formatter.
pub(super) fn every_mail_row_formats_its_timestamp() {
    let flat_raw = "2000-01-02T09:05:00Z";
    let thread_raw = "2000-02-03T09:05:00Z";
    let message_raw = "2000-03-04T09:05:00Z";
    let single = SnapshotRow::Flat {
        row: FlatRow {
            avatar: crate::ui::model::blank_avatar(),
            account: "fixture".to_owned(),
            key: "single".to_owned(),
            subject: "Single".to_owned(),
            from: "Sender".to_owned(),
            date: flat_raw.to_owned(),
            unread: false,
            flagged: false,
            has_attachment: false,
            preview: String::new(),
        },
    };
    let mut conversation = thread(
        "Conversation",
        vec![message("only", "Sender", "Preview", false, false)],
    );
    conversation.latest_date = thread_raw.to_owned();
    conversation.messages[0].date = message_raw.to_owned();
    let snapshot = snapshot(vec![single, SnapshotRow::Thread { row: conversation }]);
    let (widgets, _sender, _receiver) = rendered(&snapshot, &[]);
    let shown = labels(widgets.upcast_ref::<gtk::Widget>());

    for raw in [flat_raw, thread_raw, message_raw] {
        let expected = crate::ui::timestamps::relative_date(raw, "UTC");
        assert!(
            shown.iter().any(|label| label.text() == expected),
            "the formatted label {expected:?} must be visible: {shown:?}"
        );
        assert!(
            shown.iter().all(|label| label.text() != raw),
            "the engine timestamp must not reach the row unchanged: {shown:?}"
        );
    }
}

/// Opening, closing and reading one message of a conversation each reach the host as themselves.
pub(super) fn a_conversation_reports_what_the_reader_asked_for() {
    let conversation = thread(
        "Quarterly planning",
        vec![
            message("newest", "Anna Bakker", "Latest reply", false, false),
            message("oldest", "Bram de Vries", "Where it started", false, false),
        ],
    );
    let key = ThreadKey::of(&conversation);
    let rows = snapshot(vec![SnapshotRow::Thread { row: conversation }]);

    // Rebuilt with the conversation recorded open, it comes back open; and reports nothing, so a
    // sync cannot re-open its representative over whichever sub-row the reader is on.
    let (widgets, sender, receiver) = rendered(&rows, std::slice::from_ref(&key));
    let expander = expander_at(&widgets, 0);
    assert!(expander.is_expanded());
    assert_eq!(reported(&sender, &receiver), Vec::<String>::new());
    expander.set_expanded(false);
    assert_eq!(reported(&sender, &receiver), vec!["expanded false"]);

    // A sub-row opens the message it stands for, not the conversation's representative.
    let (widgets, sender, receiver) = rendered(&rows, &[]);
    let expander = expander_at(&widgets, 0);
    assert!(!expander.is_expanded());
    expander.emit_activate();
    let opened = labels(expander.upcast_ref::<gtk::Widget>())
        .into_iter()
        .find(|label| label.text() == "Where it started")
        .expect("the oldest message is on screen")
        .ancestor(adw::ActionRow::static_type())
        .expect("its sub-row")
        .downcast::<adw::ActionRow>()
        .expect("a message renders as a row");
    opened
        .activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("a message row has a native primary action")
        .emit_clicked();
    assert_eq!(
        reported(&sender, &receiver),
        vec!["expanded true", "open oldest"]
    );
}

/// Every `ListBoxRow` in `list`, in order.
fn children(list: &gtk::ListBox) -> Vec<gtk::ListBoxRow> {
    let mut rows = Vec::new();
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        rows.push(row);
        index += 1;
    }
    rows
}

/// Re-renders `list` from `before` to `after`, as a sync commit does.
fn rerender(
    list: &gtk::ListBox,
    before: &MailboxListSnapshot,
    after: &MailboxListSnapshot,
    sender: &relm4::Sender<AppInput>,
) -> Vec<gtk::ListBoxRow> {
    render_messages(
        list,
        &before
            .rows
            .iter()
            .map(|row| super::display_row(row, "UTC"))
            .collect::<Vec<_>>(),
        after,
        &std::collections::HashSet::new(),
        false,
        "UTC",
        sender,
    );
    children(list)
}

/// A re-render rebuilds the row whose rendering changed and **nothing else**.
///
/// Nothing else in this suite can see it: every assertion about a rendered row passes just as
/// well over a list that was torn down and rebuilt, which is what a sync commit used to do to it
/// several times a second. So this asserts on widget *identity*; the same GObject, not an equal
/// one.
pub(super) fn a_rerender_rebuilds_only_the_row_that_changed() {
    let before = snapshot(vec![
        flat("One", "a@example.test", true),
        flat("Two", "b@example.test", true),
        flat("Three", "c@example.test", true),
    ]);
    let (list, sender, _receiver) = rendered(&before, &[]);
    let widgets = children(&list);
    assert_eq!(widgets.len(), 3, "the fixture renders three rows");
    list.select_row(Some(&widgets[1]));

    // The middle message has been read; its neighbours are untouched.
    let after = snapshot(vec![
        flat("One", "a@example.test", true),
        flat("Two", "b@example.test", false),
        flat("Three", "c@example.test", true),
    ]);
    let rebuilt = rerender(&list, &before, &after, &sender);

    assert_eq!(rebuilt.len(), 3, "the list still holds three rows");
    assert_eq!(widgets[0], rebuilt[0], "an untouched row keeps its widget");
    assert_ne!(
        widgets[1], rebuilt[1],
        "the row that was read is rebuilt, or it keeps its unread weight"
    );
    assert_eq!(widgets[2], rebuilt[2], "and its neighbour is left alone");
    assert_eq!(
        list.selected_row(),
        Some(rebuilt[1].clone()),
        "the message remains selected when its read state rebuilds its row"
    );
}

/// A row that leaves takes its widget with it, and the survivors keep theirs; the archive case.
pub(super) fn a_removed_row_leaves_its_neighbours_widgets_alone() {
    let before = snapshot(vec![
        flat("One", "a@example.test", true),
        flat("Two", "b@example.test", true),
        flat("Three", "c@example.test", true),
    ]);
    let (list, sender, _receiver) = rendered(&before, &[]);
    let widgets = children(&list);

    let after = snapshot(vec![
        flat("One", "a@example.test", true),
        flat("Three", "c@example.test", true),
    ]);
    let rebuilt = rerender(&list, &before, &after, &sender);

    assert_eq!(rebuilt.len(), 2, "the archived row left the list");
    assert_eq!(widgets[0], rebuilt[0], "the row above it is untouched");
    assert_eq!(
        widgets[2], rebuilt[1],
        "the row below it moved up without being rebuilt"
    );
}

/// New mail arrives at the top, and the rows it pushes down keep their widgets; the case a
/// sync produces most, and the one a positional rebuild gets wrong for every row at once.
pub(super) fn mail_arriving_at_the_top_does_not_rebuild_the_list_below_it() {
    let before = snapshot(vec![
        flat("One", "a@example.test", true),
        flat("Two", "b@example.test", true),
    ]);
    let (list, sender, _receiver) = rendered(&before, &[]);
    let widgets = children(&list);

    let after = snapshot(vec![
        flat("Newest", "d@example.test", true),
        flat("One", "a@example.test", true),
        flat("Two", "b@example.test", true),
    ]);
    let rebuilt = rerender(&list, &before, &after, &sender);

    assert_eq!(rebuilt.len(), 3, "the new message joined the list");
    assert_eq!(
        widgets[0], rebuilt[1],
        "the rows below it shifted, not rebuilt"
    );
    assert_eq!(widgets[1], rebuilt[2], "every one of them");
}
