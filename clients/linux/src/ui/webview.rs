//! Hardened WebKitGTK hosts shared by the reading view and rich composer.

use std::{cell::Cell, rc::Rc};

use gtk::{gio, glib, prelude::*};
use webkit6::{
    ContextMenuAction, ContextMenuItem, NavigationPolicyDecision, NetworkSession,
    PolicyDecisionType, Settings, UserContentFilter, UserContentFilterStore, UserContentManager,
    UserScript, WebView, prelude::*,
};

use super::{
    AppInput,
    web_security::{is_initial_document_uri, network_block_filter},
};

mod paint;
mod place;
mod zoom;

use paint::PaintGate;
use zoom::install_zoom_gestures;
#[cfg(test)]
use zoom::{PINCH_CONTROLLER, SCROLL_CONTROLLER, clamp_zoom};

/// Whether the local document is inert mail or the trusted editor bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentKind {
    Reading,
    Composer,
}

/// A WebView whose host-level gates are installed before any document is loaded. A clone is a
/// second handle on the same view, for a callback that has to load into it later.
#[derive(Clone)]
pub(crate) struct SecureWebView {
    view: WebView,
    manager: UserContentManager,
    filter: Rc<std::cell::RefCell<Option<UserContentFilter>>>,
    filter_ready: Rc<Cell<bool>>,
    filter_failed: Rc<Cell<bool>>,
    expecting_load: Rc<Cell<bool>>,
    last_document: Rc<std::cell::RefCell<Option<(String, bool)>>>,
    paint: Rc<std::cell::RefCell<PaintGate>>,
    /// The composer's answer to a paste that carries a picture, which its context menu's Paste
    /// needs (`super::editor_paste`). Held behind a cell because the handler wants the
    /// composer's own error line, and that does not exist until the composer is built around
    /// this view; a reading view never sets one.
    paste: Rc<std::cell::RefCell<Option<super::editor_paste::PasteAnswer>>>,
}

#[derive(Clone)]
struct FilterSetup {
    view: WebView,
    manager: UserContentManager,
    filter: Rc<std::cell::RefCell<Option<UserContentFilter>>>,
    ready: Rc<Cell<bool>>,
    failed: Rc<Cell<bool>>,
    expecting_load: Rc<Cell<bool>>,
    last_document: Rc<std::cell::RefCell<Option<(String, bool)>>>,
}

impl SecureWebView {
    pub(crate) fn new(kind: DocumentKind, sender: relm4::Sender<AppInput>) -> Self {
        let manager = UserContentManager::new();
        let settings = hardened_settings(kind);
        let view = WebView::builder()
            .network_session(&NetworkSession::new_ephemeral())
            .settings(&settings)
            .user_content_manager(&manager)
            .build();
        view.set_hexpand(true);
        view.set_vexpand(true);
        if kind == DocumentKind::Reading {
            // The base the web process presents until the document has painted. Left at the
            // toolkit's default it can arrive **black** on a large view (WebKit composites the
            // page on its own surface, and a first frame of that surface at full-screen size
            // beats the paint to the screen), which puts the flicker back for exactly the
            // messages heavy enough to be slow to lay out. The message canvas is the honest
            // answer: it is what the document is about to paint anyway
            // (`super::reading::canvas`). The composer's page is themed, not this canvas, so it
            // keeps the toolkit's default.
            // The reading pane sets it again for the scheme of each message it draws.
            view.set_background_color(&super::reading::canvas::canvas_rgba(
                mailcal_bindings::MessageScheme::Light,
            ));
        }

        let expecting_load = Rc::new(Cell::new(false));
        let paste = Rc::new(std::cell::RefCell::new(None));
        install_navigation_gates(&view, kind, Rc::clone(&expecting_load), Rc::clone(&paste));
        if kind == DocumentKind::Reading {
            install_zoom_gestures(&view);
        }

        let paint = Rc::new(std::cell::RefCell::new(PaintGate::default()));
        if kind == DocumentKind::Reading {
            let load_reported = Rc::clone(&paint);
            let load_sender = sender.clone();
            view.connect_load_changed(move |_, event| {
                if load_reported.borrow_mut().observed(event) {
                    // Nothing reads this input; it exists to provoke the render that reveals
                    // the page now that there is one to reveal. The composer keeps its own
                    // `connect_finished`, and has no page to hold back.
                    load_sender.emit(AppInput::ReadingBodyPainted);
                }
            });
        }

        let filter = Rc::new(std::cell::RefCell::new(None));
        let filter_ready = Rc::new(Cell::new(false));
        let filter_failed = Rc::new(Cell::new(false));
        let last_document = Rc::new(std::cell::RefCell::new(None));
        let filter_setup = FilterSetup {
            view: view.clone(),
            manager: manager.clone(),
            filter: Rc::clone(&filter),
            ready: Rc::clone(&filter_ready),
            failed: Rc::clone(&filter_failed),
            expecting_load: Rc::clone(&expecting_load),
            last_document: Rc::clone(&last_document),
        };
        compile_filter(&filter_setup, sender, kind);

        Self {
            view,
            manager,
            filter,
            filter_ready,
            filter_failed,
            expecting_load,
            last_document,
            paint,
            paste,
        }
    }

