//! Hardened WebKitGTK hosts shared by the reading view and rich composer.

use std::{cell::Cell, rc::Rc};

use gtk::{gio, glib, prelude::*};
use webkit6::{
    NavigationPolicyDecision, NetworkSession, PolicyDecisionType, Settings, UserContentFilter,
    UserContentFilterStore, UserContentManager, WebView, prelude::*,
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

        let expecting_load = Rc::new(Cell::new(false));
        install_navigation_gates(&view, kind, Rc::clone(&expecting_load));

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
        }
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
    view.connect_context_menu(|_, _, _| true);
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
