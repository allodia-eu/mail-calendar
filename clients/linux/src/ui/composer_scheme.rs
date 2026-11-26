//! The app's light or dark, which the shared editor's chrome follows
//! (`clients/composer/src/scheme.ts`), and the page its message is written on
//! (`docs/reading-dark-mode.md`, "Writing in a dark app"), on the composer's and the signature
//! editor's web views.
//!
//! The scheme is the style manager's, which carries an appearance chosen in the app
//! (`appearance.rs`). WebKitGTK's own `prefers-color-scheme` is not asked: it answers from the GTK
//! settings, not from the colour scheme libadwaita was told to use. The page is dark exactly when
//! a reading view's would be, so both inputs of that rule move it: the style manager's flip, and
//! the dark-messages preference, which arrives with the core's Settings signal.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{composer_page_preload_script, composer_page_script};
use webkit6::{
    UserContentInjectedFrames, UserScript, UserScriptInjectionTime, WebView, prelude::*,
};

use super::reading::scheme;

thread_local! {
    /// The reader's dark-messages preference as the core last said it. On, the product default,
    /// until the core has been read.
    static DARK_MESSAGES: Cell<bool> = const { Cell::new(true) };
    /// Every editor that follows the app, for as long as it lives.
    static VIEWS: RefCell<Vec<glib::WeakRef<WebView>>> = const { RefCell::new(Vec::new()) };
}

/// The script naming the app's scheme to the chrome and the page to the message.
pub(crate) fn script(app_dark: bool, dark_messages: bool) -> String {
    let chrome = if app_dark { "dark" } else { "light" };
    format!(
        "window.setComposerScheme(\"{chrome}\");{}",
        composer_page_script(scheme::of(app_dark, dark_messages))
    )
}

/// The script naming both as they are now, for a seed script.
pub(crate) fn current_script() -> String {
    script(
        adw::StyleManager::default().is_dark(),
        DARK_MESSAGES.with(Cell::get),
    )
}

/// The script the web view runs before the editor's document does, naming both as they are now,
/// so the editor's first frame is drawn on them rather than on a light page that the seed script,
/// which runs once the document has loaded, then repaints.
pub(crate) fn preload_script(app_dark: bool, dark_messages: bool) -> String {
    let chrome = if app_dark { "dark" } else { "light" };
    format!(
        "window.composerScheme = \"{chrome}\";{}",
        composer_page_preload_script(scheme::of(app_dark, dark_messages))
    )
}

/// [`preload_script`] as it is now, as the user script the editor's load carries
/// (`SecureWebView::load_editor`): run at the document's start, in the page's own world, which is
/// the one the editor reads it from.
pub(crate) fn preload() -> UserScript {
    UserScript::new(
        &preload_script(
            adw::StyleManager::default().is_dark(),
            DARK_MESSAGES.with(Cell::get),
        ),
        UserContentInjectedFrames::TopFrame,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    )
}

/// Sends `view`'s editor each change of the app's scheme and of the page for as long as the view
/// lives. Both at load are the preload's (`preload`), named again by the seed script
/// (`current_script`).
pub(crate) fn follow(view: &WebView) {
    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        views.retain(|view| view.upgrade().is_some());
        views.push(view.downgrade());
        // The first editor connects the one handler; the style manager outlives every editor.
        if views.len() == 1 {
            follow_the_style_manager();
        }
    });
}

fn follow_the_style_manager() {
    static CONNECTED: std::sync::Once = std::sync::Once::new();
    CONNECTED.call_once(|| {
        adw::StyleManager::default().connect_dark_notify(|_| resend());
    });
}

/// Takes the preference as the core says it now, from its Settings signal; every editor is told
/// when that moves its page.
pub(crate) fn follow_dark_messages(dark: bool) {
    if DARK_MESSAGES.with(|held| held.replace(dark)) != dark {
        resend();
    }
}

fn resend() {
    let script = current_script();
    VIEWS.with(|views| {
        views.borrow_mut().retain(|view| {
            let Some(view) = view.upgrade() else {
                return false;
            };
            view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
            true
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{preload_script, script};

    #[test]
    fn names_the_chrome_the_apps_scheme_and_the_page_the_reading_views() {
        let dark = script(true, true);
        assert!(
            dark.starts_with("window.setComposerScheme(\"dark\");"),
            "{dark}"
        );
        assert!(
            dark.contains("window.setComposerPage({\"scheme\":\"dark\""),
            "{dark}"
        );
        // A reader who keeps messages light writes on a light page in a dark app.
        let kept_light = script(true, false);
        assert!(
            kept_light.starts_with("window.setComposerScheme(\"dark\");"),
            "{kept_light}"
        );
        assert!(kept_light.contains("{\"scheme\":\"light\""), "{kept_light}");
        let light = script(false, true);
        assert!(
            light.starts_with("window.setComposerScheme(\"light\");"),
            "{light}"
        );
        assert!(light.contains("{\"scheme\":\"light\""), "{light}");
    }

    #[test]
    fn hands_both_over_before_the_document_runs() {
        let preload = preload_script(true, true);
        assert!(
            preload.starts_with("window.composerScheme = \"dark\";"),
            "{preload}"
        );
        assert!(
            preload.contains("window.composerPage = {\"scheme\":\"dark\""),
            "{preload}"
        );
        let kept_light = preload_script(true, false);
        assert!(kept_light.contains("{\"scheme\":\"light\""), "{kept_light}");
    }
}
