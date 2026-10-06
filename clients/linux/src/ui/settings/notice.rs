//! What a change on an account's page came to, said where the person is looking: a toast for
//! progress and success, a dialog to dismiss for a failure. Never a banner at the top of the
//! page, which a person who scrolled down to the button they pressed does not see.
//!
//! The window rebuilds its pages on every redraw, so the toasts live in an overlay above them
//! that the window keeps, and each notice carries the number it was raised under so a redraw
//! never shows it twice.

use adw::prelude::*;

use crate::l10n;

/// One thing to tell the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Notice {
    /// Progress or success.
    Toast(String),
    /// A failure, with what failed and why.
    Error { title: String, detail: String },
}

/// A notice and the number it was raised under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui) struct Raised {
    pub(in crate::ui) id: u64,
    pub(in crate::ui) notice: Notice,
}

/// The notice the window has not shown yet, if any: `shown` is the last one it did.
pub(super) fn due(raised: Option<&Raised>, shown: u64) -> Option<&Raised> {
    raised.filter(|raised| raised.id != shown)
}

/// Shows the notice `window` has not shown yet, if any, and records it as shown.
pub(super) fn show_due(
    window: &gtk::Window,
    toasts: &adw::ToastOverlay,
    raised: Option<&Raised>,
    shown: &mut u64,
) {
    if let Some(raised) = due(raised, *shown) {
        show(window, toasts, &raised.notice);
        *shown = raised.id;
    }
}

/// Shows `notice` over `window`.
pub(super) fn show(window: &gtk::Window, toasts: &adw::ToastOverlay, notice: &Notice) {
    match notice {
        Notice::Toast(text) => {
            toasts.add_toast(adw::Toast::builder().title(text).use_markup(false).build());
        }
        Notice::Error { title, detail } => {
            let dialog = adw::AlertDialog::new(Some(title), Some(detail));
            dialog.add_response("close", l10n::action_close());
            dialog.set_close_response("close");
            dialog.present(Some(window));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Notice, Raised, due};

    #[test]
    fn a_notice_is_shown_once_however_often_the_window_redraws() {
        let raised = Raised {
            id: 3,
            notice: Notice::Toast("Signed in again.".to_owned()),
        };
        assert_eq!(due(Some(&raised), 2), Some(&raised));
        assert_eq!(due(Some(&raised), 3), None);
        assert_eq!(due(None, 0), None);
    }
}
