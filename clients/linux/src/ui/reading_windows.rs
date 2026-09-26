//! The model's half of the detached windows: which are open, what each is showing, and the core
//! slot behind it (`docs/reading-window.md`).
//!
//! A window is a *view of the running app*: it holds no core of its own and fetches nothing of its
//! own. Opening one is the pane's own open aimed at a different slot, so a window inherits the
//! bounded retry for an account still dialing, the threshold before a loading state is announced
//! and the mark-read, rather than restating any of them. The widgets that draw all this are in
//! [`super::detached`].

use mailcal_bindings::{Intent, MailcalApp};

use super::{
    AppModel,
    composer_model::ComposeContext,
    composer_notice::ComposerNotice,
    model::{self, OpenedMessage, ReadingState},
    reader::{ComposerHost, ReadingSource},
};

/// One draft being written in a window of its own.
///
/// Host state: a message exists to the core only once it is sent, so a draft in a window is
/// nothing the core holds. The error line is the window's own for the same reason the pane's is
/// the pane's; a failed prepare belongs beside the draft it failed for.
pub(crate) struct DetachedDraft {
    pub(crate) id: u64,
    pub(crate) request: ComposeContext,
    pub(crate) error: Option<ComposerNotice>,
}

impl AppModel {
    /// What one reader is drawing, or `None` for a window that has since closed.
    pub(super) fn reader(&self, source: &ReadingSource) -> Option<&ReadingState> {
        match source {
            ReadingSource::Pane => Some(&self.reading),
            ReadingSource::Window(window) => self.reading_windows.get(window),
        }
    }

    pub(super) fn reader_mut(&mut self, source: &ReadingSource) -> Option<&mut ReadingState> {
        match source {
            ReadingSource::Pane => Some(&mut self.reading),
            ReadingSource::Window(window) => self.reading_windows.get_mut(window),
        }
    }

    /// The message one reader has open.
    pub(super) fn reader_message(&self, source: &ReadingSource) -> Option<&OpenedMessage> {
        self.reader(source).and_then(|state| state.opened.as_ref())
    }

    /// Opens `message` in a window of its own, or brings the window it is already in forward.
    ///
    /// The second case is why the id is derived from the message rather than minted: a row
    /// double-clicked twice must reach one window, not two on the same mail.
    ///
    /// It leaves the pane, the selection and any open draft exactly as they were: opening a
    /// second reader must not disturb the first, which is the whole point of the window, and it
    /// means this can never raise the unsent-draft question, because it takes nothing away
    /// (`docs/reading-window.md`).
    pub(super) fn open_reading_window(&mut self, message: OpenedMessage) {
        let window = ReadingSource::window_id(&message.account, &message.key);
        self.reading_window_seq = self.reading_window_seq.wrapping_add(1);
        self.reading_window_focus = Some(window.clone());
        if self.reading_windows.contains_key(&window) {
            return;
        }
        self.dispatch(Intent::OpenMessageInWindow {
            window: window.clone(),
            account: message.account.clone(),
            key: message.key.clone(),
        });
        let mut state = ReadingState::new(model::empty_reading());
        state.open(message);
        self.reading_windows.insert(window, state);
    }

    /// The window has gone: forget its header and tell the core to drop its body.
    ///
    /// Both halves matter. A sanitised body carries every inline image resolved into it, so it is
    /// the largest thing either side holds per window, and a session that opened twenty messages
    /// would otherwise hold twenty of them for as long as it ran.
    pub(super) fn close_reading_window(&mut self, window: &str) {
        if self.reading_windows.remove(window).is_none() {
            return;
        }
        if let Some(app) = &self.app {
            app.close_reading_window(window.to_owned());
        }
        if self.reading_window_focus.as_deref() == Some(window) {
            self.reading_window_focus = None;
        }
    }

    /// Re-pulls every open window's body.
    ///
    /// A `Surface::Reading` signal says that *some* reader's body changed, not which, so each open
    /// window reads its own slot back. On that signal only: a body is a large string, and there is
    /// no reason to copy one per window per sync.
    pub(super) fn reload_reading_windows(&mut self, app: &MailcalApp) {
        for (window, state) in &mut self.reading_windows {
            state.snapshot = app.reading_window_view(window.clone());
        }
    }

    /// The id the next detached draft would be raised under.
    ///
    /// Read before the request is built, because the request carries its host; the counter moves
    /// only once the window actually opens, so a compose that turns back has spent nothing.
    pub(super) fn next_composer_window(&self) -> u64 {
        self.composer_window_seq.wrapping_add(1)
    }

    pub(super) fn open_composer_window(&mut self, id: u64, request: ComposeContext) {
        debug_assert_eq!(request.host, ComposerHost::Window(id));
        self.composer_window_seq = id;
        self.composer_windows.push(DetachedDraft {
            id,
            request,
            error: None,
        });
    }

    /// Sent, cancelled, or closed: all three are the same act here, because closing a composer
    /// window finishes with the draft exactly as Cancel does (`docs/reading-window.md`).
    ///
    /// What it finishes with is the composer, not the message: the composition is forgotten and
    /// whatever it had saved stays in Drafts (`docs/drafts.md`).
    pub(super) fn close_composer_window(&mut self, id: u64) {
        let composition = self
            .composer_windows
            .iter()
            .find(|draft| draft.id == id)
            .map(|draft| draft.request.composition.clone());
        self.forget_composer_window(id);
        if let Some(composition) = composition {
            self.close_composition(&composition);
        }
    }

    /// Takes the window off screen without forgetting its composition: what a **sent** draft's
    /// window does, because the send owns the composition from the submit on (`docs/drafts.md`).
    pub(super) fn forget_composer_window(&mut self, id: u64) {
        self.composer_windows.retain(|draft| draft.id != id);
    }

    /// Reports a failure on the window that raised it, leaving the pane's own error line alone.
    pub(super) fn show_composer_error(&mut self, host: ComposerHost, notice: ComposerNotice) {
        match host {
            ComposerHost::Pane => self.composer_error = Some(notice),
            ComposerHost::Window(id) => {
                if let Some(draft) = self
                    .composer_windows
                    .iter_mut()
                    .find(|draft| draft.id == id)
                {
                    draft.error = Some(notice);
                }
            }
        }
    }
}
