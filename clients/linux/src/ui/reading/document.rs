//! The message document in the reading pane's web view, and when it is built again.
//!
//! The pane renders on every update, and the document is a walk of the whole message, so it is
//! built only when what it is built from moved: the body, the remote-images choice, or the scheme
//! (`../../../../docs/reading-dark-mode.md`). A change of scheme alone keeps the reader's place;
//! anything else is a different document and opens at its top.

use std::cell::RefCell;

use mailcal_bindings::{MessageScheme, render_message_html};

use crate::ui::webview::SecureWebView;

/// How the next render puts the body on the page.
#[derive(Debug, PartialEq, Eq)]
enum Load {
    /// The page already shows it.
    Nothing,
    /// The same body in another scheme: where the reader left it.
    KeepingPlace,
    /// A different body, at its top.
    Fresh,
}

/// The body last handed to the web view, its remote-images choice and the scheme it was drawn in.
#[derive(Default)]
pub(super) struct ShownBody(RefCell<Option<(String, bool, MessageScheme)>>);

impl ShownBody {
    /// Puts `html` on `web` in `scheme`, unless it is already there.
    pub(super) fn draw(
        &self,
        web: &SecureWebView,
        html: &str,
        remote_images: bool,
        scheme: MessageScheme,
    ) {
        let load = next(self.0.borrow().as_ref(), html, remote_images, scheme);
        if load == Load::Nothing {
            return;
        }
        let document = render_message_html(html.to_owned(), remote_images, scheme);
        if load == Load::KeepingPlace {
            web.load_keeping_place(&document, remote_images);
        } else {
            web.load(&document, remote_images);
        }
        self.0
            .replace(Some((html.to_owned(), remote_images, scheme)));
    }

    /// The web view was emptied: whatever comes next is drawn afresh.
    pub(super) fn forget(&self) {
        self.0.replace(None);
    }
}

/// Compared with the scheme the body was drawn in, not the last render's: the scheme can change
/// while another message, or none, is open, and the body has to follow it on return.
fn next(
    shown: Option<&(String, bool, MessageScheme)>,
    html: &str,
    remote_images: bool,
    scheme: MessageScheme,
) -> Load {
    match shown {
        Some((body, remote, drawn)) if body == html && *remote == remote_images => {
            if *drawn == scheme {
                Load::Nothing
            } else {
                Load::KeepingPlace
            }
        }
        _ => Load::Fresh,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::MessageScheme::{Dark, Light};

    use super::{Load, next};

    #[test]
    fn a_body_is_built_again_only_when_what_it_is_built_from_moved() {
        let shown = ("<p>a</p>".to_owned(), false, Dark);
        assert_eq!(next(None, "<p>a</p>", false, Dark), Load::Fresh);
        assert_eq!(next(Some(&shown), "<p>a</p>", false, Dark), Load::Nothing);
        // Also the body coming back after the sun was pressed on a plain-text message: no render
        // saw the scheme move while this body was on screen, and it still has to follow.
        assert_eq!(
            next(Some(&shown), "<p>a</p>", false, Light),
            Load::KeepingPlace
        );
        assert_eq!(next(Some(&shown), "<p>b</p>", false, Dark), Load::Fresh);
        assert_eq!(next(Some(&shown), "<p>a</p>", true, Dark), Load::Fresh);
    }
}
