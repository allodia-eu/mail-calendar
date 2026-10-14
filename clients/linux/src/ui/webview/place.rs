//! Keeping the reader's place when the same message is drawn again in another colour scheme
//! (`docs/reading-dark-mode.md`, rule 9).
//!
//! Only the colours changed, so the layout and the old position are the same. WebKitGTK reports no
//! scroll position, so it is read and set by script the app runs in a world of its own: page
//! script cannot see that world, and the reading host's settings strip the page's own script as
//! the document is parsed (`super::hardened_settings`).

use gtk::gio;
use webkit6::{UserContentInjectedFrames, UserScript, UserScriptInjectionTime, prelude::*};

use super::SecureWebView;

/// The isolated world the place is read and set in.
const WORLD: &str = "mailcal-reader";

impl SecureWebView {
    /// Loads `document`, the message on screen drawn in another scheme, where the reader left it.
    ///
    /// The document is claimed at once, so a render arriving while the place is read finds it
    /// current and does not load it a second time without the place; one superseded by a
    /// different document meanwhile is dropped.
    pub(crate) fn load_keeping_place(&self, document: &str, allow_remote_images: bool) {
        if !self.claim(document, allow_remote_images) {
            return;
        }
        let claimed = Some((document.to_owned(), allow_remote_images));
        let web = self.clone();
        self.view.evaluate_javascript(
            "window.scrollX + ',' + window.scrollY",
            Some(WORLD),
            None,
            None::<&gio::Cancellable>,
            move |result| {
                if *web.last_document.borrow() != claimed {
                    return;
                }
                let script = result
                    .ok()
                    .and_then(|value| place(&value.to_str()))
                    .map(|(x, y)| {
                        UserScript::for_world(
                            &format!("window.scrollTo({x}, {y})"),
                            UserContentInjectedFrames::TopFrame,
                            UserScriptInjectionTime::End,
                            WORLD,
                            &[],
                            &[],
                        )
                    });
                if let Some((document, allow_remote_images)) = claimed {
                    web.hand_over(&document, allow_remote_images, script.as_ref(), true);
                }
            },
        );
    }
}

/// `x,y` as the read reports it, or `None` at the very top, where there is nothing to restore.
fn place(reported: &str) -> Option<(f64, f64)> {
    let (x, y) = reported.split_once(',')?;
    let x = x.trim().parse::<f64>().ok().filter(|x| x.is_finite())?;
    let y = y.trim().parse::<f64>().ok().filter(|y| y.is_finite())?;
    (x.abs() + y.abs() > 0.0).then_some((x, y))
}

#[cfg(test)]
mod tests {
    use super::place;

    #[test]
    fn a_reported_place_is_read_and_the_top_is_nothing_to_restore() {
        assert_eq!(place("0,1240.5"), Some((0.0, 1240.5)));
        assert_eq!(place("0,0"), None);
        assert_eq!(place("NaN,12"), None);
        assert_eq!(place("undefined"), None);
    }
}