    /// Gives the composer's editor its answer to a paste that carries a picture or a file, so the
    /// context menu's Paste reaches the same place the Ctrl+V chord does.
    pub(crate) fn set_paste_handler(&self, paste: super::editor_paste::PasteAnswer) {
        self.paste.replace(Some(paste));
    }

    /// Whether the document last asked for is on screen; see [`PaintGate`].
    pub(crate) fn painted(&self) -> bool {
        self.paint.borrow().painted()
    }

    pub(crate) fn widget(&self) -> &WebView {
        &self.view
    }

    /// The base the view presents until a document has painted: the page of the scheme the next
    /// message is drawn in (`super::reading::canvas`).
    pub(crate) fn set_page(&self, page: &gtk::gdk::RGBA) {
        self.view.set_background_color(page);
    }

    /// Loads an in-memory document with an opaque origin. `allow_remote_images` removes the
    /// native HTTP(S) filter only after the user's per-message opt-in; the shared document CSP
    /// still permits images alone and blocks every other remote resource type.
    pub(crate) fn load(&self, document: &str, allow_remote_images: bool) {
        if self.claim(document, allow_remote_images) {
            self.hand_over(document, allow_remote_images, None, false);
        }
    }

    /// Loads the shared editor's document with `preload` run at its start, before the editor's own
    /// script, so the editor's first frame is drawn on what it says (`composer_scheme::preload`).
    /// A load replaces every script the view had, so the preload goes with the load and nowhere
    /// else.
    pub(crate) fn load_editor(&self, document: &str, preload: &UserScript) {
        if self.claim(document, false) {
            self.hand_over(document, false, Some(preload), false);
        }
    }

    /// Records `document` as the one this view is showing, unless it already is (or the view
    /// cannot load at all), and says whether the caller should go on and load it.
    fn claim(&self, document: &str, allow_remote_images: bool) -> bool {
        if self.filter_failed.get() {
            return false;
        }
        let next = (document.to_owned(), allow_remote_images);
        if self.last_document.borrow().as_ref() == Some(&next) {
            return false;
        }
        self.last_document.replace(Some(next));
        true
    }

    /// Hands a claimed document to WebKit, with `script` run in it at document end, in a world of
    /// its own (`place`), or no script at all. `same_message` keeps the reader's zoom: the same
    /// message drawn in another scheme is still the one the zoom was made on.
    fn hand_over(
        &self,
        document: &str,
        allow_remote_images: bool,
        script: Option<&UserScript>,
        same_message: bool,
    ) {
        self.paint.borrow_mut().asked();
        // A zoom belongs to the message it was made on (`docs/reading-zoom.md`). `zoom-level` is
        // the view's, not the document's, so it outlives a load. Reset here rather than beside
        // each `load_html`: the deferred replay in `compile_filter` re-loads the document this
        // call registered, so it is the same message and must keep nothing of its own. The script
        // is installed before that replay can happen, for the same reason.
        if !same_message {
            self.view.set_zoom_level(1.0);
        }
        self.manager.remove_all_scripts();
        if let Some(script) = script {
            self.manager.add_script(script);
        }
        if !self.filter_ready.get() {
            return;
        }
        self.manager.remove_all_filters();
        if !allow_remote_images && let Some(filter) = self.filter.borrow().as_ref() {
            self.manager.add_filter(filter);
        }
        self.expecting_load.set(true);
        self.view.load_html(document, None);
    }

