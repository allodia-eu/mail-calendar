//! The windows beside the mailbox: one per message opened in a window of its own, one per draft
//! raised inside one (`docs/reading-window.md`).
//!
//! Each is an `AdwWindow` whose whole content is a view the mailbox window already has, drawing a
//! slot of its own in the one core: the reading window is [`ReadingPane`], the composer window is
//! [`ComposerPane`]. Nothing here holds a second model, and none of these windows is added to the
//! `GtkApplication`, so the app still ends when the mailbox does rather than living on as a
//! scatter of message windows.
//!
//! The model decides which windows exist; this file only brings the screen to that list, the way
//! the message rows are brought to the mailbox snapshot.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use adw::prelude::*;

use super::{
    AppInput, AppModel,
    composer::ComposerPane,
    reader::ReadingSource,
    reading::{InvitationClock, ReadingPane},
};
use crate::l10n;

/// One message in a window of its own.
struct ReadingWindow {
    window: adw::Window,
    view: ReadingPane,
}

/// One draft in a window of its own.
struct ComposerWindow {
    window: adw::Window,
    view: ComposerPane,
}

#[derive(Default)]
struct Windows {
    reading: HashMap<String, ReadingWindow>,
    composer: HashMap<u64, ComposerWindow>,
    /// The bring-forward this host has already answered. A render runs on every model change, and
    /// presenting on each would keep pulling a window in front of whatever the person moved to.
    focused: u64,
    /// Whether the mailbox has gone. A window's own close is reported back through the input
    /// queue, so a render can still run between the sweep and the model hearing about it; without
    /// this it would helpfully build the windows again on the way out.
    swept: bool,
}

/// Every detached window this session has open.
///
/// Shared rather than owned, because the mailbox window's own close handler has to reach the list
/// too: the windows were opened out of the mailbox and go with it.
#[derive(Clone, Default)]
pub(super) struct DetachedWindows {
    windows: Rc<RefCell<Windows>>,
}

impl DetachedWindows {
    pub(super) fn render(
        &self,
        model: &AppModel,
        parent: &adw::ApplicationWindow,
        sender: &relm4::Sender<AppInput>,
    ) {
        let mut windows = self.windows.borrow_mut();
        if windows.swept {
            return;
        }
        windows.reading.retain(|window, open| {
            let live = model.reading_windows.contains_key(window);
            if !live {
                open.view.suspend();
                open.window.close();
            }
            live
        });
        let clock = InvitationClock {
            zone: model.calendar.display_zone(),
            use_24_hour: model.calendar.uses_24_hour(),
            write_status: model.calendar.write_status(),
            generation: model.reading_generation,
        };
        for (window, state) in &model.reading_windows {
            let open = windows
                .reading
                .entry(window.clone())
                .or_insert_with(|| reading_window(window, parent, sender));
            // The subject, so the window list the desktop draws tells two open messages apart.
            // From the header the window was opened on rather than from the body, which is what
            // lets it be named from its first frame, before the fetch has landed.
            open.window.set_title(Some(&window_title(
                state
                    .opened
                    .as_ref()
                    .map(|message| message.subject.as_str()),
                l10n::mail_no_subject(),
            )));
            open.view.render(
                state,
                model.app.as_ref(),
                model.webview_available,
                clock,
                sender,
            );
        }
        if windows.focused != model.reading_window_seq {
            windows.focused = model.reading_window_seq;
            if let Some(open) = model
                .reading_window_focus
                .as_ref()
                .and_then(|window| windows.reading.get(window))
            {
                open.window.present();
            }
        }

        windows.composer.retain(|id, open| {
            let live = model.composer_windows.iter().any(|draft| draft.id == *id);
            if !live {
                open.view.teardown();
                open.window.close();
            }
            live
        });
        if model.composer_windows.is_empty() {
            return;
        }
        let accounts = sender_accounts(model);
        for draft in &model.composer_windows {
            let open = windows
                .composer
                .entry(draft.id)
                .or_insert_with(|| composer_window(draft.id, parent, sender));
            if !open.view.is_active(draft.id) {
                open.window.set_title(Some(&window_title(
                    Some(draft.request.subject.as_str()),
                    l10n::compose_title_new(),
                )));
                open.view.show(
                    draft.id,
                    &draft.request,
                    &accounts,
                    model.app.as_ref(),
                    &open.window,
                    sender.clone(),
                );
                open.window.present();
            }
            if let Some(notice) = draft.error {
                open.view.show_error(notice.text());
            }
        }
    }

