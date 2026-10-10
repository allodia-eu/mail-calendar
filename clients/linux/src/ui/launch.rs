//! The window while the core opens: the launch view of `docs/boot-sequence.md`.
//!
//! `MailcalApp::new_accounts` opens the engine store, migrations included, so it runs on a worker
//! thread and the window comes up without it. Until its result arrives as [`AppInput::Booted`]
//! there is no [`AppModel`] at all: every other input is held, and handed over in the order it
//! arrived once the model exists.

use std::time::Duration;

use adw::prelude::*;

use super::{AppInput, AppModel, shell::AppWidgets};
use crate::{boot, l10n, logger, observer::SurfaceObserver};

/// The component behind the main window.
pub(crate) enum AppWindow {
    /// The core is opening on its worker thread.
    Opening(Opening),
    /// The core has opened, or failed to, and the model owns the window.
    Open(Box<AppModel>),
}

/// What the window holds while the core opens.
#[derive(Debug, Default)]
pub(crate) struct Opening {
    status_due: bool,
    /// The core's own signals raised while it was being built, and a mail link or share the
    /// launch was handed. The model takes them, in arrival order, the moment it exists.
    held: Vec<AppInput>,
}

impl Opening {
    pub(super) const fn view(&self) -> LaunchView {
        if self.status_due {
            LaunchView::Status
        } else {
            LaunchView::Blank
        }
    }

    pub(super) fn status_due(&mut self) {
        self.status_due = true;
    }

    pub(super) fn hold(&mut self, input: AppInput) {
        self.held.push(input);
    }

    pub(super) fn take_held(&mut self) -> Vec<AppInput> {
        std::mem::take(&mut self.held)
    }
}

/// What the launch page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaunchView {
    /// Inside the delay: a normal open ends here, and a status raised and removed within it
    /// reads as flicker.
    Blank,
    /// The open has outlasted the delay.
    Status,
}

/// How long the launch page stays blank, as the core exports it.
pub(super) fn status_delay() -> Duration {
    Duration::from_millis(mailcal_bindings::launch_status_after_ms().into())
}

/// Opens the core on a worker thread, and arms the timer that lets the launch page speak.
///
/// The credential store goes into the constructor on the worker, never afterwards
/// (`docs/boot-sequence.md`, invariant 4); [`boot::app`] builds it there.
pub(super) fn open_core(input: &relm4::Sender<AppInput>) {
    let observer = SurfaceObserver::new(input.clone());
    let worker = input.clone();
    std::thread::spawn(move || {
        // A panic here ends the process, as it did when the core opened on the UI thread: a
        // launch page left waiting for a worker that is gone would spin for ever.
        let booted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            boot::app(Box::new(observer))
        }))
        .unwrap_or_else(|_| std::process::abort());
        worker.emit(AppInput::Booted(booted));
    });
    let due = input.clone();
    gtk::glib::timeout_add_local_once(status_delay(), move || {
        due.emit(AppInput::LaunchStatusDue);
    });
}

/// The window's widgets: the launch page until the model exists, then the shell in its place.
pub(crate) struct WindowWidgets {
    root: adw::ApplicationWindow,
    input: relm4::Sender<AppInput>,
    launch: LaunchPage,
    shell: Option<AppWidgets>,
}

impl WindowWidgets {
    pub(super) fn new(root: adw::ApplicationWindow, input: relm4::Sender<AppInput>) -> Self {
        let launch = LaunchPage::new();
        root.set_content(Some(launch.widget()));
        Self {
            root,
            input,
            launch,
            shell: None,
        }
    }

    pub(super) fn render(&mut self, window: &AppWindow) {
        match window {
            AppWindow::Opening(opening) => self.launch.render(opening.view()),
            AppWindow::Open(model) => {
                let shell = self.shell.get_or_insert_with(|| {
                    // Takes the window's content, so the launch page goes with it.
                    let shell = AppWidgets::new(self.root.clone(), self.input.clone());
                    announce_on_screen(&self.root);
                    shell
                });
                shell.render(model);
            }
        }
    }
}

/// Writes [`logger::WINDOW_ON_SCREEN`] once the mailbox is on screen.
///
/// Not at the launch page: the core installs the log sink as it opens, so a line written before
/// then lands nowhere, and a caller waiting to drive the app needs the shell, not the page in
/// front of it. `map` fires again on a remap, so the line is written once.
fn announce_on_screen(root: &adw::ApplicationWindow) {
    if root.is_mapped() {
        log::info!("{}", logger::WINDOW_ON_SCREEN);
        return;
    }
    let announced = std::cell::Cell::new(false);
    root.connect_map(move |_| {
        if !announced.replace(true) {
            log::info!("{}", logger::WINDOW_ON_SCREEN);
        }
    });
}

/// The launch page: blank, then a centred spinner over [`l10n::status_opening_mailbox`]. A header
/// bar with no title keeps the window movable and closable while it waits.
struct LaunchPage {
    page: adw::ToolbarView,
    status: gtk::Box,
}

impl LaunchPage {
    fn new() -> Self {
        let spinner = gtk::Spinner::new();
        spinner.set_spinning(true);
        spinner.set_size_request(48, 48);
        let status = gtk::Box::new(gtk::Orientation::Vertical, 12);
        status.set_halign(gtk::Align::Center);
        status.set_valign(gtk::Align::Center);
        status.append(&spinner);
        status.append(&gtk::Label::new(Some(l10n::status_opening_mailbox())));
        status.set_visible(false);
        let header = adw::HeaderBar::new();
        header.set_show_title(false);
        let page = adw::ToolbarView::new();
        page.add_top_bar(&header);
        page.set_content(Some(&status));
        Self { page, status }
    }

    const fn widget(&self) -> &adw::ToolbarView {
        &self.page
    }

    fn render(&self, view: LaunchView) {
        self.status.set_visible(view == LaunchView::Status);
    }
}

#[cfg(test)]
mod tests {
    use super::{AppInput, LaunchView, Opening};

    #[test]
    fn the_page_is_blank_until_the_delay_has_passed() {
        let mut opening = Opening::default();
        assert_eq!(opening.view(), LaunchView::Blank);
        opening.status_due();
        assert_eq!(opening.view(), LaunchView::Status);
    }

    /// A mail link handed to the launch, or a signal the core raised while it was being built,
    /// reaches the model as it would have had the core opened on the UI thread: all of them, in
    /// the order they came.
    #[test]
    fn what_arrives_while_opening_reaches_the_model_in_order() {
        let mut opening = Opening::default();
        opening.hold(AppInput::ShowCalendar);
        opening.hold(AppInput::RefreshRequested);
        opening.hold(AppInput::ShowMail);
        assert_eq!(
            format!("{:?}", opening.take_held()),
            "[ShowCalendar, RefreshRequested, ShowMail]"
        );
        assert!(
            opening.take_held().is_empty(),
            "nothing is handed over twice"
        );
    }

    #[test]
    fn the_delay_is_the_cores() {
        assert_eq!(
            super::status_delay().as_millis(),
            u128::from(mailcal_bindings::launch_status_after_ms())
        );
    }
}