    pub(crate) fn clear(&self) {
        self.last_document.replace(None);
        self.paint.borrow_mut().asked();
        self.view.set_zoom_level(1.0);
        if !self.filter_ready.get() {
            return;
        }
        self.manager.remove_all_filters();
        if let Some(filter) = self.filter.borrow().as_ref() {
            self.manager.add_filter(filter);
        }
        self.expecting_load.set(true);
        self.view
            .load_html("<!doctype html><html><body></body></html>", None);
    }

    pub(crate) fn connect_finished<F: Fn(&WebView) + 'static>(&self, callback: F) {
        self.view.connect_load_changed(move |view, event| {
            if event == webkit6::LoadEvent::Finished {
                callback(view);
            }
        });
    }
}

/// The composer's right-click menu, rebuilt from stock editing actions rather than shown as
/// WebKit's own. Returning `true` suppresses the menu, which is what the reading view gets.
///
/// The default menu carries items this composer must not offer: opening a link (navigation is
/// blocked), downloading one, reloading the document, and Inspect Element. So it is emptied and
/// re-filled with the actions that make sense in a message being written.
///
/// **The link item is the one that has to be here.** A link inside a quoted original cannot be
/// clicked open in the composer, so without a way to copy its address it is text the user can see
/// and not use. Every client offers the same set; the labels are the toolkit's own, so they are
/// already in the user's language.
fn build_context_menu(
    kind: DocumentKind,
    view: &WebView,
    menu: &webkit6::ContextMenu,
    hit_test: &webkit6::HitTestResult,
    paste: &Rc<std::cell::RefCell<Option<super::editor_paste::PasteAnswer>>>,
) -> bool {
    if kind != DocumentKind::Composer {
        return true;
    }
    menu.remove_all();
    if hit_test.context_is_link() {
        menu.append(&ContextMenuItem::from_stock_action(
            ContextMenuAction::CopyLinkToClipboard,
        ));
        menu.append(&ContextMenuItem::new_separator());
    }
    for action in [ContextMenuAction::Cut, ContextMenuAction::Copy] {
        menu.append(&ContextMenuItem::from_stock_action(action));
    }
    // Paste is the composer's own, because the stock one cannot put a picture in the message on
    // this toolkit; it keeps the stock label and falls back to the stock behaviour for everything
    // that is not a picture (`super::editor_paste`).
    menu.append(&super::editor_paste::paste_menu_item(
        view,
        paste.borrow().as_ref(),
    ));
    menu.append(&ContextMenuItem::new_separator());
    menu.append(&ContextMenuItem::from_stock_action(
        ContextMenuAction::SelectAll,
    ));
    false
}

fn hardened_settings(kind: DocumentKind) -> Settings {
    let settings = Settings::new();
    // The reading host runs no script of the page's: markup script is stripped as the document is
    // parsed, and the CSP forbids script besides. The engine stays on only for the app's own,
    // which keeps the reader's place across a scheme change in a world the page cannot reach
    // (`place`, `docs/rendering-security.md` gate 1). The composer is the trusted editor bundle.
    settings.set_enable_javascript(true);
    settings.set_enable_javascript_markup(kind == DocumentKind::Composer);
    settings.set_allow_file_access_from_file_urls(false);
    settings.set_allow_universal_access_from_file_urls(false);
    settings.set_allow_modal_dialogs(false);
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_media_stream(false);
    settings.set_enable_webrtc(false);
    settings.set_enable_webgl(false);
    settings
}

