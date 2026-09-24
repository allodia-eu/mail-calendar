//! Printing the open message (`../../../docs/reading-actions.md`, "Printing a message"). The page
//! is built in shared Rust; what is native is the web view it is laid out in, which is the reading
//! view's own `SecureWebView`, and the toolkit's print dialog.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{PrintHeaderLine, render_message_print_html};

use super::{AppInput, DocumentKind, ReadingState, SecureWebView};
use crate::l10n;

/// What Print needs of the open message, kept in step with it like the export's file name: the
/// menu is built once and the message changes under it. `None` while there is no body to print.
pub(super) type PrintSource = Rc<RefCell<Option<MessagePrint>>>;

/// One message as it is printed: the header the pane draws, and the body it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MessagePrint {
    subject: String,
    lines: Vec<(&'static str, String)>,
    html: Option<String>,
    plain: Option<String>,
    load_remote_images: bool,
}

impl MessagePrint {
    /// The message `state` holds, printed under `subject` and `date` as the pane draws them, or
    /// `None` while the open is still running or its body could not be fetched.
    pub(super) fn of(state: &ReadingState, subject: &str, date: &str) -> Option<Self> {
        let opened = state.opened.as_ref()?;
        let reading = &state.snapshot;
        if !state.matches_opened() || reading.pending || reading.load_error {
            return None;
        }
        let from = if reading.from.trim().is_empty() {
            &opened.from
        } else {
            &reading.from
        };
        Some(Self {
            subject: subject.to_owned(),
            lines: vec![
                (l10n::compose_from(), from.clone()),
                (l10n::compose_to(), reading.to.clone()),
                (l10n::compose_cc(), reading.cc.clone()),
                (l10n::compose_bcc(), reading.bcc.clone()),
                (l10n::quote_sent(), date.to_owned()),
            ],
            html: reading.html.clone(),
            plain: reading.plain.clone(),
            load_remote_images: state.load_remote_images,
        })
    }

    fn document(&self) -> String {
        render_message_print_html(
            self.subject.clone(),
            self.lines
                .iter()
                .map(|(label, value)| PrintHeaderLine {
                    label: (*label).to_owned(),
                    value: value.clone(),
                })
                .collect(),
            self.html.clone(),
            self.plain.clone(),
            self.load_remote_images,
        )
    }
}

thread_local! {
    /// The web view of the print in flight. The print operation borrows it, so something has to
    /// hold it until the operation says it is done.
    static PRINTING: RefCell<Option<SecureWebView>> = const { RefCell::new(None) };
}

/// The "Print" item: closes the menu, then prints the message [`PrintSource`] holds.
pub(super) fn print_item(
    window: &gtk::Window,
    print_source: &PrintSource,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let item = gtk::Button::with_label(l10n::action_print());
    item.add_css_class("flat");
    item.set_sensitive(false);
    let parent = window.clone();
    let print_source = Rc::clone(print_source);
    let sender = sender.clone();
    item.connect_clicked(move |button| {
        if let Some(popover) = button
            .ancestor(gtk::Popover::static_type())
            .and_downcast::<gtk::Popover>()
        {
            popover.popdown();
        }
        if let Some(message) = print_source.borrow().as_ref() {
            print(&parent, message, &sender);
        }
    });
    item
}

/// Lays the message out in a reading web view nobody sees, then hands it to the print dialog.
fn print(window: &gtk::Window, message: &MessagePrint, sender: &relm4::Sender<AppInput>) {
    let web = SecureWebView::new(DocumentKind::Reading, sender.clone());
    let parent = window.clone();
    let asked = std::cell::Cell::new(false);
    web.connect_finished(move |view| {
        if asked.replace(true) {
            return;
        }
        let view = view.clone();
        let parent = parent.clone();
        // Out of the load signal before the dialog runs its own main loop.
        glib::idle_add_local_once(move || {
            let operation = webkit6::PrintOperation::new(&view);
            operation.connect_finished(|_| release());
            operation.connect_failed(|_, _| release());
            if operation.run_dialog(Some(&parent)) == webkit6::PrintOperationResponse::Cancel {
                release();
            }
        });
    });
    web.load(&message.document(), message.load_remote_images);
    PRINTING.with(|printing| *printing.borrow_mut() = Some(web));
}

fn release() {
    PRINTING.with(|printing| *printing.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::MessagePrint;
    use crate::{
        l10n,
        ui::model::{OpenedMessage, ReadingState, blank_avatar, empty_reading},
    };

    fn opened() -> ReadingState {
        let mut state = ReadingState::new(empty_reading());
        state.open(OpenedMessage {
            account: "account".to_owned(),
            key: "message".to_owned(),
            subject: "Subject".to_owned(),
            from: "Sender".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&blank_avatar()),
        });
        state.snapshot.key = "message".to_owned();
        state.snapshot.from = "Sender <sender@example.test>".to_owned();
        state.snapshot.to = "me@example.test".to_owned();
        state.snapshot.html = Some("<p>Body</p>".to_owned());
        state
    }

    #[test]
    fn the_printed_header_says_what_the_pane_says() {
        let print = MessagePrint::of(&opened(), "Subject", "20 July 2026, 14:05").unwrap();
        assert_eq!(
            print.lines,
            vec![
                (
                    l10n::compose_from(),
                    "Sender <sender@example.test>".to_owned()
                ),
                (l10n::compose_to(), "me@example.test".to_owned()),
                (l10n::compose_cc(), String::new()),
                (l10n::compose_bcc(), String::new()),
                (l10n::quote_sent(), "20 July 2026, 14:05".to_owned()),
            ]
        );
    }

    #[test]
    fn only_a_fetched_body_is_printable() {
        let mut state = opened();
        assert!(MessagePrint::of(&state, "s", "d").is_some());
        state.snapshot.pending = true;
        assert!(MessagePrint::of(&state, "s", "d").is_none());
        state.snapshot.pending = false;
        state.snapshot.load_error = true;
        assert!(MessagePrint::of(&state, "s", "d").is_none());
        // A snapshot still for the message open a moment ago is not this one's body.
        state.snapshot.load_error = false;
        state.snapshot.key = "previous".to_owned();
        assert!(MessagePrint::of(&state, "s", "d").is_none());
    }
}
