//! The app's light or dark, which the shared editor's chrome follows
//! (`clients/composer/src/scheme.ts`), on the composer's and the signature editor's web views.
//!
//! The scheme is the style manager's, which carries an appearance chosen in the app
//! (`appearance.rs`). WebKitGTK's own `prefers-color-scheme` is not asked: it answers from the GTK
//! settings, not from the colour scheme libadwaita was told to use.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::gio;
use webkit6::{WebView, prelude::*};

/// The script naming `dark`.
pub(crate) fn script(dark: bool) -> String {
    let scheme = if dark { "dark" } else { "light" };
    format!("window.setComposerScheme(\"{scheme}\");")
}

/// The script naming the app's scheme as it is now, for a seed script.
pub(crate) fn current_script() -> String {
    script(adw::StyleManager::default().is_dark())
}

/// Sends `view`'s editor each change of the app's scheme for as long as the view lives. The scheme
/// at load is the seed script's (`current_script`).
pub(crate) fn follow(view: &WebView) {
    let manager = adw::StyleManager::default();
    // Weak: the style manager outlives every editor.
    let weak = view.downgrade();
    let handler = manager.connect_dark_notify(move |manager| {
        if let Some(view) = weak.upgrade() {
            view.evaluate_javascript(
                &script(manager.is_dark()),
                None,
                None,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
    });
    let handler = RefCell::new(Some(handler));
    view.connect_destroy(move |_| {
        if let Some(handler) = handler.take() {
            manager.disconnect(handler);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::script;

    #[test]
    fn names_the_scheme_the_bundle_knows() {
        assert_eq!(script(true), "window.setComposerScheme(\"dark\");");
        assert_eq!(script(false), "window.setComposerScheme(\"light\");");
    }
}