fn install_navigation_gates(
    view: &WebView,
    kind: DocumentKind,
    expecting_load: Rc<Cell<bool>>,
    paste: Rc<std::cell::RefCell<Option<super::editor_paste::PasteAnswer>>>,
) {
    view.connect_context_menu(move |view, menu, hit_test| {
        build_context_menu(kind, view, menu, hit_test, &paste)
    });
    view.connect_create(|_, _| None);
    view.connect_permission_request(|_, request| {
        request.deny();
        true
    });
    view.connect_decide_policy(move |_, decision, decision_type| {
        if matches!(
            decision_type,
            PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction
        ) {
            let navigation = decision.downcast_ref::<NavigationPolicyDecision>();
            let action = navigation.and_then(NavigationPolicyDecision::navigation_action);
            let uri = action
                .as_ref()
                .and_then(webkit6::NavigationAction::request)
                .and_then(|request| request.uri());
            if decision_type == PolicyDecisionType::NavigationAction
                && expecting_load.replace(false)
                && is_initial_document_uri(uri.as_deref())
            {
                decision.use_();
                return true;
            }
            if kind == DocumentKind::Reading
                && let Some(action) = action
                && action.is_user_gesture()
                && let Some(uri) = uri
                && mailcal_bindings::should_open_external_link(uri.to_string())
            {
                // `GtkUriLauncher`, not `AppInfo`: the latter resolves against the desktop's
                // application database, which a sandboxed build does not have, and blocks the
                // main thread while GIO falls back onto the session bus.
                gtk::UriLauncher::new(&uri).launch(
                    None::<&gtk::Window>,
                    gio::Cancellable::NONE,
                    |_| (),
                );
            }
            decision.ignore();
            return true;
        }
        false
    });
}

thread_local! {
    /// The filter each kind compiled first, which every later view of that kind is given: its
    /// source is fixed, so compiling and writing it again for every view only costs time.
    static COMPILED: std::cell::RefCell<[Option<UserContentFilter>; 2]> =
        const { std::cell::RefCell::new([None, None]) };
}

/// Where a view's filter comes from: one this kind compiled already, or a store to compile it in.
enum FilterSource {
    Compiled(UserContentFilter),
    Compile(UserContentFilterStore),
}

fn compile_filter(setup: &FilterSetup, sender: relm4::Sender<AppInput>, kind: DocumentKind) {
    let slot = usize::from(kind == DocumentKind::Composer);
    let identifier = match kind {
        DocumentKind::Reading => "mailcal-reading-network-block-v1",
        DocumentKind::Composer => "mailcal-composer-network-block-v1",
    };
    let source = if let Some(filter) = COMPILED.with_borrow(|compiled| compiled[slot].clone()) {
        FilterSource::Compiled(filter)
    } else {
        let directory = glib::user_cache_dir().join("mailcal/webkit-filters");
        if std::fs::create_dir_all(&directory).is_err() {
            setup.failed.set(true);
            sender.emit(AppInput::WebViewUnavailable);
            return;
        }
        FilterSource::Compile(UserContentFilterStore::new(&directory.to_string_lossy()))
    };
    let setup = setup.clone();
    glib::MainContext::default().spawn_local(async move {
        let filter = match source {
            FilterSource::Compiled(filter) => Ok(filter),
            FilterSource::Compile(store) => {
                let rules = glib::Bytes::from_static(network_block_filter().as_bytes());
                let saved = store.save_future(identifier, &rules).await;
                if let Ok(filter) = &saved {
                    COMPILED.with_borrow_mut(|compiled| compiled[slot] = Some(filter.clone()));
                }
                saved
            }
        };
        if let Ok(filter) = filter {
            setup.filter.replace(Some(filter));
            setup.ready.set(true);
            if let Some((document, allow_remote_images)) = setup.last_document.borrow().clone() {
                setup.manager.remove_all_filters();
                if !allow_remote_images && let Some(filter) = setup.filter.borrow().as_ref() {
                    setup.manager.add_filter(filter);
                }
                setup.expecting_load.set(true);
                setup.view.load_html(&document, None);
            }
            sender.emit(AppInput::WebViewReady);
        } else {
            setup.failed.set(true);
            sender.emit(AppInput::WebViewUnavailable);
        }
    });
}

#[cfg(test)]
#[path = "webview_tests.rs"]
pub(crate) mod tests;
