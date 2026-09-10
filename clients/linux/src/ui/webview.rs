//! Hardened WebKitGTK hosts shared by the reading view and rich composer.

use std::{cell::Cell, rc::Rc};

use gtk::{gio, glib, prelude::*};
use webkit6::{
    ContextMenuAction, ContextMenuItem, NavigationPolicyDecision, NetworkSession,
    PolicyDecisionType, Settings, UserContentFilter, UserContentFilterStore, UserContentManager,
    WebView, prelude::*,
};

use super::{
    AppInput,
    web_security::{is_initial_document_uri, network_block_filter},
};

/// Whether the local document is inert mail or the trusted editor bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentKind {
    Reading,
    Composer,
}

/// A WebView whose host-level gates are installed before any document is loaded.
pub(crate) struct SecureWebView {
    view: WebView,
    manager: UserContentManager,
    filter: Rc<std::cell::RefCell<Option<UserContentFilter>>>,
    filter_ready: Rc<Cell<bool>>,
    filter_failed: Rc<Cell<bool>>,
    expecting_load: Rc<Cell<bool>>,
    last_document: Rc<std::cell::RefCell<Option<(String, bool)>>>,
    paint: Rc<std::cell::RefCell<PaintGate>>,
}

/// Whether the document last handed to [`SecureWebView::load`] has finished painting.
///
/// The page a message renders is white, but the surface WebKit composites it on is not: on a
/// large view the web process presents a **black** frame until the document's first paint, and on
/// a heavy message that is hundreds of milliseconds (467 ms, measured, at full screen). A host
/// that reveals the view as soon as it asks for a document therefore shows black in the middle of
/// the reading pane's page. `webkit_web_view_set_background_color` does not reach it: that colour
/// is what the *page* renders on, not what the compositor presents before there is a page.
///
/// `Finished` alone does not answer the question, which is why this is a pair rather than a flag.
/// A load cancelled by the next `load_html` still reaches `Finished`, and it arrives **after** the
/// call that cancelled it, so the naive reading hands the previous message's completion to the
/// message now loading and reveals an unpainted view. Requiring a `Started` since the last ask
/// separates the two. The measured sequences are the fixtures this module's tests drive.
#[derive(Default)]
struct PaintGate {
    /// Whether a load has begun since the last [`Self::asked`]. Only a `Finished` after one
    /// belongs to the document we are waiting for.
    started: bool,
    painted: bool,
}

impl PaintGate {
    /// A document was handed to the view: nothing on screen belongs to it yet.
    fn asked(&mut self) {
        self.started = false;
        self.painted = false;
    }

    /// Folds in one `load-changed`, and reports whether the page just arrived.
    fn observed(&mut self, event: webkit6::LoadEvent) -> bool {
        match event {
            webkit6::LoadEvent::Started => {
                self.started = true;
                false
            }
            webkit6::LoadEvent::Finished if self.started => {
                self.painted = true;
                true
            }
            _ => false,
        }
    }

    fn painted(&self) -> bool {
        self.painted
    }
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
            view.set_background_color(&super::reading::canvas::canvas_rgba());
        }

        let expecting_load = Rc::new(Cell::new(false));
        install_navigation_gates(&view, kind, Rc::clone(&expecting_load));
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
        }
    }

    /// Whether the document last asked for is on screen; see [`PaintGate`].
    pub(crate) fn painted(&self) -> bool {
        self.paint.borrow().painted()
    }

    pub(crate) fn widget(&self) -> &WebView {
        &self.view
    }

    /// Loads an in-memory document with an opaque origin. `allow_remote_images` removes the
    /// native HTTP(S) filter only after the user's per-message opt-in; the shared document CSP
    /// still permits images alone and blocks every other remote resource type.
    pub(crate) fn load(&self, document: &str, allow_remote_images: bool) {
        if self.filter_failed.get() {
            return;
        }
        let next = (document.to_owned(), allow_remote_images);
        if self.last_document.borrow().as_ref() == Some(&next) {
            return;
        }
        self.last_document.replace(Some(next.clone()));
        self.paint.borrow_mut().asked();
        // A zoom belongs to the message it was made on (`docs/reading-zoom.md`). `zoom-level` is
        // the view's, not the document's, so it outlives a load. Reset here rather than beside
        // each `load_html`: the deferred replay in `compile_filter` re-loads the document this
        // call registered, so it is the same message and must keep nothing of its own.
        self.view.set_zoom_level(1.0);
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
    menu: &webkit6::ContextMenu,
    hit_test: &webkit6::HitTestResult,
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
    for action in [
        ContextMenuAction::Cut,
        ContextMenuAction::Copy,
        ContextMenuAction::Paste,
    ] {
        menu.append(&ContextMenuItem::from_stock_action(action));
    }
    menu.append(&ContextMenuItem::new_separator());
    menu.append(&ContextMenuItem::from_stock_action(
        ContextMenuAction::SelectAll,
    ));
    false
}