    /// The mailbox is closing, so its windows close with it.
    ///
    /// Each window's own close request is what frees the body the core was holding for it, so
    /// closing here is the whole sweep rather than half of one.
    pub(super) fn close_all(&self) {
        let mut windows = self.windows.borrow_mut();
        windows.swept = true;
        for (_, open) in windows.reading.drain() {
            open.view.suspend();
            open.window.close();
        }
        for (_, open) in windows.composer.drain() {
            open.view.teardown();
            open.window.close();
        }
    }
}

/// (id, the label the From picker shows). The label is `Name <address>`, or the address alone when
/// no name is set, composed by the core so the four clients cannot disagree about the empty case
/// (`docs/sending.md`).
pub(super) fn sender_accounts(model: &AppModel) -> Vec<(String, String)> {
    model
        .snapshot
        .accounts
        .iter()
        .map(|account| {
            (
                account.id.clone(),
                mailcal_bindings::sender_label(account.name.clone(), account.email.clone()),
            )
        })
        .collect()
}

fn reading_window(
    window: &str,
    parent: &adw::ApplicationWindow,
    sender: &relm4::Sender<AppInput>,
) -> ReadingWindow {
    let source = ReadingSource::Window(window.to_owned());
    let detached = new_window(parent, 720, 640);
    let view = ReadingPane::new(&detached, sender.clone(), source);
    detached.set_content(Some(view.widget()));
    let input = sender.clone();
    let window = window.to_owned();
    detached.connect_close_request(move |_| {
        input.emit(AppInput::CloseReadingWindow(window.clone()));
        gtk::glib::Propagation::Proceed
    });
    ReadingWindow {
        window: detached,
        view,
    }
}

fn composer_window(
    id: u64,
    parent: &adw::ApplicationWindow,
    sender: &relm4::Sender<AppInput>,
) -> ComposerWindow {
    let detached = new_window(parent, 760, 700);
    let view = ComposerPane::new();
    detached.set_content(Some(view.widget()));
    // Closing discards the draft, exactly as Cancel does, and asks no more than Cancel does: the
    // two are the same act, and a client that questioned one but not the other would be teaching
    // two rules for one thing (`docs/reading-window.md`).
    let input = sender.clone();
    detached.connect_close_request(move |_| {
        input.emit(AppInput::CloseComposerWindow(id));
        gtk::glib::Propagation::Proceed
    });
    ComposerWindow {
        window: detached,
        view,
    }
}

/// A window beside the mailbox: transient for it, and deliberately **not** an application window.
///
/// `AdwWindow` rather than `GtkWindow` because the view inside it carries its own `AdwHeaderBar`,
/// which is then the window's titlebar and draws the desktop's own controls.
fn new_window(parent: &adw::ApplicationWindow, width: i32, height: i32) -> adw::Window {
    adw::Window::builder()
        .transient_for(parent)
        .default_width(width)
        .default_height(height)
        .build()
}

/// What a window is called: what it is about, or what it is when that is still blank.
fn window_title(subject: Option<&str>, untitled: &str) -> String {
    match subject.map(str::trim) {
        Some(subject) if !subject.is_empty() => subject.to_owned(),
        _ => untitled.to_owned(),
    }
}

#[cfg(test)]
#[path = "detached_tests.rs"]
pub(crate) mod widget_tests;

#[cfg(test)]
mod tests {
    use super::window_title;

    #[test]
    fn a_window_is_named_after_its_message_and_a_blank_subject_says_what_it_is() {
        assert_eq!(
            window_title(Some("Quarterly planning"), "(no subject)"),
            "Quarterly planning"
        );
        assert_eq!(window_title(Some("   "), "(no subject)"), "(no subject)");
        assert_eq!(window_title(None, "(no subject)"), "(no subject)");
    }
}