/// The range the reader may zoom the message to, matching what a browser offers.
const ZOOM_RANGE: std::ops::RangeInclusive<f64> = 0.25..=5.0;

/// Pinch and Ctrl+scroll zoom for the reading view (`docs/reading-zoom.md`). WebKitGTK carries a
/// `zoom-level` but binds no gesture to it, so unlike the other three hosts this one has to supply
/// both itself.
///
/// ⚠️ **Both controllers must be in the `Capture` phase.** The web view claims a pinch and a scroll
/// for its own handling, so a controller left in the default `Bubble` phase is never reached and
/// the gesture silently does nothing. A scroll without Ctrl still `Proceed`s, so ordinary scrolling
/// reaches the page untouched.
fn install_zoom_gestures(view: &WebView) {
    let zoom = gtk::GestureZoom::new();
    zoom.set_propagation_phase(gtk::PropagationPhase::Capture);
    // The scale a pinch reports is relative to where that pinch began, not to 1, so the level it
    // started from has to be captured when it does.
    let started_at = Rc::new(Cell::new(1.0_f64));
    let begin_view = view.clone();
    let begin_level = Rc::clone(&started_at);
    zoom.connect_begin(move |_, _| begin_level.set(begin_view.zoom_level()));
    let pinch_view = view.clone();
    zoom.connect_scale_changed(move |_, scale| {
        pinch_view.set_zoom_level(clamp_zoom(started_at.get() * scale));
    });
    view.add_controller(zoom);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scroll_view = view.clone();
    scroll.connect_scroll(move |controller, _, dy| {
        if !controller
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        // A tenth per notch, compounding, so a step covers the same proportion of the range
        // wherever the reader already is.
        let step = if dy < 0.0 { 1.1 } else { 1.0 / 1.1 };
        scroll_view.set_zoom_level(clamp_zoom(scroll_view.zoom_level() * step));
        glib::Propagation::Stop
    });
    view.add_controller(scroll);
}

/// Holds a zoom level inside `ZOOM_RANGE`. A pinch reports an unbounded scale, so without this a
/// long enough one leaves the message at a size with no gesture back.
fn clamp_zoom(level: f64) -> f64 {
    level.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end())
}

fn hardened_settings(kind: DocumentKind) -> Settings {
    let settings = Settings::new();
    settings.set_enable_javascript(kind == DocumentKind::Composer);
    settings.set_allow_file_access_from_file_urls(false);
    settings.set_allow_universal_access_from_file_urls(false);
    settings.set_allow_modal_dialogs(false);
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_enable_dns_prefetching(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_offline_web_application_cache(false);
    settings.set_enable_media_stream(false);
    settings.set_enable_webrtc(false);
    settings.set_enable_webgl(false);
    settings
}

fn install_navigation_gates(view: &WebView, kind: DocumentKind, expecting_load: Rc<Cell<bool>>) {
    view.connect_context_menu(move |_, menu, hit_test| build_context_menu(kind, menu, hit_test));
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

fn compile_filter(setup: &FilterSetup, sender: relm4::Sender<AppInput>, kind: DocumentKind) {
    let directory = glib::user_cache_dir().join("mailcal/webkit-filters");
    if std::fs::create_dir_all(&directory).is_err() {
        setup.failed.set(true);
        sender.emit(AppInput::WebViewUnavailable);
        return;
    }
    let store = UserContentFilterStore::new(&directory.to_string_lossy());
    let source = glib::Bytes::from_static(network_block_filter().as_bytes());
    let setup = setup.clone();
    let identifier = match kind {
        DocumentKind::Reading => "mailcal-reading-network-block-v1",
        DocumentKind::Composer => "mailcal-composer-network-block-v1",
    };
    glib::MainContext::default().spawn_local(async move {
        if let Ok(filter) = store.save_future(identifier, &source).await {
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
